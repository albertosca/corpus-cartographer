//! The engine's input, written by the Rails app (`Export::EngineExport`) and
//! pinned by contracts/engine-export.v2.schema.json and its golden.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "recipes-cartographer/engine-export";
pub const VERSION: u32 = 2;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineExport {
    pub schema: String,
    pub version: u32,
    pub taxonomy_version: Option<u64>,
    pub themes: Vec<Theme>,
    pub recipes: Vec<Recipe>,
    pub previous: Option<Positions>,
    /// Recipe pairs the owner hid, each `[low, high]` with `low < high`.
    pub rejected_pairs: Vec<[u64; 2]>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    pub id: u64,
    pub parent_id: Option<u64>,
    pub name: String,
    pub epicure_terms: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub id: u64,
    pub title: String,
    pub subtheme_id: Option<u64>,
    pub status: Status,
    pub seal: Seal,
    pub ingredients: Vec<Ingredient>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Assigned,
    NeedsOwner,
    Unclassified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Seal {
    Complete,
    Absent,
    None,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ingredient {
    pub name: String,
    pub epicure_term: Option<String>,
}

/// Positions of the theme levels, keyed by theme id as a decimal string
/// (JSON object keys are strings). Sent back by Rails from the last run.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Positions {
    pub territories: BTreeMap<String, [f64; 2]>,
    pub subthemes: BTreeMap<String, [f64; 2]>,
}

#[derive(Debug)]
pub struct ExportError(pub String);

impl fmt::Display for ExportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "input: {}", self.0)
    }
}

impl std::error::Error for ExportError {}

/// Only the fields that say which contract a document follows; unknown
/// fields are allowed so another version still gets this far.
#[derive(Deserialize)]
struct Header {
    schema: String,
    version: u32,
}

pub fn parse(text: &str) -> Result<EngineExport, ExportError> {
    let header: Header =
        serde_json::from_str(text).map_err(|error| ExportError(error.to_string()))?;
    check_version(&header.schema, header.version)?;
    let export: EngineExport =
        serde_json::from_str(text).map_err(|error| ExportError(error.to_string()))?;
    export.validate()?;
    Ok(export)
}

fn check_version(schema: &str, version: u32) -> Result<(), ExportError> {
    if schema == SCHEMA && version == VERSION {
        return Ok(());
    }
    Err(ExportError(format!(
        "expected {SCHEMA} version {VERSION}, found {schema} version {version}; the app and the engine disagree, run bin/setup-engine"
    )))
}

impl EngineExport {
    /// Every term the corpus names, in the items and in the themes, sorted.
    pub fn terms(&self) -> BTreeSet<String> {
        let from_items = self
            .recipes
            .iter()
            .flat_map(|recipe| &recipe.ingredients)
            .filter_map(|ingredient| ingredient.epicure_term.clone());
        let from_themes = self
            .themes
            .iter()
            .flat_map(|theme| theme.epicure_terms.iter().cloned());
        from_items.chain(from_themes).collect()
    }

    /// The hidden pairs as (lower id, higher id).
    pub fn rejected_set(&self) -> BTreeSet<(u64, u64)> {
        self.rejected_pairs
            .iter()
            .map(|[low, high]| (*low, *high))
            .collect()
    }

    /// The top-level theme a sub-theme belongs to; None for a top-level
    /// theme or an unknown id.
    pub fn top_level_of(&self, subtheme_id: u64) -> Option<u64> {
        self.themes
            .iter()
            .find(|theme| theme.id == subtheme_id)
            .and_then(|theme| theme.parent_id)
    }

