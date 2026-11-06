//! The engine on a corpus with no embeddings: every term gets its own axis,
//! so a map needs nothing but the export.

mod support;

use std::fmt::Write;

use corpus_cartographer::embeddings::Embeddings;
use corpus_cartographer::export;

const THEMES: &str = r#"[
  {"id":1,"parent_id":null,"name":"A","epicure_terms":[]},
  {"id":2,"parent_id":null,"name":"B","epicure_terms":[]},
  {"id":11,"parent_id":1,"name":"A1","epicure_terms":[]},
  {"id":21,"parent_id":2,"name":"B1","epicure_terms":[]}
]"#;

/// An export whose items are `(id, subtheme id, terms)`.
fn export_of(items: &[(u64, u64, Vec<&str>)]) -> String {
    let recipes: Vec<String> = items
        .iter()
        .map(|(id, subtheme_id, terms)| {
            let ingredients: Vec<String> = terms
                .iter()
                .map(|term| format!(r#"{{"name":"name of {term}","epicure_term":"{term}"}}"#))
                .collect();
            format!(
                r#"{{"id":{id},"title":"Item {id}","subtheme_id":{subtheme_id},"status":"assigned","seal":"complete","ingredients":[{}]}}"#,
                ingredients.join(",")
            )
        })
        .collect();
    format!(
        r#"{{"schema":"recipes-cartographer/engine-export","version":2,"taxonomy_version":1,"themes":{THEMES},"recipes":[{}],"previous":null,"rejected_pairs":[]}}"#,
        recipes.join(",")
    )
}

fn run(items: &[(u64, u64, Vec<&str>)]) -> corpus_cartographer::EngineOutput {
    corpus_cartographer::run_without_embeddings(&export_of(items), &support::golden_options())
        .unwrap()
}

#[test]
fn an_item_with_no_terms_is_placed_without_a_vector_or_links() {
    let output = run(&[
        (1, 11, vec!["a", "b"]),
        (2, 11, vec!["a", "c"]),
        (3, 21, vec!["d", "e"]),
        (4, 21, vec![]),
    ]);
    assert!(output.summary.contains("recipes=4"), "{}", output.summary);
    assert!(
        output.summary.contains("with_vector=3"),
        "{}",
        output.summary
    );
    let links: serde_json::Value = serde_json::from_str(&output.links_json).unwrap();
    let mentions_four = links["links"]
        .as_array()
        .unwrap()
        .iter()
        .any(|link| link["source"] == 4 || link["target"] == 4);
    assert!(!mentions_four, "{}", output.links_json);
}

#[test]
fn items_sharing_every_term_get_real_coordinates_and_link_at_one() {
    let output = run(&[
        (1, 11, vec!["a", "b"]),
        (2, 11, vec!["a", "b"]),
        (3, 21, vec!["a", "b"]),
    ]);
    // serde_json writes a non-finite number as null, so check the numbers.
    let spec: serde_json::Value = serde_json::from_str(&output.map_spec_v1).unwrap();
    for node in spec["nodes"].as_array().unwrap() {
        assert!(node["x"].is_f64() && node["y"].is_f64(), "{node}");
    }
    // A flavour equal to the corpus mean keeps its raw direction, so
    // identical items stay linked (cosine 1), as recipes do today.
    assert!(output.summary.contains("links=6"), "{}", output.summary);
}

#[test]
fn a_term_used_by_exactly_one_item_still_gives_it_a_vector() {
    let output = run(&[
        (1, 11, vec!["only-here"]),
        (2, 11, vec!["a", "b"]),
        (3, 21, vec!["b", "c"]),
    ]);
    assert!(
        output.summary.contains("with_vector=3"),
        "{}",
        output.summary
    );
}

#[test]
fn a_term_repeated_inside_one_item_counts_once() {
    let once = run(&[
        (1, 11, vec!["a", "b"]),
        (2, 11, vec!["b", "c"]),
        (3, 21, vec!["c", "d"]),
    ]);
    let repeated = run(&[
        (1, 11, vec!["a", "a", "b"]),
        (2, 11, vec!["b", "c"]),
        (3, 21, vec!["c", "d"]),
    ]);
    assert_eq!(once.map_spec_v1, repeated.map_spec_v1);
}

#[test]
fn terms_that_differ_only_in_case_are_different_terms() {
    let output = run(&[
        (1, 11, vec!["Trail"]),
        (2, 11, vec!["trail"]),
        (3, 21, vec!["trail", "Trail"]),
    ]);
    assert!(
        output.summary.contains("with_vector=3"),
        "{}",
        output.summary
    );
    let links: serde_json::Value = serde_json::from_str(&output.links_json).unwrap();
    let pair_one_two = links["links"].as_array().unwrap().iter().any(|link| {
        (link["source"] == 1 && link["target"] == 2) || (link["source"] == 2 && link["target"] == 1)
    });
    assert!(!pair_one_two, "{}", output.links_json);
}

#[test]
fn a_corpus_with_no_terms_at_all_still_runs() {
    let output = run(&[(1, 11, vec![]), (2, 21, vec![])]);
    assert!(
        output.summary.contains("with_vector=0"),
        "{}",
        output.summary
    );
    assert!(output.summary.contains("links=0"), "{}", output.summary);
}

#[test]
fn no_link_carries_a_closest_term_because_there_are_no_embeddings() {
    // A floor of -1 links every pair, including pairs that share no term,
    // which is where a closest pair of terms would otherwise be offered.
    let options = corpus_cartographer::links::LinkOptions {
        max_links: 10,
        floor: -1.0,
    };
    let items = [
        (1, 11, vec!["a", "b"]),
        (2, 11, vec!["a", "c"]),
        (3, 21, vec!["d"]),
        (4, 21, vec!["e", "d"]),
    ];
    let output = corpus_cartographer::run_without_embeddings(&export_of(&items), &options).unwrap();
    let links: serde_json::Value = serde_json::from_str(&output.links_json).unwrap();
    let without_shared_terms: Vec<&serde_json::Value> = links["links"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|link| link["shared"].as_array().is_some_and(Vec::is_empty))
        .collect();
    assert!(!without_shared_terms.is_empty(), "{}", output.links_json);
    for link in without_shared_terms {
        assert!(link["closest"].is_null(), "{link}");
    }
}

#[test]
fn a_corpus_too_big_for_dense_axes_is_refused_with_both_numbers() {
    // 5001 distinct terms over 5001 items: 5001 x 5001 cells is over the limit.
    let terms: Vec<String> = (0..5001).map(|index| format!("term-{index}")).collect();
    let mut recipes = String::new();
    for (index, term) in terms.iter().enumerate() {
        if index > 0 {
            recipes.push(',');
        }
        write!(
            recipes,
            r#"{{"id":{id},"title":"Item","subtheme_id":11,"status":"assigned","seal":"complete","ingredients":[{{"name":"n","epicure_term":"{term}"}}]}}"#,
            id = index + 1
        )
        .unwrap();
    }
    let text = format!(
        r#"{{"schema":"recipes-cartographer/engine-export","version":2,"taxonomy_version":1,"themes":{THEMES},"recipes":[{recipes}],"previous":null,"rejected_pairs":[]}}"#
    );
    let error = corpus_cartographer::run_without_embeddings(&text, &support::golden_options())
        .err()
        .expect("the guard must fire");
    let message = error.to_string();
    assert!(!message.starts_with("input"), "{message}");
    assert!(message.contains("5001 terms"), "{message}");
    assert!(message.contains("5001 items"), "{message}");
    assert!(message.contains("embeddings"), "{message}");
}

#[test]
fn identity_axes_agree_with_an_explicit_identity_matrix_on_the_golden_export() {
    let text = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("contracts/engine-export.v2.json"),
    )
    .unwrap();
    let terms: Vec<String> = export::parse(&text)
        .unwrap()
        .0
        .terms()
        .into_iter()
        .collect();
    let dimension = terms.len();
    let mut values = vec![0.0_f32; dimension * dimension];
    for index in 0..dimension {
        values[index * dimension + index] = 1.0;
    }
    let explicit = Embeddings::from_rows(terms, values, dimension);
    let dense = corpus_cartographer::run(&text, &explicit, &support::golden_options()).unwrap();
    let generic =
        corpus_cartographer::run_without_embeddings(&text, &support::golden_options()).unwrap();
    assert_eq!(dense.map_spec_v1, generic.map_spec_v1);
    assert_eq!(dense.summary, generic.summary);
}

fn example_text(name: &str) -> String {
    std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("examples")
            .join(name),
    )
    .unwrap()
}

