//! Each contract document validates against its versioned schema, and the
//! golden export parses into the engine's own types (which refuse unknown
//! fields), so Rails and Rust cannot drift apart silently.

use std::fs;
use std::path::PathBuf;

pub fn contracts_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("contracts")
}

pub fn read_json(name: &str) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(contracts_directory().join(name)).unwrap()).unwrap()
}

/// Every error message for `instance` against the schema in `schema_name`.
pub fn schema_errors(schema_name: &str, instance: &serde_json::Value) -> Vec<String> {
    let validator = jsonschema::validator_for(&read_json(schema_name)).unwrap();
    validator
        .iter_errors(instance)
        .map(|error| error.to_string())
        .collect()
}

#[test]
fn the_golden_export_matches_its_schema() {
    let errors = schema_errors(
        "engine-export.v2.schema.json",
        &read_json("engine-export.v2.json"),
    );
    assert!(errors.is_empty(), "{errors:#?}");
}

#[test]
fn the_export_schema_rejects_an_unknown_field() {
    let mut export = read_json("engine-export.v2.json");
    export["recipes"][0]["creator_name"] = "someone".into();
    assert_ne!(
        schema_errors("engine-export.v2.schema.json", &export),
        Vec::<String>::new()
    );
}

#[test]
fn the_v3_golden_export_matches_its_schema() {
    let errors = schema_errors(
        "engine-export.v3.schema.json",
        &read_json("engine-export.v3.json"),
    );
    assert!(errors.is_empty(), "{errors:#?}");
}

#[test]
fn the_v3_export_schema_rejects_an_unknown_field() {
    let mut export = read_json("engine-export.v3.json");
    export["items"][0]["creator_name"] = "someone".into();
    assert_ne!(
        schema_errors("engine-export.v3.schema.json", &export),
        Vec::<String>::new()
    );
}

#[test]
fn the_v3_golden_export_parses_into_the_engine_types() {
    let text = fs::read_to_string(contracts_directory().join("engine-export.v3.json")).unwrap();
    let (export, format) = corpus_cartographer::export::parse(&text).unwrap();
    assert_eq!(format, corpus_cartographer::export::Format::V3);
    assert_eq!((export.themes.len(), export.items.len()), (6, 7));
    assert_eq!(export.rejected_pairs, vec![[101, 102]]);
}

#[test]
fn a_v3_document_does_not_pass_the_v2_schema_nor_the_reverse() {
    assert_ne!(
        schema_errors(
            "engine-export.v2.schema.json",
            &read_json("engine-export.v3.json")
        ),
        Vec::<String>::new()
    );
    assert_ne!(
        schema_errors(
            "engine-export.v3.schema.json",
            &read_json("engine-export.v2.json")
        ),
        Vec::<String>::new()
    );
}

#[test]
fn an_unknown_version_names_both_accepted_ones() {
    for text in [
        r#"{"schema":"corpus-cartographer/engine-export","version":4}"#,
        r#"{"schema":"recipes-cartographer/engine-export","version":1}"#,
    ] {
        let message = corpus_cartographer::export::parse(text)
            .unwrap_err()
            .to_string();
        assert!(
            message.contains("version 3") && message.contains("version 2"),
            "{message}"
        );
    }
}

#[test]
fn an_empty_badge_is_refused_by_the_schema_and_by_the_parser() {
    let mut export = read_json("engine-export.v3.json");
    export["items"][0]["badges"] = serde_json::json!([""]);
    assert_ne!(
        schema_errors("engine-export.v3.schema.json", &export),
        Vec::<String>::new()
    );
    let message = corpus_cartographer::export::parse(&export.to_string())
        .unwrap_err()
        .to_string();
    assert!(message.contains("empty badge"), "{message}");
}

#[test]
fn the_golden_export_parses_into_the_engine_types() {
    let text = fs::read_to_string(contracts_directory().join("engine-export.v2.json")).unwrap();
    let (export, format) = corpus_cartographer::export::parse(&text).unwrap();
    assert_eq!(format, corpus_cartographer::export::Format::V2);
    assert_eq!((export.themes.len(), export.items.len()), (6, 7));
    assert_eq!(export.top_level_of(22), Some(2));
    assert_eq!(export.rejected_pairs, vec![[101, 102]]);
}

mod support;

fn golden_output() -> corpus_cartographer::EngineOutput {
    golden_output_of("engine-export.v2.json")
}

