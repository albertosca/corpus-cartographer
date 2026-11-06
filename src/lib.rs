//! The recipes-cartographer map engine: layout, flavour links and the map spec.

pub mod epicure;
pub mod export;
pub mod geometry;
pub mod layout;
pub mod links;
pub mod output;
pub mod vectors;
pub mod warning;

use std::fmt::Write;

use sha2::{Digest, Sha256};

use crate::epicure::Epicure;
use crate::export::ExportError;
use crate::links::LinkOptions;
use crate::output::Counts;
use crate::vectors::FlavourSpace;
use crate::warning::Warning;

pub struct EngineOutput {
    pub links_json: String,
    pub map_spec_v1: String,
    pub map_spec_nested: String,
    pub summary: String,
}

/// The most cells (axes by the larger of axes and items) the engine will hold
/// when every term is its own axis; past it the corpus needs embeddings.
const MAXIMUM_AXIS_CELLS: usize = 4_000_000;

/// Everything the binary does except reading and writing files.
pub fn run(
    input_text: &str,
    epicure: &Epicure,
    options: &LinkOptions,
) -> Result<EngineOutput, ExportError> {
    let export = export::parse(input_text)?;
    Ok(build(input_text, &export, epicure, options))
}

/// `run` for a corpus with no embeddings: each term the export names becomes
/// its own axis, so similarity comes only from shared terms.
pub fn run_without_embeddings(
    input_text: &str,
    options: &LinkOptions,
) -> Result<EngineOutput, NoEmbeddingsError> {
    let export = export::parse(input_text)?;
    let terms = export.terms();
    let items_with_terms = export
        .recipes
        .iter()
        .filter(|recipe| {
            recipe
                .ingredients
                .iter()
                .any(|ingredient| ingredient.epicure_term.is_some())
        })
        .count();
    check_axis_budget(terms.len(), items_with_terms)?;
    let epicure = Epicure::one_axis_per_term(terms.into_iter().collect());
    Ok(build(input_text, &export, &epicure, options))
}

/// Layout and links grow roughly with the cube of the corpus (measured: 1000
/// terms over 1000 items took 14 s, 2000 over 2000 took 138 s), so the limit
/// is set by waiting time as much as by memory.
fn check_axis_budget(terms: usize, items_with_terms: usize) -> Result<(), NoEmbeddingsError> {
    let cells = terms.saturating_mul(terms.max(items_with_terms));
    if cells > MAXIMUM_AXIS_CELLS {
        return Err(NoEmbeddingsError::TooLarge {
            terms,
            items: items_with_terms,
        });
    }
    Ok(())
}

/// Why `run_without_embeddings` stopped: the export is unusable, or it is
/// well-formed but too big for one axis per term.
#[derive(Debug)]
pub enum NoEmbeddingsError {
    Export(ExportError),
    TooLarge { terms: usize, items: usize },
}

impl std::fmt::Display for NoEmbeddingsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Export(error) => error.fmt(formatter),
            Self::TooLarge { terms, items } => write!(
                formatter,
                "{terms} terms across {items} items is too large for one axis per term (more than {MAXIMUM_AXIS_CELLS} cells); supply embeddings to compress the terms"
            ),
        }
    }
}

impl std::error::Error for NoEmbeddingsError {}

impl From<ExportError> for NoEmbeddingsError {
    fn from(error: ExportError) -> Self {
        Self::Export(error)
    }
}

fn build(
    input_text: &str,
    export: &export::EngineExport,
    epicure: &Epicure,
    options: &LinkOptions,
) -> EngineOutput {
    let input_sha256 = Sha256::digest(input_text.as_bytes()).iter().fold(
        String::with_capacity(64),
        |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        },
    );
    let space = FlavourSpace::build(&export.recipes, epicure);
    let mut layout = layout::lay_out(export, &space, epicure);
    if space.unknown_terms > 0 {
        layout
            .warnings
            .push(Warning::unknown_terms(space.unknown_terms));
        layout.warnings.sort();
    }
    let links = links::find_links(export, &space, epicure, options);
    let counts = Counts::of(export, &space, &links);
    EngineOutput {
        links_json: output::links_json(&input_sha256, &layout, &links, counts, options),
        map_spec_v1: output::map_spec_v1(export, &layout),
        map_spec_nested: output::map_spec_nested(export, &layout, &links),
        summary: format!(
            "recipes={} placed={} waiting={} with_vector={} links={} warnings={}",
            counts.recipes,
            counts.placed,
            counts.waiting,
            counts.with_vector,
            counts.links,
            layout.warnings.len()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::check_axis_budget;

    #[test]
    fn the_budget_is_strict_at_the_limit() {
        assert!(check_axis_budget(2000, 2000).is_ok());
        assert!(check_axis_budget(2001, 2000).is_err());
        assert!(check_axis_budget(2000, 2001).is_err());
    }

    #[test]
    fn more_items_than_terms_count_against_the_budget() {
        assert!(check_axis_budget(1000, 4000).is_ok());
        assert!(check_axis_budget(1000, 4001).is_err());
    }
}
