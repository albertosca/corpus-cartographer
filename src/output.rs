//! The three documents the engine writes: links.json for Rails, the v1 map
//! spec today's corpus-map reads, and the v2 proposal with nesting and edges.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::export::{EngineExport, Format, Positions, Status};
use crate::geometry::round_to;
use crate::layout::Layout;
use crate::legacy_v2;
use crate::links::{ClosestPair, Link, LinkOptions};
use crate::vectors::FlavourSpace;
use crate::warning::Warning;

pub const LINKS_SCHEMA: &str = "corpus-cartographer/engine-links";
pub const LINKS_VERSION: u32 = 3;
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");
const SIMILARITY_DECIMALS: i32 = 4;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Counts {
    pub items: usize,
    pub placed: usize,
    pub waiting: usize,
    pub with_vector: usize,
    pub links: usize,
}

impl Counts {
    pub fn of(export: &EngineExport, space: &FlavourSpace, links: &[Link]) -> Self {
        let placed = export
            .items
            .iter()
            .filter(|item| item.status == Status::Assigned)
            .count();
        Self {
            items: export.items.len(),
            placed,
            waiting: export.items.len() - placed,
            with_vector: space
                .items
                .values()
                .filter(|flavour| flavour.vector.is_some())
                .count(),
            links: links.len(),
        }
    }
}

#[derive(Serialize)]
struct LinksDocument<'a, L: Serialize, C: Serialize> {
    schema: &'static str,
    version: u32,
    engine_version: &'static str,
    input_sha256: &'a str,
    links: Vec<L>,
    positions: Positions,
    warnings: &'a [Warning],
    counts: C,
    options: LinkOptions,
}

fn rounded(link: &Link) -> Link {
    Link {
        similarity: round_to(link.similarity, SIMILARITY_DECIMALS),
        closest: link.closest.clone().map(|closest| ClosestPair {
            similarity: round_to(closest.similarity, SIMILARITY_DECIMALS),
            ..closest
        }),
        ..link.clone()
    }
}

fn pretty<T: Serialize>(document: &T) -> String {
    let mut text =
        serde_json::to_string_pretty(document).expect("engine documents always serialize");
    text.push('\n');
    text
}

pub fn links_json(
    format: Format,
    input_sha256: &str,
    layout: &Layout,
    links: &[Link],
    counts: Counts,
    options: &LinkOptions,
) -> String {
    let rounded_links: Vec<Link> = links.iter().map(rounded).collect();
    match format {
        Format::V3 => pretty(&LinksDocument {
            schema: LINKS_SCHEMA,
            version: LINKS_VERSION,
            engine_version: ENGINE_VERSION,
            input_sha256,
            links: rounded_links,
            positions: layout.positions(),
            warnings: &layout.warnings,
            counts,
            options: *options,
        }),
        Format::V2 => pretty(&LinksDocument {
            schema: legacy_v2::LINKS_SCHEMA,
            version: legacy_v2::LINKS_VERSION,
            engine_version: ENGINE_VERSION,
            input_sha256,
            links: rounded_links.iter().map(legacy_v2::LinkV2::from).collect(),
            positions: layout.positions(),
            warnings: &layout.warnings,
            counts: legacy_v2::CountsV2::from(counts),
            options: *options,
        }),
    }
}

#[derive(Serialize)]
struct Node {
    id: String,
    label: String,
    x: f64,
    y: f64,
    territory: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    badges: Vec<String>,
}

#[derive(Serialize)]
struct TerritoryV1 {
    id: String,
    name: String,
    members: Vec<String>,
    centroid: [f64; 2],
}

#[derive(Serialize)]
struct MapSpecV1 {
    version: &'static str,
    territories: Vec<TerritoryV1>,
    nodes: Vec<Node>,
}

// corpus-map's validateSpec refuses `parent: null` and a reason that is an
// empty string, so both are left out instead.
#[derive(Serialize)]
struct TerritoryNested {
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent: Option<String>,
    members: Vec<String>,
    centroid: [f64; 2],
}