    fn validate(&self) -> Result<(), ExportError> {
        let mut parent_by_theme: BTreeMap<u64, Option<u64>> = BTreeMap::new();
        for theme in &self.themes {
            if parent_by_theme.insert(theme.id, theme.parent_id).is_some() {
                return Err(ExportError(format!("theme {} appears twice", theme.id)));
            }
        }
        for theme in &self.themes {
            if let Some(parent_id) = theme.parent_id {
                match parent_by_theme.get(&parent_id) {
                    Some(None) => {}
                    Some(Some(_)) => {
                        return Err(ExportError(format!(
                            "theme {}: parent {parent_id} is itself a sub-theme",
                            theme.id
                        )));
                    }
                    None => {
                        return Err(ExportError(format!(
                            "theme {}: unknown parent {parent_id}",
                            theme.id
                        )));
                    }
                }
            }
        }
        let mut recipe_ids = BTreeSet::new();
        for recipe in &self.recipes {
            if !recipe_ids.insert(recipe.id) {
                return Err(ExportError(format!("recipe {} appears twice", recipe.id)));
            }
            match (recipe.status, recipe.subtheme_id) {
                (Status::Assigned, Some(subtheme_id)) => {
                    if !matches!(parent_by_theme.get(&subtheme_id), Some(Some(_))) {
                        return Err(ExportError(format!(
                            "recipe {}: {subtheme_id} is not a sub-theme",
                            recipe.id
                        )));
                    }
                }
                (Status::Assigned, None) => {
                    return Err(ExportError(format!(
                        "recipe {}: assigned without a sub-theme",
                        recipe.id
                    )));
                }
                (_, Some(_)) => {
                    return Err(ExportError(format!(
                        "recipe {}: only an assigned recipe has a sub-theme",
                        recipe.id
                    )));
                }
                (_, None) => {}
            }
        }
        for [low, high] in &self.rejected_pairs {
            if low >= high {
                return Err(ExportError(format!(
                    "rejected pair [{low}, {high}]: the lower id comes first"
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn export_with(themes: &str, recipes: &str) -> String {
        format!(
            r#"{{"schema":"{SCHEMA}","version":2,"taxonomy_version":null,"themes":{themes},"recipes":{recipes},"previous":null,"rejected_pairs":[]}}"#
        )
    }

    const THEMES: &str = r#"[{"id":1,"parent_id":null,"name":"Top","epicure_terms":[]},{"id":2,"parent_id":1,"name":"Sub","epicure_terms":[]}]"#;

    fn recipe(id: u64, subtheme: &str, status: &str) -> String {
        format!(
            r#"{{"id":{id},"title":"Synthetic","subtheme_id":{subtheme},"status":"{status}","seal":"complete","ingredients":[]}}"#
        )
    }

    fn error_of(text: &str) -> String {
        parse(text)
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default()
    }

    #[test]
    fn an_assigned_recipe_in_a_sub_theme_parses() {
        let export = parse(&export_with(
            THEMES,
            &format!("[{}]", recipe(5, "2", "assigned")),
        ))
        .unwrap();
        assert_eq!(export.recipes[0].status, Status::Assigned);
        assert_eq!(export.top_level_of(2), Some(1));
        assert_eq!(export.top_level_of(1), None);
    }

    #[test]
    fn another_version_is_refused_naming_setup_engine() {
        let text = export_with(THEMES, "[]").replace(r#""version":2"#, r#""version":1"#);
        let error = error_of(&text);
        assert!(error.contains("version 1"), "{error}");
        assert!(error.contains("bin/setup-engine"), "{error}");
    }

    // A real v1 export has no rejected_pairs, and a later version may add a
    // field: the version is checked before the shape, or serde would name
    // the field instead of the fix.
    #[test]
    fn a_v1_export_or_a_later_one_with_new_fields_names_setup_engine() {
        let version_one = export_with(THEMES, "[]")
            .replace(r#""version":2"#, r#""version":1"#)
            .replace(r#","rejected_pairs":[]"#, "");
        let version_three = export_with(THEMES, "[]")
            .replace(r#""version":2"#, r#""version":3"#)
            .replace(r#""previous":null"#, r#""previous":null,"new_field":true"#);
        for text in [version_one, version_three] {
            let error = error_of(&text);
            assert!(error.contains("bin/setup-engine"), "{error}");
        }
    }

    #[test]
    fn an_export_without_rejected_pairs_is_refused() {
        let text = export_with(THEMES, "[]").replace(r#","rejected_pairs":[]"#, "");
        assert!(
            error_of(&text).contains("rejected_pairs"),
            "{}",
            error_of(&text)
        );
    }

    #[test]
    fn a_rejected_pair_lists_the_lower_id_first() {
        for pair in ["[5,5]", "[7,3]"] {
            let text = export_with(THEMES, "[]").replace(
                r#""rejected_pairs":[]"#,
                &format!(r#""rejected_pairs":[{pair}]"#),
            );
            assert!(
                error_of(&text).contains("rejected pair"),
                "{pair}: {}",
                error_of(&text)
            );
        }
    }

    #[test]
    fn rejected_pairs_become_a_set() {
        let text = export_with(THEMES, "[]").replace(
            r#""rejected_pairs":[]"#,
            r#""rejected_pairs":[[3,7],[1,2]]"#,
        );
        let export = parse(&text).unwrap();
        assert_eq!(
            export.rejected_set().into_iter().collect::<Vec<_>>(),
            vec![(1, 2), (3, 7)]
        );
    }

    #[test]
    fn an_unknown_field_is_refused() {
        let text =
            export_with(THEMES, "[]").replace(r#""previous":null"#, r#""previous":null,"extra":1"#);
        assert!(error_of(&text).contains("extra"), "{}", error_of(&text));
    }

    #[test]
    fn a_duplicate_theme_id_is_refused() {
        let themes = r#"[{"id":1,"parent_id":null,"name":"A","epicure_terms":[]},{"id":1,"parent_id":null,"name":"B","epicure_terms":[]}]"#;
        assert!(error_of(&export_with(themes, "[]")).contains("theme 1"));
    }

    #[test]
    fn a_parent_that_is_itself_a_sub_theme_is_refused() {
        let themes = r#"[{"id":1,"parent_id":null,"name":"A","epicure_terms":[]},{"id":2,"parent_id":1,"name":"B","epicure_terms":[]},{"id":3,"parent_id":2,"name":"C","epicure_terms":[]}]"#;
        assert!(error_of(&export_with(themes, "[]")).contains("theme 3"));
    }

    #[test]
    fn an_unknown_parent_is_refused() {
        let themes = r#"[{"id":2,"parent_id":9,"name":"B","epicure_terms":[]}]"#;
        assert!(error_of(&export_with(themes, "[]")).contains("unknown parent 9"));
    }

    #[test]
    fn a_duplicate_recipe_id_is_refused() {
        let recipes = format!(
            "[{},{}]",
            recipe(5, "null", "needs_owner"),
            recipe(5, "null", "unclassified")
        );
        assert!(error_of(&export_with(THEMES, &recipes)).contains("recipe 5"));
    }

    #[test]
    fn assignment_and_sub_theme_must_agree() {
        for (subtheme, status) in [
            ("null", "assigned"),
            ("1", "assigned"),
            ("7", "assigned"),
            ("2", "needs_owner"),
            ("2", "unclassified"),
        ] {
            let text = export_with(THEMES, &format!("[{}]", recipe(5, subtheme, status)));
            assert!(
                error_of(&text).contains("recipe 5"),
                "{subtheme} {status}: {}",
                error_of(&text)
            );
        }
    }
}
