//! contracts/engine-links.v2.json is what the engine writes for the golden
//! export; Rails imports it in its own contract test. Regenerate with
//! `UPDATE_GOLDEN=1` and read the diff before committing it.

mod support;

use std::fs;
use std::path::PathBuf;

use corpus_cartographer::links::LinkOptions;

fn contracts_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("contracts")
}

fn output() -> corpus_cartographer::EngineOutput {
    output_of("engine-export.v2.json")
}

fn output_of(export_name: &str) -> corpus_cartographer::EngineOutput {
    let input = fs::read_to_string(contracts_directory().join(export_name)).unwrap();
    corpus_cartographer::run(
        &input,
        &support::mini_embeddings(),
        &support::golden_options(),
    )
    .unwrap()
}

#[test]
fn the_links_document_matches_the_golden() {
    let path = contracts_directory().join("engine-links.v2.json");
    let output = output();
    if std::env::var("UPDATE_GOLDEN").as_deref() == Ok("1") {
        fs::write(&path, &output.links_json).unwrap();
    }
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        output.links_json,
        "rerun with UPDATE_GOLDEN=1 and review the diff"
    );
}

#[test]
fn the_v3_links_document_matches_the_golden() {
    let path = contracts_directory().join("engine-links.v3.json");
    let output = output_of("engine-export.v3.json");
    if std::env::var("UPDATE_GOLDEN").as_deref() == Ok("1") {
        fs::write(&path, &output.links_json).unwrap();
    }
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        output.links_json,
        "rerun with UPDATE_GOLDEN=1 and review the diff"
    );
}

#[test]
fn the_golden_links_only_join_flavoured_assigned_recipes() {
    let document: serde_json::Value = serde_json::from_str(&output().links_json).unwrap();
    let links = document["links"].as_array().unwrap();
    assert_ne!(links, &Vec::<serde_json::Value>::new());
    for link in links {
        for end in ["source", "target"] {
            let id = link[end].as_u64().unwrap();
            assert!((101..=104).contains(&id), "{end} {id} should have no links");
        }
    }
    assert_eq!(document["counts"]["waiting"], 2);
    assert!(document["positions"]["territories"]["1"].is_array());
}

#[test]
fn the_golden_rejected_pair_is_never_linked() {
    let document: serde_json::Value = serde_json::from_str(&output().links_json).unwrap();
    for link in document["links"].as_array().unwrap() {
        let pair = [
            link["source"].as_u64().unwrap(),
            link["target"].as_u64().unwrap(),
        ];
        assert!(pair != [101, 102] && pair != [102, 101], "{pair:?}");
    }
}

#[test]
fn the_links_document_records_the_options_it_ran_with() {
    let input = fs::read_to_string(contracts_directory().join("engine-export.v2.json")).unwrap();
    let output = corpus_cartographer::run(
        &input,
        &support::mini_embeddings(),
        &LinkOptions {
            max_links: 1,
            floor: 0.7,
        },
    )
    .unwrap();
    let document: serde_json::Value = serde_json::from_str(&output.links_json).unwrap();
    assert_eq!(
        document["options"],
        serde_json::json!({"max_links": 1, "floor": 0.7})
    );
}