#[derive(Debug, PartialEq, Serialize)]
struct Edge {
    source: String,
    target: String,
    weight: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[derive(Serialize)]
struct MapSpecNested {
    version: &'static str,
    territories: Vec<TerritoryNested>,
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

fn theme_key(id: u64) -> String {
    format!("theme:{id}")
}

fn item_key(format: Format, id: u64) -> String {
    match format {
        Format::V3 => format!("item:{id}"),
        Format::V2 => legacy_v2::item_key(id),
    }
}

/// Assigned item ids per sub-theme, sorted.
fn members_by_subtheme(export: &EngineExport) -> BTreeMap<u64, Vec<u64>> {
    let mut members: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
    for item in &export.items {
        if let (Status::Assigned, Some(subtheme_id)) = (item.status, item.subtheme_id) {
            members.entry(subtheme_id).or_default().push(item.id);
        }
    }
    for ids in members.values_mut() {
        ids.sort_unstable();
    }
    members
}

fn nodes(format: Format, export: &EngineExport, layout: &Layout, innermost: bool) -> Vec<Node> {
    let mut items: Vec<_> = export.items.iter().collect();
    items.sort_by_key(|item| item.id);
    items
        .into_iter()
        .map(|item| {
            let point = layout.items.get(&item.id).copied().unwrap_or_default();
            let territory = item
                .subtheme_id
                .filter(|_| item.status == Status::Assigned)
                .map(|subtheme_id| {
                    if innermost {
                        subtheme_id
                    } else {
                        export.top_level_of(subtheme_id).unwrap_or(subtheme_id)
                    }
                })
                .map(theme_key);
            Node {
                id: item_key(format, item.id),
                label: item.title.clone(),
                x: point[0],
                y: point[1],
                territory,
                badges: item.badges.clone(),
            }
        })
        .collect()
}

fn sorted_themes(export: &EngineExport) -> Vec<&crate::export::Theme> {
    let mut themes: Vec<_> = export.themes.iter().collect();
    themes.sort_by_key(|theme| theme.id);
    themes
}

fn members_under(
    format: Format,
    export: &EngineExport,
    members: &BTreeMap<u64, Vec<u64>>,
    top_level_id: u64,
) -> Vec<String> {
    let mut ids: Vec<u64> = export
        .themes
        .iter()
        .filter(|theme| theme.parent_id == Some(top_level_id))
        .flat_map(|subtheme| members.get(&subtheme.id).cloned().unwrap_or_default())
        .collect();
    ids.sort_unstable();
    ids.into_iter().map(|id| item_key(format, id)).collect()
}

pub fn map_spec_v1(format: Format, export: &EngineExport, layout: &Layout) -> String {
    let members = members_by_subtheme(export);
    let territories = sorted_themes(export)
        .into_iter()
        .filter(|theme| theme.parent_id.is_none())
        .map(|theme| TerritoryV1 {
            id: theme_key(theme.id),
            name: theme.name.clone(),
            members: members_under(format, export, &members, theme.id),
            centroid: layout
                .territories
                .get(&theme.id)
                .copied()
                .unwrap_or_default(),
        })
        .collect();
    pretty(&MapSpecV1 {
        version: "1",
        territories,
        nodes: nodes(format, export, layout, false),
    })
}

// The edge runs from the lower id, so a link kept from the other direction
// names its sides the other way round.
fn reason_text(link: &Link, edge_source: u64) -> String {
    let forward = link.source == edge_source;
    if link.shared.is_empty() {
        link.closest
            .as_ref()
            .map(|closest| {
                let (first, second) = if forward {
                    (&closest.source_label, &closest.target_label)
                } else {
                    (&closest.target_label, &closest.source_label)
                };
                format!("parecidos: {first} ~ {second}")
            })
            .unwrap_or_default()
    } else {
        let names: Vec<&str> = link
            .shared
            .iter()
            .filter_map(|shared| {
                let side = if forward {
                    &shared.source_labels
                } else {
                    &shared.target_labels
                };
                side.first().map(String::as_str)
            })
            .collect();
        format!("em comum: {}", names.join(", "))
    }
}

// The weight is a similarity, which a floor below zero lets go negative;
// corpus-map wants it `>= 0`.
fn edge(format: Format, lower: u64, higher: u64, link: &Link) -> Edge {
    let reason = reason_text(link, lower);
    Edge {
        source: item_key(format, lower),
        target: item_key(format, higher),
        weight: round_to(link.similarity, SIMILARITY_DECIMALS).max(0.0),
        reason: (!reason.is_empty()).then_some(reason),
    }
}

/// corpus-map's `MapSpec` version 1 with `parent` and `edges`: a theme lists
/// no members of its own (its members are its sub-themes'), and nodes sit
/// in sub-themes.
pub fn map_spec_nested(
    format: Format,
    export: &EngineExport,
    layout: &Layout,
    links: &[Link],
) -> String {
    let members = members_by_subtheme(export);
    let territories = sorted_themes(export)
        .into_iter()
        .map(|theme| TerritoryNested {
            id: theme_key(theme.id),
            name: theme.name.clone(),
            parent: theme.parent_id.map(theme_key),
            members: match theme.parent_id {
                None => Vec::new(),
                Some(_) => members
                    .get(&theme.id)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|id| item_key(format, id))
                    .collect(),
            },
            centroid: if theme.parent_id.is_none() {
                layout.territories.get(&theme.id)
            } else {
                layout.subthemes.get(&theme.id)
            }
            .copied()
            .unwrap_or_default(),
        })
        .collect();
    let mut best_by_pair: BTreeMap<(u64, u64), &Link> = BTreeMap::new();
    for link in links {
        let key = (link.source.min(link.target), link.source.max(link.target));
        let replace = best_by_pair.get(&key).is_none_or(|current| {
            link.similarity > current.similarity
                || (link.similarity.total_cmp(&current.similarity).is_eq()
                    && link.source < current.source)
        });
        if replace {
            best_by_pair.insert(key, link);
        }
    }
    let edges = best_by_pair
        .into_iter()
        .map(|((lower, higher), link)| edge(format, lower, higher, link))
        .collect();
    pretty(&MapSpecNested {
        version: "1",
        territories,
        nodes: nodes(format, export, layout, true),
        edges,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::links::SharedTerm;

    fn link(
        source: u64,
        target: u64,
        shared: Vec<SharedTerm>,
        closest: Option<ClosestPair>,
    ) -> Link {
        Link {
            source,
            target,
            similarity: 0.9,
            crosses_theme: false,
            shared,
            closest,
        }
    }

    #[test]
    fn a_shared_reason_names_the_edge_source_term() {
        let shared = vec![SharedTerm {
            term: "garlic".into(),
            source_labels: vec!["alho de 7".into()],
            target_labels: vec!["alho de 3".into()],
        }];
        let reversed = link(7, 3, shared, None);
        assert_eq!(reason_text(&reversed, 3), "em comum: alho de 3");
        assert_eq!(reason_text(&reversed, 7), "em comum: alho de 7");
    }

    #[test]
    fn a_closest_reason_reads_from_the_edge_source() {
        let closest = ClosestPair {
            source_term: "lime".into(),
            source_label: "limão de 7".into(),
            target_term: "lemon".into(),
            target_label: "siciliano de 3".into(),
            similarity: 0.8,
        };
        let reversed = link(7, 3, Vec::new(), Some(closest));
        assert_eq!(
            reason_text(&reversed, 3),
            "parecidos: siciliano de 3 ~ limão de 7"
        );
        assert_eq!(
            reason_text(&reversed, 7),
            "parecidos: limão de 7 ~ siciliano de 3"
        );
    }

    #[test]
    fn a_negative_similarity_is_sent_as_weight_zero() {
        let mut below_zero = link(3, 7, Vec::new(), None);
        below_zero.similarity = -0.25;
        assert_eq!(edge(Format::V3, 3, 7, &below_zero).weight, 0.0);
    }

    #[test]
    fn a_link_with_nothing_to_say_sends_no_reason() {
        let silent = link(3, 7, Vec::new(), None);
        let sent = serde_json::to_value(edge(Format::V3, 3, 7, &silent)).unwrap();
        assert_eq!(
            sent,
            serde_json::json!({ "source": "item:3", "target": "item:7", "weight": 0.9 })
        );
    }
}
