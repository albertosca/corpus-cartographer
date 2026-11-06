//! The engine's input, written by the host application and pinned by
//! contracts/engine-export.v3.schema.json and its golden. Export v2, the
//! item-era wire format, is still read through `legacy_v2`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::legacy_v2;

pub const SCHEMA: &str = "corpus-cartographer/engine-export";
pub const VERSION: u32 = 3;

/// Which wire format a document used. The engine answers in the same one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    V2,
    V3,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineExport {
    pub schema: String,
    pub version: u32,
    pub taxonomy_version: Option<u64>,
    pub themes: Vec<Theme>,
    pub items: Vec<Item>,
    pub previous: Option<Positions>,
    /// Item pairs the owner hid, each `[low, high]` with `low < high`.
    pub rejected_pairs: Vec<[u64; 2]>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    pub id: u64,
    pub parent_id: Option<u64>,
    pub name: String,
    pub terms: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub id: u64,
    pub title: String,
    pub subtheme_id: Option<u64>,
    pub status: Status,
    /// Short texts the producer wants shown on the item's map node.
    pub badges: Vec<String>,
    pub terms: Vec<ItemTerm>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Assigned,
    NeedsOwner,
    Unclassified,
}

/// A term of an item: the producer's own `label` for it, and the `term` it
/// maps to in the embeddings (absent when it maps to none).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemTerm {
    pub label: String,
    pub term: Option<String>,
}

/// Positions of the theme levels, keyed by theme id as a decimal string
/// (JSON object keys are strings). Sent back by the host from the last run.
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

pub fn parse(text: &str) -> Result<(EngineExport, Format), ExportError> {
    let header: Header =
        serde_json::from_str(text).map_err(|error| ExportError(error.to_string()))?;
    let (export, format) = match (header.schema.as_str(), header.version) {
        (SCHEMA, VERSION) => (
            serde_json::from_str::<EngineExport>(text)
                .map_err(|error| ExportError(error.to_string()))?,
            Format::V3,
        ),
        (legacy_v2::SCHEMA, legacy_v2::VERSION) => (legacy_v2::parse(text)?, Format::V2),
        (schema, version) => {
            return Err(ExportError(format!(
                "expected {SCHEMA} version {VERSION} or {} version {}, found {schema} version {version}",
                legacy_v2::SCHEMA,
                legacy_v2::VERSION
            )));
        }
    };
    export.validate()?;
    Ok((export, format))
}

impl EngineExport {
    /// Every term the corpus names, in the items and in the themes, sorted.
    pub fn terms(&self) -> BTreeSet<String> {
        let from_items = self
            .items
            .iter()
            .flat_map(|item| &item.terms)
            .filter_map(|term| term.term.clone());
        let from_themes = self
            .themes
            .iter()
            .flat_map(|theme| theme.terms.iter().cloned());
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
        let mut item_ids = BTreeSet::new();
        for item in &self.items {
            if !item_ids.insert(item.id) {
                return Err(ExportError(format!("item {} appears twice", item.id)));
            }
            match (item.status, item.subtheme_id) {
                (Status::Assigned, Some(subtheme_id)) => {
                    if !matches!(parent_by_theme.get(&subtheme_id), Some(Some(_))) {
                        return Err(ExportError(format!(
                            "item {}: {subtheme_id} is not a sub-theme",
                            item.id
                        )));
                    }
                }
                (Status::Assigned, None) => {
                    return Err(ExportError(format!(
                        "item {}: assigned without a sub-theme",
                        item.id
                    )));
                }
                (_, Some(_)) => {
                    return Err(ExportError(format!(
                        "item {}: only an assigned item has a sub-theme",
                        item.id
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

    fn export_with(themes: &str, items: &str) -> String {
        format!(
            r#"{{"schema":"{SCHEMA}","version":3,"taxonomy_version":null,"themes":{themes},"items":{items},"previous":null,"rejected_pairs":[]}}"#
        )
    }

    const THEMES: &str = r#"[{"id":1,"parent_id":null,"name":"Top","terms":[]},{"id":2,"parent_id":1,"name":"Sub","terms":[]}]"#;

    fn item(id: u64, subtheme: &str, status: &str) -> String {
        format!(
            r#"{{"id":{id},"title":"Synthetic","subtheme_id":{subtheme},"status":"{status}","badges":[],"terms":[]}}"#
        )
    }

    fn error_of(text: &str) -> String {
        parse(text)
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default()
    }

    #[test]
    fn an_assigned_item_in_a_sub_theme_parses() {
        let (export, _format) = parse(&export_with(
            THEMES,
            &format!("[{}]", item(5, "2", "assigned")),
        ))
        .unwrap();
        assert_eq!(export.items[0].status, Status::Assigned);
        assert_eq!(export.top_level_of(2), Some(1));
        assert_eq!(export.top_level_of(1), None);
    }

    #[test]
    fn another_version_is_refused_naming_both_accepted_ones() {
        let text = export_with(THEMES, "[]").replace(r#""version":3"#, r#""version":1"#);
        let error = error_of(&text);
        assert!(
            error.contains("found corpus-cartographer/engine-export version 1"),
            "{error}"
        );
        assert!(error.contains("version 3 or"), "{error}");
        assert!(error.contains("engine-export version 2"), "{error}");
    }

    // A later version may add fields: the version is checked before the
    // shape, or serde would name the field instead of the version.
    #[test]
    fn a_later_version_with_new_fields_names_the_versions_not_the_field() {
        let text = export_with(THEMES, "[]")
            .replace(r#""version":3"#, r#""version":4"#)
            .replace(r#""previous":null"#, r#""previous":null,"new_field":true"#);
        let error = error_of(&text);
        assert!(
            error.contains("version 4") && error.contains("version 3 or"),
            "{error}"
        );
        assert!(!error.contains("new_field"), "{error}");
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
        let (export, _format) = parse(&text).unwrap();
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
        let themes = r#"[{"id":1,"parent_id":null,"name":"A","terms":[]},{"id":1,"parent_id":null,"name":"B","terms":[]}]"#;
        assert!(error_of(&export_with(themes, "[]")).contains("theme 1"));
    }

    #[test]
    fn a_parent_that_is_itself_a_sub_theme_is_refused() {
        let themes = r#"[{"id":1,"parent_id":null,"name":"A","terms":[]},{"id":2,"parent_id":1,"name":"B","terms":[]},{"id":3,"parent_id":2,"name":"C","terms":[]}]"#;
        assert!(error_of(&export_with(themes, "[]")).contains("theme 3"));
    }

    #[test]
    fn an_unknown_parent_is_refused() {
        let themes = r#"[{"id":2,"parent_id":9,"name":"B","terms":[]}]"#;
        assert!(error_of(&export_with(themes, "[]")).contains("unknown parent 9"));
    }

    #[test]
    fn a_duplicate_item_id_is_refused() {
        let items = format!(
            "[{},{}]",
            item(5, "null", "needs_owner"),
            item(5, "null", "unclassified")
        );
        assert!(error_of(&export_with(THEMES, &items)).contains("item 5"));
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
            let text = export_with(THEMES, &format!("[{}]", item(5, subtheme, status)));
            assert!(
                error_of(&text).contains("item 5"),
                "{subtheme} {status}: {}",
                error_of(&text)
            );
        }
    }
}
