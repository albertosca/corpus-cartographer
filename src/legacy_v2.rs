//! Export v2, the recipe-era wire format. It is read into the same model as
//! v3: ingredients become item terms, the quantity seal becomes a badge.
//! This is the only file that keeps the old words.

use serde::{Deserialize, Serialize};

use crate::export::{EngineExport, ExportError, Item, ItemTerm, Positions, Status, Theme};
use crate::links::Link;
use crate::output::Counts;

pub const SCHEMA: &str = "recipes-cartographer/engine-export";
pub const VERSION: u32 = 2;
/// What a v2 run has always shown on a recipe with no quantities.
const NO_RECIPE_BADGE: &str = "sem receita";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportV2 {
    schema: String,
    version: u32,
    taxonomy_version: Option<u64>,
    themes: Vec<ThemeV2>,
    recipes: Vec<RecipeV2>,
    previous: Option<Positions>,
    rejected_pairs: Vec<[u64; 2]>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeV2 {
    id: u64,
    parent_id: Option<u64>,
    name: String,
    epicure_terms: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecipeV2 {
    id: u64,
    title: String,
    subtheme_id: Option<u64>,
    status: Status,
    seal: SealV2,
    ingredients: Vec<IngredientV2>,
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SealV2 {
    Complete,
    Absent,
    None,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IngredientV2 {
    name: String,
    epicure_term: Option<String>,
}

pub fn parse(text: &str) -> Result<EngineExport, ExportError> {
    let export: ExportV2 =
        serde_json::from_str(text).map_err(|error| ExportError(error.to_string()))?;
    Ok(EngineExport {
        schema: export.schema,
        version: export.version,
        taxonomy_version: export.taxonomy_version,
        themes: export
            .themes
            .into_iter()
            .map(|theme| Theme {
                id: theme.id,
                parent_id: theme.parent_id,
                name: theme.name,
                terms: theme.epicure_terms,
            })
            .collect(),
        items: export
            .recipes
            .into_iter()
            .map(|recipe| Item {
                id: recipe.id,
                title: recipe.title,
                subtheme_id: recipe.subtheme_id,
                status: recipe.status,
                badges: if recipe.seal == SealV2::None {
                    vec![NO_RECIPE_BADGE.to_string()]
                } else {
                    Vec::new()
                },
                terms: recipe
                    .ingredients
                    .into_iter()
                    .map(|ingredient| ItemTerm {
                        label: ingredient.name,
                        term: ingredient.epicure_term,
                    })
                    .collect(),
            })
            .collect(),
        previous: export.previous,
        rejected_pairs: export.rejected_pairs,
    })
}

// ---- Output side: what a run over a v2 export has always written. ----

pub const LINKS_SCHEMA: &str = "recipes-cartographer/engine-links";
pub const LINKS_VERSION: u32 = 2;
/// The noun the one-line run summary has always used for items.
pub const COUNT_NOUN: &str = "recipes";

pub fn item_key(id: u64) -> String {
    format!("recipe:{id}")
}

#[derive(Serialize)]
pub struct CountsV2 {
    recipes: usize,
    placed: usize,
    waiting: usize,
    with_vector: usize,
    links: usize,
}

impl From<Counts> for CountsV2 {
    fn from(counts: Counts) -> Self {
        Self {
            recipes: counts.items,
            placed: counts.placed,
            waiting: counts.waiting,
            with_vector: counts.with_vector,
            links: counts.links,
        }
    }
}

#[derive(Serialize)]
pub struct LinkV2<'a> {
    source: u64,
    target: u64,
    similarity: f64,
    crosses_theme: bool,
    shared: Vec<SharedTermV2<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    closest: Option<ClosestPairV2<'a>>,
}

#[derive(Serialize)]
struct SharedTermV2<'a> {
    term: &'a str,
    source_names: &'a [String],
    target_names: &'a [String],
}

#[derive(Serialize)]
struct ClosestPairV2<'a> {
    source_term: &'a str,
    source_name: &'a str,
    target_term: &'a str,
    target_name: &'a str,
    similarity: f64,
}

impl<'a> From<&'a Link> for LinkV2<'a> {
    fn from(link: &'a Link) -> Self {
        Self {
            source: link.source,
            target: link.target,
            similarity: link.similarity,
            crosses_theme: link.crosses_theme,
            shared: link
                .shared
                .iter()
                .map(|shared| SharedTermV2 {
                    term: &shared.term,
                    source_names: &shared.source_labels,
                    target_names: &shared.target_labels,
                })
                .collect(),
            closest: link.closest.as_ref().map(|closest| ClosestPairV2 {
                source_term: &closest.source_term,
                source_name: &closest.source_label,
                target_term: &closest.target_term,
                target_name: &closest.target_label,
                similarity: closest.similarity,
            }),
        }
    }
}
