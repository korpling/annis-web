use axum_sessions::extractors::ReadableSession;
use std::collections::{BTreeMap, BTreeSet};
use tokio::sync::mpsc::Sender;

use graphannis::graph::AnnoKey;
use transient_btree_index::BtreeIndex;

use crate::{
    client::{
        corpora,
        search::{self, FindQuery},
    },
    state::GlobalAppState,
    Result,
};

pub struct CSVExporter {
    query: FindQuery,
    annotations_for_matched_nodes: BTreeMap<usize, BTreeSet<AnnoKey>>,
    progress: Option<Sender<f32>>,
}

const SINGLE_STEP_PROGRESS: f32 = 1.0 / 3.0;
const FIRST_PASS_PROGRESS: f32 = SINGLE_STEP_PROGRESS;
const SECOND_PASS_PROGRESS: f32 = SINGLE_STEP_PROGRESS * 2.0;

impl CSVExporter {
    pub fn new(query: FindQuery, progress: Option<Sender<f32>>) -> Self {
        Self {
            query,
            annotations_for_matched_nodes: BTreeMap::new(),
            progress,
        }
    }

    pub async fn convert_text<W: std::io::Write>(
        &mut self,
        session_id: &str,
        state: &GlobalAppState,
        limit: Option<u64>,
        output: &mut W,
    ) -> Result<()> {
        // Get all the matches as Salt ID
        let mut query = self.query.clone();
        query.limit = limit;

        let result = search::find(&query, state, session_id).await?;

        if let Some(progress) = &self.progress {
            progress.send(FIRST_PASS_PROGRESS).await?;
        }
        self.first_pass(&result, state, session_id).await?;

        if let Some(progress) = &self.progress {
            progress.send(SECOND_PASS_PROGRESS).await?;
        }
        self.second_pass(&result, state, session_id, output).await?;

        if let Some(progress) = &self.progress {
            progress.send(1.0).await?;
        }
        Ok(())
    }

    async fn first_pass(
        &mut self,
        matches: &BtreeIndex<u64, Vec<String>>,
        state: &GlobalAppState,
        session_id: &str,
    ) -> Result<()> {
        for m in matches.range(..)? {
            let (match_nr, node_ids) = m?;
            // Get the corpus from the first node
            if let Some(id) = node_ids.first() {
                let (corpus, _) = id.split_once('/').unwrap_or_default();
                // Get the subgraph for the IDs
                let g = corpora::subgraph(session_id, corpus, node_ids.clone(), None, 1, 1, state)
                    .await?;
                // Collect annotations for the matched nodes
                for (pos_in_match, node_name) in node_ids.iter().enumerate() {
                    if let Some(n_id) = g.get_node_id_from_name(node_name)? {
                        let annos = g
                            .get_node_annos()
                            .get_annotations_for_item(&n_id)?
                            .into_iter()
                            .filter(|a| a.key.ns != "annis")
                            .map(|a| a.key);
                        self.annotations_for_matched_nodes
                            .entry(pos_in_match)
                            .or_default()
                            .extend(annos);
                    }
                }
            }
            if match_nr % 10 == 0 {
                if let Some(sender) = &self.progress {
                    let partial_progress = match_nr as f32 / matches.len() as f32;
                    sender
                        .send(FIRST_PASS_PROGRESS + (partial_progress * SINGLE_STEP_PROGRESS))
                        .await?;
                }
            }
        }
        Ok(())
    }

    async fn second_pass<W>(
        &self,
        matches: &BtreeIndex<u64, Vec<String>>,
        state: &GlobalAppState,
        session_id: &str,
        output: &mut W,
    ) -> Result<()>
    where
        W: std::io::Write,
    {
        let mut writer = csv::Writer::from_writer(output);
        // Create the header from the first entry
        if matches.contains_key(&0)? {
            let mut header = Vec::default();
            header.push("match number".to_string());
            for (m_nr, annos) in &self.annotations_for_matched_nodes {
                header.push(format!("{} node name", m_nr + 1));
                for anno_key in annos {
                    let anno_qname =
                        graphannis_core::util::join_qname(&anno_key.ns, &anno_key.name);
                    header.push(format!("{} {}", m_nr + 1, anno_qname));
                }
            }
            writer.write_record(header)?;
        }

        // Iterate over all matches
        for m in matches.range(..)? {
            let (idx, node_ids) = m?;
            if let Some(first_id) = node_ids.first() {
                let (corpus, _) = first_id.split_once('/').unwrap_or_default();
                // Get the subgraph for the IDs
                let g = corpora::subgraph(session_id, corpus, node_ids.clone(), None, 1, 1, state)
                    .await?;

                let mut record: Vec<String> = Vec::with_capacity(node_ids.len() + 1);
                // Output all columns for this match, first column is the match number
                record.push((idx + 1).to_string());
                for (m_nr, annos) in &self.annotations_for_matched_nodes {
                    // Each matched nodes contains the node ID
                    record.push(node_ids[*m_nr].clone());
                    if let Some(id) = g.get_node_id_from_name(&node_ids[*m_nr])? {
                        // Get the annotation values for this node
                        for anno_key in annos {
                            let value = g
                                .get_node_annos()
                                .get_value_for_item(&id, anno_key)?
                                .unwrap_or_default();
                            record.push(value.to_string());
                        }
                    }
                }
                writer.write_record(record)?;
            }
            if idx % 10 == 0 {
                if let Some(sender) = &self.progress {
                    let partial_progress = idx as f32 / matches.len() as f32;
                    sender
                        .send(SECOND_PASS_PROGRESS + (partial_progress * SINGLE_STEP_PROGRESS))
                        .await?;
                }
            }
        }
        Ok(())
    }
}
