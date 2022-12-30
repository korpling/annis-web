use transient_btree_index::BtreeIndex;

use crate::{
    client::{corpora, search},
    state::GlobalAppState,
    Result,
};

pub struct CSVExporter {
    aql: String,
}

impl CSVExporter {
    pub fn new<S: AsRef<str>>(aql: S) -> Self {
        Self {
            aql: String::from(aql.as_ref()),
        }
    }

    pub async fn convert_text<W: std::io::Write>(
        &self,
        state: &GlobalAppState,
        limit: Option<u64>,
        output: &mut W,
    ) -> Result<()> {
        // Get all the matches as Salt ID
        let result = search::find(&self.aql, vec!["pcc2".to_string()], limit, state).await?;

        self.first_pass(&result, state)?;
        self.second_pass(&result, output)?;
        Ok(())
    }

    fn first_pass(
        &self,
        matches: &BtreeIndex<u64, Vec<String>>,
        state: &GlobalAppState,
    ) -> Result<()> {
        for m in matches.range(..)? {
            let (idx, node_ids) = m?;
            // Get the corpus from the first node
            if let Some(id) = node_ids.first() {
                let (corpus, _) = id.split_once("/").unwrap_or_default();
                // Get the subgraph for the IDs
                let g = corpora::subgraph(corpus, node_ids.clone(), None, 0, 0, state);
                // Collect annotations for the matched nodes
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
