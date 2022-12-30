use std::collections::{HashMap, HashSet};

use graphannis::graph::AnnoKey;
use transient_btree_index::BtreeIndex;

use crate::{
    client::{corpora, search},
    state::GlobalAppState,
    Result,
};

pub struct CSVExporter {
    aql: String,
    annotations_for_matched_nodes: HashMap<usize, HashSet<AnnoKey>>,
}

impl CSVExporter {
    pub fn new<S: AsRef<str>>(aql: S) -> Self {
        Self {
            aql: String::from(aql.as_ref()),
            annotations_for_matched_nodes: HashMap::new(),
        }
    }

    pub async fn convert_text<W: std::io::Write>(
        &mut self,
        state: &GlobalAppState,
        limit: Option<u64>,
        output: &mut W,
    ) -> Result<()> {
        // Get all the matches as Salt ID
        let result = search::find(&self.aql, vec!["pcc2".to_string()], limit, state).await?;

        self.first_pass(&result, state).await?;
        self.second_pass(&result, output)?;
        Ok(())
    }

    async fn first_pass(
        &mut self,
        matches: &BtreeIndex<u64, Vec<String>>,
        state: &GlobalAppState,
    ) -> Result<()> {
        for m in matches.range(..)? {
            let (match_nr, node_ids) = m?;
            // Get the corpus from the first node
            if let Some(id) = node_ids.first() {
                let (corpus, _) = id.split_once("/").unwrap_or_default();
                // Get the subgraph for the IDs
                let g = corpora::subgraph(corpus, node_ids.clone(), None, 0, 0, state).await?;
                // Collect annotations for the matched nodes
                for (pos_in_match, node_name) in node_ids.iter().enumerate() {
                    if let Some(n_id) = g.get_node_id_from_name(&node_name)? {
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
        }
        Ok(())
    }

    fn second_pass<W>(&self, matches: &BtreeIndex<u64, Vec<String>>, output: &mut W) -> Result<()>
    where
        W: std::io::Write,
    {
        let mut writer = csv::Writer::from_writer(output);
        // TODO: actually produce the CSV, just output the node IDs for node
        if let Some(first_id) = matches.get(&0)? {
            let mut header = Vec::with_capacity(first_id.len() + 1);
            header.push("match_number".to_string());
            for i in 0..first_id.len() {
                header.push(format!("{}_node_name", i + 1));
            }
            writer.write_record(header)?;
        }
        for m in matches.range(..)? {
            let (idx, node_ids) = m?;
            let mut record = Vec::with_capacity(node_ids.len() + 1);
            record.push((idx + 1).to_string());
            for n in node_ids {
                record.push(n);
            }
            writer.write_record(record)?;
        }
        Ok(())
    }
}