fn golden_output_of(export_name: &str) -> corpus_cartographer::EngineOutput {
    let input = fs::read_to_string(contracts_directory().join(export_name)).unwrap();
    corpus_cartographer::run(
        &input,
        &support::mini_embeddings(),
        &support::golden_options(),
    )
    .unwrap()
}

#[test]
fn every_output_matches_its_schema() {
    let output = golden_output();
    for (schema, document) in [
        ("engine-links.v2.schema.json", &output.links_json),
        ("map-spec.v1.schema.json", &output.map_spec_v1),
        ("map-spec.nested.schema.json", &output.map_spec_nested),
    ] {
        let errors = schema_errors(schema, &serde_json::from_str(document).unwrap());
        assert!(errors.is_empty(), "{schema}: {errors:#?}");
    }
}

#[test]
fn every_v3_output_matches_its_schema_and_names_items_with_the_item_prefix() {
    let output = golden_output_of("engine-export.v3.json");
    for (schema, document) in [
        ("engine-links.v3.schema.json", &output.links_json),
        ("map-spec.v1.schema.json", &output.map_spec_v1),
        ("map-spec.nested.schema.json", &output.map_spec_nested),
    ] {
        let errors = schema_errors(schema, &serde_json::from_str(document).unwrap());
        assert!(errors.is_empty(), "{schema}: {errors:#?}");
    }
    let spec: serde_json::Value = serde_json::from_str(&output.map_spec_nested).unwrap();
    for node in spec["nodes"].as_array().unwrap() {
        assert!(node["id"].as_str().unwrap().starts_with("item:"), "{node}");
    }
    let old: serde_json::Value = serde_json::from_str(&golden_output().map_spec_nested).unwrap();
    assert!(
        old["nodes"][0]["id"]
            .as_str()
            .unwrap()
            .starts_with("recipe:")
    );
}

#[test]
fn map_spec_v1_membership_agrees_both_ways() {
    // corpus-map's validateSpec refuses a spec where a node's territory and
    // the territory's members disagree.
    let spec: serde_json::Value = serde_json::from_str(&golden_output().map_spec_v1).unwrap();
    for node in spec["nodes"].as_array().unwrap() {
        let holders: Vec<&str> = spec["territories"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|territory| {
                territory["members"]
                    .as_array()
                    .unwrap()
                    .contains(&node["id"])
            })
            .map(|territory| territory["id"].as_str().unwrap())
            .collect();
        assert_eq!(
            holders,
            node["territory"].as_str().into_iter().collect::<Vec<_>>(),
            "{}",
            node["id"]
        );
    }
}

#[test]
fn map_spec_nested_follows_the_corpus_map_rules_a_schema_cannot_state() {
    // corpus-map's validateSpec: a parent lists no members (its members are
    // its children's), nodes belong to leaves and agree with the leaf's
    // members both ways, a parent has no parent, and edges join two
    // different existing nodes, at most once per pair.
    let spec: serde_json::Value = serde_json::from_str(&golden_output().map_spec_nested).unwrap();
    let territories = spec["territories"].as_array().unwrap();
    let parents: Vec<&str> = territories
        .iter()
        .filter_map(|territory| territory["parent"].as_str())
        .collect();
    assert!(!parents.is_empty(), "the golden export has sub-themes");
    for territory in territories {
        let id = territory["id"].as_str().unwrap();
        if parents.contains(&id) {
            assert_eq!(territory["members"], serde_json::json!([]), "{id}");
            assert!(territory.get("parent").is_none(), "{id} nests twice");
        }
    }
    for node in spec["nodes"].as_array().unwrap() {
        let holders: Vec<&str> = territories
            .iter()
            .filter(|territory| {
                territory["members"]
                    .as_array()
                    .unwrap()
                    .contains(&node["id"])
            })
            .map(|territory| territory["id"].as_str().unwrap())
            .collect();
        assert_eq!(
            holders,
            node["territory"].as_str().into_iter().collect::<Vec<_>>(),
            "{}",
            node["id"]
        );
    }
    let node_ids: Vec<&serde_json::Value> = spec["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| &node["id"])
        .collect();
    let mut pairs = std::collections::BTreeSet::new();
    let edges = spec["edges"].as_array().unwrap();
    assert!(!edges.is_empty(), "the golden export has links");
    for edge in edges {
        assert_ne!(edge["source"], edge["target"]);
        assert!(node_ids.contains(&&edge["source"]) && node_ids.contains(&&edge["target"]));
        let mut pair = [edge["source"].to_string(), edge["target"].to_string()];
        pair.sort();
        assert!(pairs.insert(pair), "{edge}");
    }
}
