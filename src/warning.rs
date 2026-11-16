//! What the engine tells the owner about a run. The engine only names the
//! kind and the theme; the host application turns each kind into a sentence.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WarningKind {
    NoFlavour,
    NoAlignment,
    UnknownTerms,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Warning {
    pub kind: WarningKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
}

impl Warning {
    pub fn no_flavour(theme_id: u64) -> Self {
        Self {
            kind: WarningKind::NoFlavour,
            theme_id: Some(theme_id),
            count: None,
        }
    }

    pub fn no_alignment(parent_id: Option<u64>) -> Self {
        Self {
            kind: WarningKind::NoAlignment,
            theme_id: parent_id,
            count: None,
        }
    }

    pub fn unknown_terms(count: usize) -> Self {
        Self {
            kind: WarningKind::UnknownTerms,
            theme_id: None,
            count: Some(count),
        }
    }
}
