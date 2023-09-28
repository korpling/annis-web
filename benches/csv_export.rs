use annis_web::{
    client::search::FindQuery,
    config::CliConfig,
    converter::{CSVConfig, CSVExporter},
    state::{GlobalAppState, SessionArg},
};
use criterion::{criterion_group, criterion_main, Criterion};
use graphannis::corpusstorage::ResultOrder;
use mockito::Server;

fn criterion_benchmark(c: &mut Criterion) {
    // Create a mock server that always returns the same result
    let mut backend = Server::new();
    let _find_mock = backend
        .mock("POST", "/search/find")
        .with_header("content-type", "text/plain")
        .with_body(
            r#"tiger::pos::pcc2/4282#tok_73 tiger::pos::pcc2/4282#tok_74
tiger::pos::pcc2/4282#tok_73 tiger::pos::pcc2/4282#tok_74
tiger::pos::pcc2/4282#tok_73 tiger::pos::pcc2/4282#tok_74
"#,
        )
        .create();

    let _subgraph_mock = backend
        .mock("POST", "/corpora/pcc2/subgraph")
        .with_body_from_file("tests/export-subgraph.graphml")
        .expect_at_least(3)
        .create();

    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("csv-export", |b| {
        b.to_async(&rt).iter(|| async move {
            let state = GlobalAppState::new(&CliConfig::default()).unwrap();

            let query = FindQuery {
                query: "pos=\"ART\" . pos=\"NN\"".into(),
                corpora: vec!["pcc2".into()],
                query_language: graphannis::corpusstorage::QueryLanguage::AQL,
                limit: None,
                order: ResultOrder::Normal,
            };
            let config = CSVConfig {
                span_segmentation: None,
                left_context: 0,
                right_context: 0,
            };
            let session_arg = SessionArg::Id(String::default());
            let mut string_buffer = Vec::new();

            let mut exporter = CSVExporter::new(query, config, None);
            exporter
                .convert_text(session_arg, &state, None, &mut string_buffer)
                .await
                .unwrap();
        });
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
