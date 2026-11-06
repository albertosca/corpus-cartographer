//! Pins the wire names the other tests only reach through goldens that have
//! no badges and no `closest` pair: the parser's unknown-field refusal at
//! every level, badges on map nodes, and the names of a closest pair.
mod support;

use std::fs;
use std::path::PathBuf;

const THEMES_V3: &str = r#""themes":[{"id":1,"parent_id":null,"name":"A","terms":[]},{"id":11,"parent_id":1,"name":"A1","terms":[]}]"#;

fn v3_export(extra_item_field: &str) -> String {
    format!(
        r#"{{"schema":"corpus-cartographer/engine-export","version":3,"taxonomy_version":1,{THEMES_V3},
        "items":[
         {{"id":101,"title":"One","subtheme_id":11,"status":"assigned","badges":["marked"],{extra_item_field}"terms":[{{"label":"Garlic","term":"garlic"}}]}},
         {{"id":102,"title":"Two","subtheme_id":11,"status":"assigned","badges":[],"terms":[{{"label":"Onion","term":"onion"}}]}},
         {{"id":103,"title":"Three","subtheme_id":11,"status":"assigned","badges":[],"terms":[{{"label":"Flour","term":"flour"}}]}}],
        "previous":null,"rejected_pairs":[]}}"#
    )
}

const V2_EXPORT: &str = r#"{"schema":"recipes-cartographer/engine-export","version":2,"taxonomy_version":1,
 "themes":[{"id":1,"parent_id":null,"name":"A","epicure_terms":[]},{"id":11,"parent_id":1,"name":"A1","epicure_terms":[]}],
 "recipes":[
  {"id":101,"title":"One","subtheme_id":11,"status":"assigned","seal":"none","ingredients":[{"name":"Garlic","epicure_term":"garlic"}]},
  {"id":102,"title":"Two","subtheme_id":11,"status":"assigned","seal":"complete","ingredients":[{"name":"Onion","epicure_term":"onion"}]},
  {"id":103,"title":"Three","subtheme_id":11,"status":"assigned","seal":"complete","ingredients":[{"name":"Flour","epicure_term":"flour"}]}],
 "previous":null,"rejected_pairs":[]}"#;

fn run(text: &str) -> (serde_json::Value, serde_json::Value) {
    let output = corpus_cartographer::run(
        text,
        &support::mini_embeddings(),
        &support::golden_options(),
    )
    .unwrap();
    (
        serde_json::from_str(&output.links_json).unwrap(),
        serde_json::from_str(&output.map_spec_nested).unwrap(),
    )
}

fn schema_errors(schema_name: &str, instance: &serde_json::Value) -> Vec<String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("contracts")
        .join(schema_name);
    let schema: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    jsonschema::validator_for(&schema)
        .unwrap()
        .iter_errors(instance)
        .map(|error| error.to_string())
        .collect()
}

#[test]
fn the_parser_refuses_an_unknown_field_at_every_level_and_names_it() {
    let good = v3_export("");
    assert!(corpus_cartographer::export::parse(&good).is_ok());
    let cases = [
        (
            "extra_top",
            good.replacen("\"version\":3,", "\"version\":3,\"extra_top\":1,", 1),
        ),
        ("extra_item", v3_export("\"extra_item\":1,")),
        (
            "extra_term",
            good.replacen(
                "\"label\":\"Garlic\"",
                "\"extra_term\":1,\"label\":\"Garlic\"",
                1,
            ),
        ),
        (
            "extra_theme",
            good.replacen("\"name\":\"A\"", "\"extra_theme\":1,\"name\":\"A\"", 1),
        ),
        (
            "extra_prev",
            good.replace(
                "\"previous\":null",
                "\"previous\":{\"territories\":{},\"subthemes\":{},\"extra_prev\":1}",
            ),
        ),
    ];
    for (field, text) in cases {
        let message = corpus_cartographer::export::parse(&text)
            .expect_err(field)
            .to_string();
        assert!(message.contains(field), "{field}: {message}");
    }
}

#[test]
fn badges_reach_the_map_nodes_in_both_formats() {
    let (_, v3_spec) = run(&v3_export(""));
    assert_eq!(v3_spec["nodes"][0]["badges"], serde_json::json!(["marked"]));
    assert!(v3_spec["nodes"][1].get("badges").is_none());
    let (_, v2_spec) = run(V2_EXPORT);
    assert_eq!(
        v2_spec["nodes"][0]["badges"],
        serde_json::json!(["sem receita"])
    );
    assert!(v2_spec["nodes"][1].get("badges").is_none());
}

#[test]
fn a_closest_pair_has_the_wire_names_of_its_format_and_passes_its_schema() {
    for (text, schema, source, target) in [
        (
            v3_export(""),
            "engine-links.v3.schema.json",
            "source_label",
            "target_label",
        ),
        (
            V2_EXPORT.to_string(),
            "engine-links.v2.schema.json",
            "source_name",
            "target_name",
        ),
    ] {
        let (links, _) = run(&text);
        let closest: Vec<&serde_json::Value> = links["links"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|link| link.get("closest"))
            .collect();
        assert_ne!(
            closest,
            Vec::<&serde_json::Value>::new(),
            "{schema} has no closest pair"
        );
        assert!(
            closest[0].get(source).is_some() && closest[0].get(target).is_some(),
            "{schema}"
        );
        let errors = schema_errors(schema, &links);
        assert!(errors.is_empty(), "{schema}: {errors:#?}");
    }
}
