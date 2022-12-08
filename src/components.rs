use askama::Template;

#[derive(Template)]
#[template(path = "components/corpus_selector.html")]
pub struct CorpusSelectorComponent {
    pub id: String,
    pub url_prefix: String,
    pub corpus_names: Vec<String>,
}
