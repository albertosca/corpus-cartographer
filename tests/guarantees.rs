//! The spec's engine guarantees: byte determinism, territory identity bound
//! to the theme, the stability bound, and the edge corpora.

mod support;

use corpus_cartographer::links::LinkOptions;
use serde_json::{Value, json};

/// A synthetic corpus: three themes of two sub-themes, four recipes each,
/// two mini-Epicure terms per recipe chosen by position.
fn corpus(extra_recipe: bool, previous: &Value) -> Value {
    let terms = support::MINI_TERMS;
    let mut themes = Vec::new();
    let mut recipes = Vec::new();
    for theme in 1..=3_u64 {
        themes.push(json!({"id": theme, "parent_id": null, "name": format!("Theme {theme}"), "epicure_terms": []}));
        for subtheme_number in 1..=2_u64 {
            let subtheme = theme * 10 + subtheme_number;
            themes.push(json!({"id": subtheme, "parent_id": theme, "name": format!("Sub {subtheme}"), "epicure_terms": []}));
            for position in 0..4_u64 {
                let id = subtheme * 100 + position;
                let first = terms[((theme * 2 + subtheme_number) as usize) % terms.len()];
                let second = terms[((theme + position) as usize) % terms.len()];
                recipes.push(json!({"id": id, "title": format!("Synthetic {id}"), "subtheme_id": subtheme, "status": "assigned", "seal": "complete",
                    "ingredients": [{"name": first, "epicure_term": first}, {"name": second, "epicure_term": second}]}));
            }
        }
    }
    if extra_recipe {
        recipes.push(json!({"id": 9999, "title": "Synthetic new", "subtheme_id": 11, "status": "assigned", "seal": "complete",
            "ingredients": [{"name": "garlic", "epicure_term": "garlic"}]}));
    }
    json!({"schema": "recipes-cartographer/engine-export", "version": 2, "taxonomy_version": 1,
        "themes": themes, "recipes": recipes, "previous": previous, "rejected_pairs": []})
}

fn run(input: &Value) -> corpus_cartographer::EngineOutput {
    corpus_cartographer::run(
        &input.to_string(),
        &support::mini_epicure(),
        &LinkOptions::default(),
    )
    .unwrap()
}

fn positions(output: &corpus_cartographer::EngineOutput) -> Value {
    serde_json::from_str::<Value>(&output.links_json).unwrap()["positions"].clone()
}

#[test]
fn the_same_input_gives_the_same_bytes() {
    let input = corpus(false, &Value::Null);
    let (first, second) = (run(&input), run(&input));
    assert_eq!(first.links_json, second.links_json);
    assert_eq!(first.map_spec_v1, second.map_spec_v1);
    assert_eq!(first.map_spec_nested, second.map_spec_nested);
}

#[test]
fn territory_identity_does_not_depend_on_input_order() {
    let input = corpus(false, &Value::Null);
    let mut shuffled = input.clone();
    for key in ["themes", "recipes"] {
        shuffled[key].as_array_mut().unwrap().reverse();
    }
    for recipe in shuffled["recipes"].as_array_mut().unwrap() {
        recipe["ingredients"].as_array_mut().unwrap().reverse();
    }
    let input_hash_free = |output: &corpus_cartographer::EngineOutput| {
        let mut document: Value = serde_json::from_str(&output.links_json).unwrap();
        document["input_sha256"] = Value::Null;
        document
    };
    let (ordered, reordered) = (run(&input), run(&shuffled));
    assert_eq!(input_hash_free(&ordered), input_hash_free(&reordered));
    assert_eq!(ordered.map_spec_nested, reordered.map_spec_nested);
}

/// Measured on this corpus as 0.048744 on 2026-10-17 and, once flavours were
/// weighted and centred on the corpus (engine 0.3.0), as 0.079999 on
/// 2026-11-01: one recipe in 24 now moves every weight and the mean. The
/// bound is that, rounded up to the next 0.05. On the real corpus the same
/// change took the largest move from 0.0104 to 0.0267 (map-engine spec,
/// open question 2).
const STABILITY_BOUND: f64 = 0.10;

#[test]
fn one_new_recipe_moves_untouched_territories_less_than_the_bound() {
    let first = run(&corpus(false, &Value::Null));
    let before = positions(&first);
    let second = run(&corpus(true, &before));
    let after = positions(&second);

    let point = |positions: &Value, id: &str| {
        [
            positions["territories"][id][0].as_f64().unwrap(),
            positions["territories"][id][1].as_f64().unwrap(),
        ]
    };
    let extent = ["1", "2", "3"]
        .iter()
        .flat_map(|origin| ["1", "2", "3"].map(|destination| (*origin, destination)))
        .map(|(origin, destination)| {
            (point(&before, origin)[0] - point(&before, destination)[0])
                .hypot(point(&before, origin)[1] - point(&before, destination)[1])
        })
        .fold(0.0, f64::max);
    // Territory 1 gained the recipe; 2 and 3 did not.
    let largest_move = ["2", "3"]
        .iter()
        .map(|id| {
            (point(&before, id)[0] - point(&after, id)[0])
                .hypot(point(&before, id)[1] - point(&after, id)[1])
        })
        .fold(0.0, f64::max);
    let relative = largest_move / extent;
    eprintln!("stability: largest move {largest_move:.6} over extent {extent:.6} = {relative:.6}");
    assert!(relative < STABILITY_BOUND, "{relative}");
}

#[test]
fn edge_corpora_produce_valid_outputs() {
    let empty = json!({"schema": "recipes-cartographer/engine-export", "version": 2, "taxonomy_version": null, "themes": [], "recipes": [], "previous": null, "rejected_pairs": []});
    let mut one = corpus(false, &Value::Null);
    one["recipes"].as_array_mut().unwrap().truncate(1);
    let mut all_waiting = corpus(false, &Value::Null);
    for recipe in all_waiting["recipes"].as_array_mut().unwrap() {
        recipe["status"] = "needs_owner".into();
        recipe["subtheme_id"] = Value::Null;
    }
    let mut no_vectors = corpus(false, &Value::Null);
    for recipe in no_vectors["recipes"].as_array_mut().unwrap() {
        recipe["ingredients"] = json!([]);
        recipe["seal"] = "none".into();
    }
    for input in [empty, one, all_waiting, no_vectors] {
        let output = run(&input);
        let document: Value = serde_json::from_str(&output.links_json).unwrap();
        assert_eq!(
            document["counts"]["recipes"],
            input["recipes"].as_array().unwrap().len()
        );
    }
}
