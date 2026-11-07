//! The spec's engine guarantees: byte determinism, territory identity bound
//! to the theme, the stability bound, and the edge corpora.

mod support;

use corpus_cartographer::links::LinkOptions;
use serde_json::{Value, json};

/// The two wire formats the engine reads; every guarantee holds in both.
#[derive(Clone, Copy, Debug)]
enum Wire {
    V3,
    V2,
}

const WIRES: [Wire; 2] = [Wire::V3, Wire::V2];

impl Wire {
    fn schema(self) -> &'static str {
        match self {
            Self::V3 => "corpus-cartographer/engine-export",
            Self::V2 => "recipes-cartographer/engine-export",
        }
    }

    fn version(self) -> u64 {
        match self {
            Self::V3 => 3,
            Self::V2 => 2,
        }
    }

    fn items_key(self) -> &'static str {
        match self {
            Self::V3 => "items",
            Self::V2 => "recipes",
        }
    }

    fn theme(self, id: u64, parent: &Value, name: &str) -> Value {
        match self {
            Self::V3 => json!({"id": id, "parent_id": parent, "name": name, "terms": []}),
            Self::V2 => json!({"id": id, "parent_id": parent, "name": name, "epicure_terms": []}),
        }
    }

    /// An assigned item with the given terms; `badged` marks it the way each
    /// format does (a badge in v3, the "none" seal in v2).
    fn item(self, id: u64, title: &str, subtheme: u64, terms: &[&str], badged: bool) -> Value {
        match self {
            Self::V3 => {
                json!({"id": id, "title": title, "subtheme_id": subtheme, "status": "assigned",
                "badges": if badged { vec!["marked"] } else { vec![] },
                "terms": terms.iter().map(|term| json!({"label": term, "term": term})).collect::<Vec<_>>()})
            }
            Self::V2 => {
                json!({"id": id, "title": title, "subtheme_id": subtheme, "status": "assigned",
                "seal": if badged { "none" } else { "complete" },
                "ingredients": terms.iter().map(|term| json!({"name": term, "epicure_term": term})).collect::<Vec<_>>()})
            }
        }
    }

    fn clear_terms(self, item: &mut Value) {
        match self {
            Self::V3 => {
                item["terms"] = json!([]);
                item["badges"] = json!(["marked"]);
            }
            Self::V2 => {
                item["ingredients"] = json!([]);
                item["seal"] = "none".into();
            }
        }
    }

    fn reverse_terms(self, item: &mut Value) {
        let key = match self {
            Self::V3 => "terms",
            Self::V2 => "ingredients",
        };
        item[key].as_array_mut().unwrap().reverse();
    }

    /// The key that counts items in the links document.
    fn count_key(self) -> &'static str {
        self.items_key()
    }
}

/// An item no owner has placed yet.
fn make_waiting(item: &mut Value) {
    item["status"] = "needs_owner".into();
    item["subtheme_id"] = Value::Null;
}

/// A synthetic corpus: three themes of two sub-themes, four items each,
/// two mini-Embeddings terms per item chosen by position.
fn corpus(wire: Wire, extra_item: bool, previous: &Value) -> Value {
    let terms = support::MINI_TERMS;
    let mut themes = Vec::new();
    let mut items = Vec::new();
    for theme in 1..=3_u64 {
        themes.push(wire.theme(theme, &Value::Null, &format!("Theme {theme}")));
        for subtheme_number in 1..=2_u64 {
            let subtheme = theme * 10 + subtheme_number;
            themes.push(wire.theme(subtheme, &theme.into(), &format!("Sub {subtheme}")));
            for position in 0..4_u64 {
                let id = subtheme * 100 + position;
                let first = terms[((theme * 2 + subtheme_number) as usize) % terms.len()];
                let second = terms[((theme + position) as usize) % terms.len()];
                items.push(wire.item(
                    id,
                    &format!("Synthetic {id}"),
                    subtheme,
                    &[first, second],
                    false,
                ));
            }
        }
    }
    if extra_item {
        items.push(wire.item(9999, "Synthetic new", 11, &["garlic"], false));
    }
    let mut document = json!({"schema": wire.schema(), "version": wire.version(), "taxonomy_version": 1,
        "themes": themes, "previous": previous, "rejected_pairs": []});
    document[wire.items_key()] = Value::Array(items);
    document
}

