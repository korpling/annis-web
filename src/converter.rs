use crate::{client::search, state::GlobalAppState, Result};

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
        output: &mut W,
    ) -> Result<()> {
        // Get all the matches as Salt ID
        let result = search::find(&self.aql, vec!["pcc2".to_string()], Some(2), state).await?;

        // TODO: actually convert the text, for now just output the original result
        for m in result {
            writeln!(output, "{}", m)?;
        }
        Ok(())
    }
}