fn assert_valid_map(name: &str, export_schema: &str, links_schema: &str, noun: &str) {
    let text = example_text(name);
    let output =
        corpus_cartographer::run_without_embeddings(&text, &support::golden_options()).unwrap();
    assert!(
        output.summary.contains(&format!("{noun}=15")),
        "{}",
        output.summary
    );
    assert!(
        output.summary.contains("with_vector=14"),
        "{}",
        output.summary
    );
    let contracts = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("contracts");
    for (schema, document) in [
        (export_schema, text.as_str()),
        (links_schema, &output.links_json),
        ("map-spec.v1.schema.json", &output.map_spec_v1),
        ("map-spec.nested.schema.json", &output.map_spec_nested),
    ] {
        let validator = jsonschema::validator_for(
            &serde_json::from_str(&std::fs::read_to_string(contracts.join(schema)).unwrap())
                .unwrap(),
        )
        .unwrap();
        let instance: serde_json::Value = serde_json::from_str(document).unwrap();
        let errors: Vec<String> = validator
            .iter_errors(&instance)
            .map(|error| error.to_string())
            .collect();
        assert!(errors.is_empty(), "{schema}: {errors:#?}");
    }
}

#[test]
fn the_hiking_trails_example_makes_a_valid_map() {
    assert_valid_map(
        "hiking-trails.json",
        "engine-export.v3.schema.json",
        "engine-links.v3.schema.json",
        "items",
    );
}

