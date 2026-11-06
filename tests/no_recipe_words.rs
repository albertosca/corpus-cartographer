//! The engine's own code and v3 documents carry no recipe words. A search
//! that finds nothing is only believed after it finds something known.
use std::fs;
use std::path::Path;

const WORDS: [&str; 4] = ["recipe", "ingredient", "epicure", "youtube"];

fn offending(directory: &Path, skip: &[&str]) -> Vec<String> {
    let mut hits = Vec::new();
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if skip.contains(&name.as_str()) || name.contains(".v2") {
            continue;
        }
        if path.is_dir() {
            hits.extend(offending(&path, skip));
            continue;
        }
        let text = fs::read_to_string(&path).unwrap_or_default().to_lowercase();
        if WORDS.iter().any(|word| text.contains(word)) {
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
fn nothing_else_in_the_engine_uses_recipe_words() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for part in ["src", "contracts", "examples"] {
        assert_eq!(
            offending(&root.join(part), &["legacy_v2.rs"]),
            Vec::<String>::new(),
            "{part}"
        );
    }
}
