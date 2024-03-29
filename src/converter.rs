use graphannis_core::{
    annostorage::ValueSearch,
    graph::ANNIS_NS,
    types::{Component, NodeID},
};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr, NoneAsEmptyString};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    sync::Arc,
};
use tokio::sync::mpsc::Sender;

use graphannis::{
    graph::{AnnoKey, GraphStorage},
    model::AnnotationComponentType,
    AnnotationGraph,
};
use transient_btree_index::BtreeIndex;

use crate::{
    client::{
        corpora,
        search::{self, FindQuery},
    },
    state::{GlobalAppState, SessionArg},
    Result,
};

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CSVConfig {
    #[serde(default)]
    #[serde_as(as = "NoneAsEmptyString")]
    pub span_segmentation: Option<String>,
    #[serde(default)]
    #[serde_as(as = "DisplayFromStr")]
    pub left_context: usize,
    #[serde(default)]
    #[serde_as(as = "DisplayFromStr")]
    pub right_context: usize,
}

pub struct CSVExporter {
    query: FindQuery,
    config: CSVConfig,
    annotations_for_matched_nodes: BTreeMap<usize, BTreeSet<AnnoKey>>,
    gap_edges: bimap::BiHashMap<NodeID, NodeID>,
    subgraphs: BTreeMap<u64, AnnotationGraph>,
    progress: Option<Sender<f32>>,
}

const SINGLE_PASS_PROGRESS: f32 = 0.5;
const AFTER_FIRST_PASS_PROGRESS: f32 = SINGLE_PASS_PROGRESS;

impl CSVExporter {
    pub fn new(query: FindQuery, config: CSVConfig, progress: Option<Sender<f32>>) -> Self {
        Self {
            query,
            config,
            annotations_for_matched_nodes: BTreeMap::new(),
            progress,
            gap_edges: bimap::BiHashMap::new(),
            subgraphs: BTreeMap::new(),
        }
    }

    pub async fn convert_text<W: std::io::Write>(
        &mut self,
        session: SessionArg,
        state: &GlobalAppState,
        limit: Option<u64>,
        output: &mut W,
    ) -> Result<()> {
        // Get all the matches as Salt ID
        let mut query = self.query.clone();
        query.limit = limit;

        let result = search::find(&session, &query, state).await?;

        self.first_pass(&result, state, &session).await?;

        if let Some(progress) = &self.progress {
            progress.send(AFTER_FIRST_PASS_PROGRESS).await?;
        }
        self.second_pass(&result, output).await?;

        if let Some(progress) = &self.progress {
            progress.send(1.0).await?;
        }
        Ok(())
    }

    async fn first_pass(
        &mut self,
        matches: &BtreeIndex<u64, Vec<String>>,
        state: &GlobalAppState,
        session: &SessionArg,
    ) -> Result<()> {
        let datasource_gap_component = Component::new(
            AnnotationComponentType::Ordering,
            ANNIS_NS.into(),
            "datasource-gap".into(),
        );
        for m in matches.range(..)? {
            let (match_nr, node_ids) = m?;
            // Get the corpus from the first node
            if let Some(id) = node_ids.first() {
                let (corpus, _) = id.split_once('/').unwrap_or_default();
                // Get the subgraph for the IDs
                let g = corpora::subgraph(
                    session,
                    corpus,
                    node_ids.clone(),
                    self.config.span_segmentation.clone(),
                    self.config.left_context,
                    self.config.right_context,
                    state,
                )
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
                // Remember all datasource gaph edges
                if let Some(gs) = g.get_graphstorage_as_ref(&datasource_gap_component) {
                    for source in gs.source_nodes() {
                        let source = source?;
                        for target in gs.get_outgoing_edges(source) {
                            let target = target?;
                            self.gap_edges.insert(source, target);
                        }
                    }
                }
                self.subgraphs.insert(match_nr, g);
            }
            if match_nr % 10 == 0 {
                if let Some(sender) = &self.progress {
                    let partial_progress = match_nr as f32 / matches.len() as f32;
                    sender.send(partial_progress * SINGLE_PASS_PROGRESS).await?;
                }
            }
        }
        Ok(())
    }