#[test]
fn the_v2_hiking_trails_witness_still_makes_a_valid_map() {
    assert_valid_map(
        "hiking-trails.v2.json",
        "engine-export.v2.schema.json",
        "engine-links.v2.schema.json",
        "recipes",
    );
}

#[test]
fn the_v2_and_v3_trails_link_the_same_pairs() {
    let pairs = |name: &str| -> Vec<(u64, u64)> {
        let output = corpus_cartographer::run_without_embeddings(
            &example_text(name),
            &support::golden_options(),
        )
        .unwrap();
        let document: serde_json::Value = serde_json::from_str(&output.links_json).unwrap();
        document["links"]
            .as_array()
            .unwrap()
            .iter()
            .map(|link| {
                (
                    link["source"].as_u64().unwrap(),
                    link["target"].as_u64().unwrap(),
                )
            })
            .collect()
    };
    let new = pairs("hiking-trails.json");
    assert_ne!(new, Vec::new());
    assert_eq!(pairs("hiking-trails.v2.json"), new);
}

#[test]
fn the_hiking_trails_map_matches_its_golden() {
    let directory = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(directory.join("examples/hiking-trails.json")).unwrap();
    let output =
        corpus_cartographer::run_without_embeddings(&text, &support::golden_options()).unwrap();
    let path = directory.join("tests/golden/hiking-trails.map-spec.v1.json");
    if std::env::var("UPDATE_GOLDEN").as_deref() == Ok("1") {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &output.map_spec_v1).unwrap();
    }
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        output.map_spec_v1,
        "rerun with UPDATE_GOLDEN=1 and review the diff"
    );
}
