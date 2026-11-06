//! The same corpus written as v2 and as v3 must give the same links.
mod support;
use std::fs;
use std::path::PathBuf;

fn run_file(name: &str) -> serde_json::Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("contracts")
        .join(name);
    let input = fs::read_to_string(path).unwrap();
    let output = corpus_cartographer::run(
        &input,
        &support::mini_embeddings(),
        &support::golden_options(),
    )
    .unwrap();
    serde_json::from_str(&output.links_json).unwrap()
}

#[test]
fn v2_and_v3_of_one_corpus_give_the_same_links_and_positions() {
    let old = run_file("engine-export.v2.json");
    let new = run_file("engine-export.v3.json");
    assert_eq!(old["version"], 2);
    assert_eq!(new["version"], 3);
    for key in ["positions", "warnings", "options"] {
        assert_eq!(old[key], new[key], "{key}");
    }
    let pairs = |document: &serde_json::Value| -> Vec<(u64, u64, f64, bool)> {
        document["links"]
            .as_array()
            .unwrap()
            .iter()
            .map(|link| {
                (
                    link["source"].as_u64().unwrap(),
                    link["target"].as_u64().unwrap(),
                    link["similarity"].as_f64().unwrap(),
                    link["crosses_theme"].as_bool().unwrap(),
                )
            })
            .collect()
    };
    assert_eq!(pairs(&old), pairs(&new));
    assert_ne!(pairs(&new), Vec::new(), "the golden corpus has links");
    assert_eq!(old["counts"]["recipes"], new["counts"]["items"]);
}