fn run(input: &Value) -> corpus_cartographer::EngineOutput {
    corpus_cartographer::run(
        &input.to_string(),
        &support::mini_embeddings(),
        &LinkOptions::default(),
    )
    .unwrap()
}

fn positions(output: &corpus_cartographer::EngineOutput) -> Value {
    serde_json::from_str::<Value>(&output.links_json).unwrap()["positions"].clone()
}

#[test]
fn the_same_input_gives_the_same_bytes() {
    for wire in WIRES {
        let input = corpus(wire, false, &Value::Null);
        let (first, second) = (run(&input), run(&input));
        assert_eq!(first.links_json, second.links_json, "{wire:?}");
        assert_eq!(first.map_spec_v1, second.map_spec_v1, "{wire:?}");
        assert_eq!(first.map_spec_nested, second.map_spec_nested, "{wire:?}");
    }
}

#[test]
fn territory_identity_does_not_depend_on_input_order() {
    for wire in WIRES {
        let input = corpus(wire, false, &Value::Null);
        let mut shuffled = input.clone();
        for key in ["themes", wire.items_key()] {
            shuffled[key].as_array_mut().unwrap().reverse();
        }
        for item in shuffled[wire.items_key()].as_array_mut().unwrap() {
            wire.reverse_terms(item);
        }
        let input_hash_free = |output: &corpus_cartographer::EngineOutput| {
            let mut document: Value = serde_json::from_str(&output.links_json).unwrap();
            document["input_sha256"] = Value::Null;
            document
        };
        let (ordered, reordered) = (run(&input), run(&shuffled));
        assert_eq!(
            input_hash_free(&ordered),
            input_hash_free(&reordered),
            "{wire:?}"
        );
        assert_eq!(
            ordered.map_spec_nested, reordered.map_spec_nested,
            "{wire:?}"
        );
    }
}

/// Measured on this corpus as 0.048744 on 2026-10-17 and, once flavours were
/// weighted and centred on the corpus (engine 0.3.0), as 0.079999 on
/// 2026-11-01: one recipe in 24 now moves every weight and the mean. The
/// bound is that, rounded up to the next 0.05. On the real corpus the same
/// change took the largest move from 0.0104 to 0.0267 (map-engine spec,
/// open question 2).
const STABILITY_BOUND: f64 = 0.10;

#[test]
fn one_new_item_moves_untouched_territories_less_than_the_bound() {
    for wire in WIRES {
        let first = run(&corpus(wire, false, &Value::Null));
        let before = positions(&first);
        let second = run(&corpus(wire, true, &before));
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
        // Territory 1 gained the item; 2 and 3 did not.
        let largest_move = ["2", "3"]
            .iter()
            .map(|id| {
                (point(&before, id)[0] - point(&after, id)[0])
                    .hypot(point(&before, id)[1] - point(&after, id)[1])
            })
            .fold(0.0, f64::max);
        let relative = largest_move / extent;
        eprintln!(
            "stability {wire:?}: largest move {largest_move:.6} over extent {extent:.6} = {relative:.6}"
        );
        assert!(relative < STABILITY_BOUND, "{wire:?}: {relative}");
    }
}

#[test]
fn edge_corpora_produce_valid_outputs() {
    for wire in WIRES {
        let mut empty = corpus(wire, false, &Value::Null);
        empty["themes"] = json!([]);
        empty[wire.items_key()] = json!([]);
        empty["taxonomy_version"] = Value::Null;
        let mut one = corpus(wire, false, &Value::Null);
        one[wire.items_key()].as_array_mut().unwrap().truncate(1);
        let mut all_waiting = corpus(wire, false, &Value::Null);
        for item in all_waiting[wire.items_key()].as_array_mut().unwrap() {
            make_waiting(item);
        }
        let mut no_vectors = corpus(wire, false, &Value::Null);
        for item in no_vectors[wire.items_key()].as_array_mut().unwrap() {
            wire.clear_terms(item);
        }
        for input in [empty, one, all_waiting, no_vectors] {
            let output = run(&input);
            let document: Value = serde_json::from_str(&output.links_json).unwrap();
            assert_eq!(
                document["counts"][wire.count_key()],
                input[wire.items_key()].as_array().unwrap().len(),
                "{wire:?}"
            );
        }
    }
}
