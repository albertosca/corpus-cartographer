//! Builds small Epicure spaces for tests, in memory or on disk, so no test
//! needs the real weights except the parity test.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use corpus_cartographer::epicure::Epicure;
use corpus_cartographer::links::{DEFAULT_MAX_LINKS, LinkOptions};
use safetensors::Dtype;
use safetensors::tensor::TensorView;

/// Seven terms in four dimensions, chosen so that garlic and onion are close,
/// lime sits apart, and flour and egg form a third corner.
pub const MINI_TERMS: [&str; 7] = ["beef", "egg", "flour", "garlic", "lime", "onion", "tomato"];
pub const MINI_ROWS: [[f32; 4]; 7] = [
    [1.0, 0.1, 0.0, 0.2],
    [0.2, 0.0, 0.9, 0.1],
    [0.0, 0.0, 1.0, 0.0],
    [0.6, 0.6, 0.0, 0.1],
    [0.0, 1.0, 0.0, 0.3],
    [0.5, 0.5, 0.1, 0.0],
    [0.3, 0.5, 0.4, 0.0],
];

/// The options the golden links document is written with: the default
/// quota and a floor of 0, "more alike than the corpus average" on the
/// centred scale, so changing the default floor never rewrites the golden.
pub fn golden_options() -> LinkOptions {
    LinkOptions {
        max_links: DEFAULT_MAX_LINKS,
        floor: 0.0,
    }
}

pub fn mini_epicure() -> Epicure {
    let terms = MINI_TERMS.iter().map(|term| (*term).to_string()).collect();
    let values = MINI_ROWS.iter().flatten().copied().collect();
    Epicure::from_rows(terms, values, 4)
}

/// Writes vocab.json and embeddings.safetensors the way epicure-core ships them.
pub fn write_epicure(directory: &Path, terms: &[&str], rows: &[&[f32]]) {
    let vocabulary: BTreeMap<&str, usize> = terms
        .iter()
        .enumerate()
        .map(|(index, term)| (*term, index))
        .collect();
    fs::write(
        directory.join("vocab.json"),
        serde_json::to_string(&vocabulary).unwrap(),
    )
    .unwrap();
    let dimension = rows.first().map_or(0, |row| row.len());
    let bytes: Vec<u8> = rows
        .iter()
        .flat_map(|row| row.iter())
        .flat_map(|value| value.to_le_bytes())
        .collect();
    let view = TensorView::new(Dtype::F32, vec![rows.len(), dimension], &bytes).unwrap();
    // safetensors 0.8: check `serialize`'s exact signature with `cargo doc --open -p safetensors`
    // if this line does not compile; the second argument is the optional metadata map.
    let encoded = safetensors::serialize([("embeddings", view)], None).unwrap();
    fs::write(directory.join("embeddings.safetensors"), encoded).unwrap();
}