    async fn second_pass<W>(
        &self,
        matches: &BtreeIndex<u64, Vec<String>>,
        output: &mut W,
    ) -> Result<()>
    where
        W: std::io::Write,
    {
        let mut writer = csv::Writer::from_writer(output);
        // Create the header from the first entry
        if matches.contains_key(&0)? {
            let mut header = Vec::default();
            header.push("text".to_string());
            for (m_nr, annos) in &self.annotations_for_matched_nodes {
                for anno_key in annos {
                    let anno_qname =
                        graphannis_core::util::join_qname(&anno_key.ns, &anno_key.name);
                    header.push(format!("{} ({})", anno_qname, m_nr + 1));
                }
            }
            writer.write_record(header)?;
        }

        // Iterate over all matches
        for m in matches.range(..)? {
            let (idx, node_ids) = m?;
            // Get the subgraph for the IDs
            if let Some(g) = self.subgraphs.get(&idx) {
                let mut record: Vec<String> = Vec::with_capacity(node_ids.len() + 1);
                // Output all columns for this match, first column is the matched text
                let text = self.get_spannd_text(g)?;
                record.push(text);
                for (m_nr, annos) in &self.annotations_for_matched_nodes {
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
                        .send(AFTER_FIRST_PASS_PROGRESS + (partial_progress * SINGLE_PASS_PROGRESS))
                        .await?;
                }
            }
        }
        Ok(())
    }

    fn get_spannd_text(&self, g: &AnnotationGraph) -> Result<String> {
        // Get ordering component that matches the configured segmentation
        let ordering_component = if let Some(seg) = &self.config.span_segmentation {
            Component::new(
                AnnotationComponentType::Ordering,
                "default_ns".into(),
                seg.into(),
            )
        } else {
            Component::new(
                AnnotationComponentType::Ordering,
                ANNIS_NS.into(),
                "".into(),
            )
        };

        let filtering_anno_key = self.config.span_segmentation.as_ref().map(|seg| AnnoKey {
            name: seg.into(),
            ns: "default_ns".into(),
        });

        let ordering_gs = g.get_graphstorage_as_ref(&ordering_component);
        let cov_edges: Vec<Arc<dyn GraphStorage>> = g
            .get_all_components(Some(AnnotationComponentType::Coverage), None)
            .into_iter()
            .filter_map(|c| g.get_graphstorage(&c))
            .filter(|gs| {
                if let Some(stats) = gs.get_statistics() {
                    stats.nodes > 0
                } else {
                    true
                }
            })
            .collect();

        let mut roots: HashSet<_> = HashSet::new();
        for n in g
            .get_node_annos()
            .exact_anno_search(Some(ANNIS_NS), "tok", ValueSearch::Any)
        {
            let n = n?;

            let has_anno = if let Some(filter) = &filtering_anno_key {
                // For segmentation search, only include the nodes that have a matching annotation
                g.get_node_annos()
                    .get_value_for_item(&n.node, filter)?
                    .is_some()
            } else {
                // Check that this is an actual token and there are no outgoing coverage edges
                let mut actual_token = true;
                for c in cov_edges.iter() {
                    if c.has_outgoing_edges(n.node)? {
                        actual_token = false;
                        break;
                    }
                }
                actual_token
            };

            if has_anno
                && (ordering_gs.is_none()
                    || ordering_gs.is_some_and(|gs| gs.get_ingoing_edges(n.node).next().is_none()))
            {
                roots.insert(n.node);
            }
        }

        // Order the roots in the overall text position by using the
        // explicit gap edges. First find the root node that has no incoming
        // gap, than follow the ordering and gap edges and construct the
        // text in between.
        let mut result = String::new();
        let mut token = roots
            .into_iter()
            .find(|r| !self.gap_edges.contains_right(r));
        let token_value_key = AnnoKey {
            ns: ANNIS_NS.into(),
            name: "tok".into(),
        };
        let whitespace_before_key = AnnoKey {
            ns: ANNIS_NS.into(),
            name: "tok-whitespace-before".into(),
        };
        let whitespace_after_key = AnnoKey {
            ns: ANNIS_NS.into(),
            name: "tok-whitespace-after".into(),
        };

        let mut is_first_token = true;

        while let Some(current_token) = token {
            // Add prefix whitespace only for first token
            if is_first_token {
                if let Some(val) = g
                    .get_node_annos()
                    .get_value_for_item(&current_token, &whitespace_before_key)?
                {
                    result.push_str(&val);
                }
            }

            if let Some(val) = g
                .get_node_annos()
                .get_value_for_item(&current_token, &token_value_key)?
            {
                result.push_str(&val);
            }

            is_first_token = false;

            // Try to get the outgoing ordering edge first
            token = if let Some(ordering_gs) = ordering_gs {
                if let Some(next_token) = ordering_gs.get_outgoing_edges(current_token).next() {
                    let next_token = next_token?;
                    Some(next_token)
                } else if let Some(next_token) = self.gap_edges.get_by_left(&current_token) {
                    result.push_str("(...) ");
                    Some(*next_token)
                } else {
                    None
                }
            } else {
                None
            };

            // Add postfix whitespace (but not for the last token)
            if token.is_some() {
                if let Some(val) = g
                    .get_node_annos()
                    .get_value_for_item(&current_token, &whitespace_after_key)?
                {
                    result.push_str(&val);
                } else if self.config.span_segmentation.is_some() {
                    // Use a space character as default seperation character
                    result.push(' ');
                }
            }
        }

        Ok(result)
    }
}
