//! The engine's own code, its v3 documents, its tests and its READMEs carry
//! no words of the first host application. The words may live only in the
//! v2 reader and in the tests of that reader. A search that finds nothing is
//! only believed after it finds something known.
use std::fs;
use std::path::Path;

const WORDS: [&str; 5] = ["recipe", "ingredient", "epicure", "youtube", "cooking"];

/// The tests that exist to speak v2 and so must use its words.
const V2_TESTS: [&str; 5] = [
    "contracts.rs",
    "equivalence.rs",
    "guarantees.rs",
    "no_recipe_words.rs",
    "wire_names.rs",
];

fn mentions_a_word(text: &str) -> bool {
    let lower = text.to_lowercase();
    WORDS.iter().any(|word| lower.contains(word))
}

fn offending(directory: &Path, skip: &[&str]) -> Vec<String> {
    let mut hits = Vec::new();
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if skip.contains(&name.as_str())
            || name.ends_with(".v2.json")
            || name.ends_with(".v2.schema.json")
        {
            continue;
        }
        if path.is_dir() {
            hits.extend(offending(&path, skip));
            continue;
        }
        if mentions_a_word(&String::from_utf8_lossy(&fs::read(&path).unwrap())) {
            hits.push(path.display().to_string());
        }
    }
    hits.sort();
    hits
}

#[test]
fn the_search_finds_a_known_hit_first() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    assert_eq!(
        offending(&src, &[]),
        vec![src.join("legacy_v2.rs").display().to_string()]
    );
}

#[test]
fn every_word_in_the_list_is_caught_by_the_check() {
    for word in WORDS {
        assert!(
            mentions_a_word(&format!("a {} here", word.to_uppercase())),
            "{word}"
        );
    }
    assert!(!mentions_a_word("a corpus of items and terms"));
}

#[test]
fn the_search_finds_a_known_hit_in_the_tests_too() {
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let hits = offending(&tests, &[]);
    assert!(
        hits.iter().any(|hit| hit.ends_with("wire_names.rs")),
        "{hits:?}"
    );
}

#[test]
fn nothing_else_in_the_engine_uses_the_words() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for part in ["src", "contracts", "examples"] {
        assert_eq!(
            offending(&root.join(part), &["legacy_v2.rs"]),
            Vec::<String>::new(),
            "{part}"
        );
    }
    assert_eq!(
        offending(&root.join("tests"), &V2_TESTS),
        Vec::<String>::new(),
        "tests"
    );
    for readme in ["README.md", "README.pt.md"] {
        let text = fs::read_to_string(root.join(readme)).unwrap();
        assert!(!mentions_a_word(&text), "{readme}");
    }
}
