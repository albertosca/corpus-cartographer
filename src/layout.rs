//! Places territories (top-level themes), sub-themes inside them and items
//! inside their sub-theme, each level by classical MDS on flavour distance
//! (1 − cosine). It works from the bottom up: a sub-theme's disk grows with
//! its item count, a territory's disk holds its sub-themes, and siblings are
//! pushed apart until no two disks overlap. It aligns the two theme levels
//! to the previous run, puts what has no flavour on a ring around its parent
//! and the items waiting for the owner in a row below the map.

use std::collections::BTreeMap;
use std::f64::consts::TAU;

use crate::embeddings::Embeddings;
use crate::export::{EngineExport, Positions, Status, Theme};
use crate::geometry::{align, classical_mds, round_to};
use crate::vectors::{FlavourSpace, cosine, mean_direction, term_vector};
use crate::warning::Warning;

/// A sub-theme of n items fills a disk of radius `ITEM_SPACING * sqrt(n)`, so
/// its area grows with its size and every item gets about the same room.
const ITEM_SPACING: f64 = 5.0;
/// The space kept between two sibling disks, as a fraction of their radii.
const SIBLING_GAP_FRACTION: f64 = 0.25;
const SEPARATION_SWEEPS: usize = 200;
const INNER_RING_FRACTION: f64 = 0.3;
const ROW_GAP: f64 = 20.0;
const ROW_STEP: f64 = ITEM_SPACING;
const DECIMALS: i32 = 6;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layout {
    pub territories: BTreeMap<u64, [f64; 2]>,
    pub subthemes: BTreeMap<u64, [f64; 2]>,
    pub items: BTreeMap<u64, [f64; 2]>,
    pub warnings: Vec<Warning>,
}

impl Layout {
    /// The theme levels, as the next run's `previous`.
    pub fn positions(&self) -> Positions {
        let keyed = |points: &BTreeMap<u64, [f64; 2]>| {
            points
                .iter()
                .map(|(id, point)| (id.to_string(), *point))
                .collect()
        };
        Positions {
            territories: keyed(&self.territories),
            subthemes: keyed(&self.subthemes),
        }
    }
}

/// One sibling to place: its id, the name that orders the ring, its flavour.
struct Sibling {
    id: u64,
    name: String,
    flavour: Option<Vec<f64>>,
}

/// How a level turns MDS coordinates into map units.
enum Fit<'a> {
    /// Items: the farthest flavoured one sits at `radius`, and the ones with
    /// no flavour on a ring `ring_gap` outside the rest.
    Radius { radius: f64, ring_gap: f64 },
    /// Themes: each sibling is a disk with its own radius, keyed by id, and
    /// no two disks overlap.
    Disks(&'a BTreeMap<u64, f64>),
}

/// The export indexed for the layout: themes by id, assigned items by sub-theme.
struct Corpus<'a> {
    themes: Vec<&'a Theme>,
    members: BTreeMap<u64, Vec<u64>>,
    space: &'a FlavourSpace,
    embeddings: &'a Embeddings,
}

impl<'a> Corpus<'a> {
    fn new(export: &'a EngineExport, space: &'a FlavourSpace, embeddings: &'a Embeddings) -> Self {
        let mut themes: Vec<&Theme> = export.themes.iter().collect();
        themes.sort_by_key(|theme| theme.id);
        let mut members: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
        for item in &export.items {
            if let (Status::Assigned, Some(subtheme_id)) = (item.status, item.subtheme_id) {
                members.entry(subtheme_id).or_default().push(item.id);
            }
        }
        for ids in members.values_mut() {
            ids.sort_unstable();
        }
        Self {
            themes,
            members,
            space,
            embeddings,
        }
    }

    fn children(&self, parent_id: u64) -> Vec<&'a Theme> {
        self.themes
            .iter()
            .copied()
            .filter(|theme| theme.parent_id == Some(parent_id))
            .collect()
    }

    fn members_of(&self, subtheme_id: u64) -> &[u64] {
        self.members
            .get(&subtheme_id)
            .map_or(&[][..], Vec::as_slice)
    }

    /// The theme as a sibling: its flavour is the mean direction of the assigned
    /// item vectors in `subtheme_ids`, else of its own `terms` (one
    /// weight each, centred like the items), else none.
    fn sibling(&self, theme: &Theme, subtheme_ids: &[u64]) -> Sibling {
        let vectors: Vec<Vec<f64>> = subtheme_ids
            .iter()
            .flat_map(|subtheme_id| self.members_of(*subtheme_id))
            .filter_map(|id| self.space.vector(*id))
            .map(<[f64]>::to_vec)
            .collect();
        let flavour = mean_direction(vectors.iter().map(Vec::as_slice)).or_else(|| {
            let terms: Vec<Vec<f64>> = theme
                .terms
                .iter()
                .filter_map(|term| term_vector(self.embeddings, term))
                .collect();
            mean_direction(terms.iter().map(Vec::as_slice)).map(|raw| self.space.centre(raw))
        });
        Sibling {
            id: theme.id,
            name: theme.name.clone(),
            flavour,
        }
    }
}

pub fn lay_out(export: &EngineExport, space: &FlavourSpace, embeddings: &Embeddings) -> Layout {
    let corpus = Corpus::new(export, space, embeddings);
    let previous = export.previous.clone().unwrap_or_default();
    let previous_territories = parse_keys(&previous.territories);
    let previous_subthemes = parse_keys(&previous.subthemes);

    let mut layout = Layout::default();
    let mut territory_siblings: Vec<Sibling> = Vec::new();
    let mut territories: BTreeMap<u64, Territory> = BTreeMap::new();
    for theme in corpus
        .themes
        .iter()
        .filter(|theme| theme.parent_id.is_none())
    {
        let subtheme_ids: Vec<u64> = corpus
            .children(theme.id)
            .iter()
            .map(|subtheme| subtheme.id)
            .collect();
        territory_siblings.push(corpus.sibling(theme, &subtheme_ids));
        territories.insert(
            theme.id,
            lay_out_territory(
                &corpus,
                theme.id,
                previous_territories.get(&theme.id),
                &previous_subthemes,
                &mut layout.warnings,
            ),
        );
    }
    let territory_radii: BTreeMap<u64, f64> = territories
        .iter()
        .map(|(id, territory)| (*id, territory.radius))
        .collect();
    layout.territories = place_siblings(
        &territory_siblings,
        &Fit::Disks(&territory_radii),
        export.previous.is_some().then_some(&previous_territories),
        None,
        true,
        &mut layout.warnings,
    );
    for (territory_id, territory_point) in &layout.territories {
        let territory = &territories[territory_id];
        for (subtheme_id, offset) in &territory.subthemes {
            layout
                .subthemes
                .insert(*subtheme_id, shifted(*territory_point, *offset));
        }
        for (item_id, offset) in &territory.items {
            layout
                .items
                .insert(*item_id, shifted(*territory_point, *offset));
        }
    }

    place_waiting_row(export, &mut layout);
    round_layout(&mut layout);
    layout.warnings.sort();
    layout
}

/// A territory laid out around the origin: its sub-themes and items as
/// offsets, and how far they reach from the origin.
struct Territory {
    subthemes: BTreeMap<u64, [f64; 2]>,
    items: BTreeMap<u64, [f64; 2]>,
    radius: f64,
}

fn shifted(origin: [f64; 2], offset: [f64; 2]) -> [f64; 2] {
    [origin[0] + offset[0], origin[1] + offset[1]]
}

fn norm(point: [f64; 2]) -> f64 {
    point[0].hypot(point[1])
}

fn lay_out_territory(
    corpus: &Corpus,
    territory_id: u64,
    previous_territory: Option<&[f64; 2]>,
    previous_subthemes: &BTreeMap<u64, [f64; 2]>,
    warnings: &mut Vec<Warning>,
) -> Territory {
    let children = corpus.children(territory_id);
    let mut item_offsets: BTreeMap<u64, BTreeMap<u64, [f64; 2]>> = BTreeMap::new();
    let mut subtheme_radii: BTreeMap<u64, f64> = BTreeMap::new();
    for subtheme in &children {
        let (offsets, radius) = lay_out_items(corpus, subtheme.id, warnings);
        subtheme_radii.insert(subtheme.id, radius);
        item_offsets.insert(subtheme.id, offsets);
    }
    let subtheme_siblings: Vec<Sibling> = children
        .iter()
        .map(|subtheme| corpus.sibling(subtheme, &[subtheme.id]))
        .collect();
    // Offsets from last time, only for this territory's current sub-themes. When the
    // previous run knew none of them (first run with sub-themes, or a new territory)
    // there is nothing to align to and nothing to warn about.
    let previous_offsets: BTreeMap<u64, [f64; 2]> = previous_territory
        .map(|previous_territory| {
            subtheme_siblings
                .iter()
                .filter_map(|sibling| {
                    previous_subthemes
                        .get(&sibling.id)
                        .map(|point| (sibling.id, *point))
                })
                .map(|(id, point)| {
                    (
                        id,
                        [
                            point[0] - previous_territory[0],
                            point[1] - previous_territory[1],
                        ],
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let subthemes = place_siblings(
        &subtheme_siblings,
        &Fit::Disks(&subtheme_radii),
        (!previous_offsets.is_empty()).then_some(&previous_offsets),
        Some(territory_id),
        true,
        warnings,
    );
    let mut items = BTreeMap::new();
    let mut radius = ITEM_SPACING;
    for (subtheme_id, offset) in &subthemes {
        radius = radius.max(norm(*offset) + subtheme_radii[subtheme_id]);
        for (item_id, item_offset) in &item_offsets[subtheme_id] {
            items.insert(*item_id, shifted(*offset, *item_offset));
        }
    }
    Territory {
        subthemes,
        items,
        radius,
    }
}

/// The items of one sub-theme around the origin, and the radius of its disk.
/// Only items with a flavour size the disk: the ones without sit on a ring at
/// its edge, so they neither weigh on the map nor make room for themselves.
fn lay_out_items(
    corpus: &Corpus,
    subtheme_id: u64,
    warnings: &mut Vec<Warning>,
) -> (BTreeMap<u64, [f64; 2]>, f64) {
    let item_siblings: Vec<Sibling> = corpus
        .members_of(subtheme_id)
        .iter()
        .map(|id| Sibling {
            id: *id,
            name: String::new(),
            flavour: corpus.space.vector(*id).map(<[f64]>::to_vec),
        })
        .collect();
    let flavoured = item_siblings
        .iter()
        .filter(|sibling| sibling.flavour.is_some())
        .count();
    let radius = ITEM_SPACING * (flavoured.max(1) as f64).sqrt();
    let inner = if flavoured < item_siblings.len() {
        radius / (1.0 + INNER_RING_FRACTION)
    } else {
        radius
    };
    let offsets = place_siblings(
        &item_siblings,
        &Fit::Radius {
            radius: inner,
            ring_gap: radius - inner,
        },
        None,
        None,
        false,
        warnings,
    );
    (offsets, radius)
}

fn parse_keys(points: &BTreeMap<String, [f64; 2]>) -> BTreeMap<u64, [f64; 2]> {
    points
        .iter()
        .filter_map(|(key, point)| key.parse().ok().map(|id| (id, *point)))
        .collect()
}

/// Lays out one group of siblings around the origin: MDS on the flavoured
/// ones, fitted, aligned to `previous` (when given) on shared ids; the rest
/// on a ring outside, ordered by (name, id). Disks are then pushed apart.
fn place_siblings(
    siblings: &[Sibling],
    fit: &Fit,
    previous: Option<&BTreeMap<u64, [f64; 2]>>,
    parent_id: Option<u64>,
    warn_flavourless: bool,
    warnings: &mut Vec<Warning>,
) -> BTreeMap<u64, [f64; 2]> {
    let flavoured: Vec<&Sibling> = siblings
        .iter()
        .filter(|sibling| sibling.flavour.is_some())
        .collect();
    let distances: Vec<Vec<f64>> = flavoured
        .iter()
        .map(|origin| {
            flavoured
                .iter()
                .map(|destination| {
                    (1.0 - cosine(
                        origin.flavour.as_deref().unwrap_or_default(),
                        destination.flavour.as_deref().unwrap_or_default(),
                    ))
                    .max(0.0)
                })
                .collect()
        })
        .collect();
    let mut points = classical_mds(&distances);
    let target = match fit {
        Fit::Radius { radius, .. } => *radius,
        Fit::Disks(radii) => {
            let squares: f64 = flavoured
                .iter()
                .map(|sibling| radii[&sibling.id].powi(2))
                .sum();
            squares.sqrt() * (1.0 + SIBLING_GAP_FRACTION)
        }
    };
    let farthest = points.iter().map(|point| norm(*point)).fold(0.0, f64::max);
    if farthest > 0.0 {
        for point in &mut points {
            *point = [point[0] * target / farthest, point[1] * target / farthest];
        }
    }
    if let Some(previous) = previous {
        let shared: Vec<usize> = (0..flavoured.len())
            .filter(|index| previous.contains_key(&flavoured[*index].id))
            .collect();
        let current: Vec<[f64; 2]> = shared.iter().map(|index| points[*index]).collect();
        let before: Vec<[f64; 2]> = shared
            .iter()
            .map(|index| previous[&flavoured[*index].id])
            .collect();
        match align(&current, &before) {
            Some(transform) => {
                for point in &mut points {
                    *point = transform.apply(*point);
                }
            }
            // One flavoured sibling has nothing to orient, and none of them
            // placed last time means there is no last time to disagree with.
            None if flavoured.len() >= 2 && !shared.is_empty() => {
                warnings.push(Warning::no_alignment(parent_id));
            }
            None => {}
        }
    }
    let mut placed: BTreeMap<u64, [f64; 2]> = flavoured
        .iter()
        .zip(points)
        .map(|(sibling, point)| (sibling.id, point))
        .collect();

    let mut flavourless: Vec<&Sibling> = siblings
        .iter()
        .filter(|sibling| sibling.flavour.is_none())
        .collect();
    flavourless.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
    place_on_ring(&flavourless, fit, &mut placed);
    if warn_flavourless {
        for sibling in &flavourless {
            warnings.push(Warning::no_flavour(sibling.id));
        }
    }
    if let Fit::Disks(radii) = fit {
        separate(&mut placed, radii);
    }
    placed
}

/// Puts the siblings with no flavour, already in ring order, evenly on a ring
/// around the centre of the ones `placed`, outside all of them.
fn place_on_ring(flavourless: &[&Sibling], fit: &Fit, placed: &mut BTreeMap<u64, [f64; 2]>) {
    let center = if placed.is_empty() {
        [0.0, 0.0]
    } else {
        let count = placed.len() as f64;
        let sum = placed.values().fold([0.0, 0.0], |total, point| {
            [total[0] + point[0], total[1] + point[1]]
        });
        [sum[0] / count, sum[1] / count]
    };
    let own_radius = |id: u64| match fit {
        Fit::Radius { .. } => 0.0,
        Fit::Disks(radii) => radii[&id],
    };
    let reach = placed
        .iter()
        .map(|(id, point)| (point[0] - center[0]).hypot(point[1] - center[1]) + own_radius(*id))
        .fold(0.0, f64::max);
    let ring_gap = match fit {
        Fit::Radius { ring_gap, .. } => *ring_gap,
        Fit::Disks(_) => {
            flavourless
                .iter()
                .map(|sibling| own_radius(sibling.id))
                .fold(0.0, f64::max)
                * (1.0 + SIBLING_GAP_FRACTION)
        }
    };
    let radius = reach + ring_gap;
    let count = flavourless.len() as f64;
    for (position, sibling) in flavourless.iter().enumerate() {
        let angle = TAU * position as f64 / count;
        placed.insert(
            sibling.id,
            [
                center[0] + radius * angle.cos(),
                center[1] + radius * angle.sin(),
            ],
        );
    }
}

/// Pushes overlapping disks apart along the line between their centres, pair
/// by pair in id order, then scales the group up around its centre if a pair
/// still overlaps, so that no two disks overlap when it returns.
fn separate(placed: &mut BTreeMap<u64, [f64; 2]>, radii: &BTreeMap<u64, f64>) {
    let ids: Vec<u64> = placed.keys().copied().collect();
    let mut points: Vec<[f64; 2]> = placed.values().copied().collect();
    let count = points.len();
    let needed = |left: usize, right: usize| {
        (radii[&ids[left]] + radii[&ids[right]]) * (1.0 + SIBLING_GAP_FRACTION)
    };
    for _sweep in 0..SEPARATION_SWEEPS {
        let mut moved = false;
        for left in 0..count {
            for right in left + 1..count {
                let delta = [
                    points[right][0] - points[left][0],
                    points[right][1] - points[left][1],
                ];
                let gap = norm(delta);
                let need = needed(left, right);
                if gap >= need * (1.0 - 1e-9) {
                    continue;
                }
                // Coincident centres get a direction from their positions in
                // the group, so the result never depends on anything else.
                let direction = if gap > 0.0 {
                    [delta[0] / gap, delta[1] / gap]
                } else {
                    let angle = TAU * (left * count + right) as f64 / (count * count) as f64;
                    [angle.cos(), angle.sin()]
                };
                let push = (need - gap) / 2.0;
                points[left] = [
                    points[left][0] - direction[0] * push,
                    points[left][1] - direction[1] * push,
                ];
                points[right] = [
                    points[right][0] + direction[0] * push,
                    points[right][1] + direction[1] * push,
                ];
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    let scale = (0..count)
        .flat_map(|left| (left + 1..count).map(move |right| (left, right)))
        .filter_map(|(left, right)| {
            let gap =
                (points[right][0] - points[left][0]).hypot(points[right][1] - points[left][1]);
            (gap > 0.0).then(|| needed(left, right) / gap)
        })
        .fold(1.0, f64::max);
    if scale > 1.0 {
        let centre = points.iter().fold([0.0, 0.0], |total, point| {
            [
                total[0] + point[0] / count as f64,
                total[1] + point[1] / count as f64,
            ]
        });
        for point in &mut points {
            *point = [
                centre[0] + (point[0] - centre[0]) * scale,
                centre[1] + (point[1] - centre[1]) * scale,
            ];
        }
    }
    for (id, point) in ids.into_iter().zip(points) {
        placed.insert(id, point);
    }
}

fn place_waiting_row(export: &EngineExport, layout: &mut Layout) {
    let mut waiting: Vec<u64> = export
        .items
        .iter()
        .filter(|item| item.status != Status::Assigned)
        .map(|item| item.id)
        .collect();
    waiting.sort_unstable();
    let everything = layout
        .territories
        .values()
        .chain(layout.subthemes.values())
        .chain(layout.items.values());
    let (leftmost, lowest) = everything
        .fold((f64::INFINITY, f64::INFINITY), |(left, low), point| {
            (left.min(point[0]), low.min(point[1]))
        });
    let (leftmost, lowest) = if leftmost.is_finite() {
        (leftmost, lowest)
    } else {
        (0.0, 0.0)
    };
    for (position, id) in waiting.into_iter().enumerate() {
        layout.items.insert(
            id,
            [leftmost + position as f64 * ROW_STEP, lowest - ROW_GAP],
        );
    }
}

fn round_layout(layout: &mut Layout) {
    for points in [
        &mut layout.territories,
        &mut layout.subthemes,
        &mut layout.items,
    ] {
        for point in points.values_mut() {
            *point = [round_to(point[0], DECIMALS), round_to(point[1], DECIMALS)];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embeddings::Embeddings;
    use crate::export::parse;
    use crate::vectors::FlavourSpace;
    use crate::warning::WarningKind;

    /// Two opposite flavour corners (beef, lime) and a third (flour).
    fn embeddings() -> Embeddings {
        Embeddings::from_rows(
            vec!["beef".into(), "flour".into(), "lime".into()],
            vec![1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0],
            3,
        )
    }

    const THEMES: &str = r#"[
        {"id":1,"parent_id":null,"name":"Churrasco","terms":[]},
        {"id":2,"parent_id":null,"name":"Massas","terms":[]},
        {"id":11,"parent_id":1,"name":"Cortes","terms":[]},
        {"id":12,"parent_id":1,"name":"Molhos","terms":[]},
        {"id":21,"parent_id":2,"name":"Frescas","terms":[]}
    ]"#;

    fn item(id: u64, subtheme: Option<u64>, terms: &[&str]) -> String {
        let (subtheme, status) = subtheme.map_or(("null".to_string(), "needs_owner"), |id| {
            (id.to_string(), "assigned")
        });
        let item_terms: Vec<String> = terms
            .iter()
            .map(|term| format!(r#"{{"label":"Synthetic {term}","term":"{term}"}}"#))
            .collect();
        let badges = if terms.is_empty() {
            r#"["bare"]"#
        } else {
            "[]"
        };
        format!(
            r#"{{"id":{id},"title":"Synthetic {id}","subtheme_id":{subtheme},"status":"{status}","badges":{badges},"terms":[{}]}}"#,
            item_terms.join(",")
        )
    }

    fn layout_of(themes: &str, items: &[String], previous: &str) -> Layout {
        let text = format!(
            r#"{{"schema":"corpus-cartographer/engine-export","version":3,"taxonomy_version":1,"themes":{themes},"items":[{}],"previous":{previous},"rejected_pairs":[]}}"#,
            items.join(",")
        );
        let (export, _format) = parse(&text).unwrap();
        let embeddings = embeddings();
        let space = FlavourSpace::build(&export.items, &embeddings);
        lay_out(&export, &space, &embeddings)
    }

    fn base_items() -> Vec<String> {
        vec![
            item(101, Some(11), &["beef"]),
            item(102, Some(12), &["beef", "lime"]),
            item(103, Some(21), &["flour"]),
            item(104, Some(21), &["flour", "lime"]),
        ]
    }

    fn distance(left: [f64; 2], right: [f64; 2]) -> f64 {
        (left[0] - right[0]).hypot(left[1] - right[1])
    }

    #[test]
    fn sub_themes_and_items_stay_inside_their_territory() {
        let layout = layout_of(THEMES, &base_items(), "null");
        let gap = distance(layout.territories[&1], layout.territories[&2]);
        let room = gap * 0.35;
        for (subtheme, territory) in [(11, 1), (12, 1), (21, 2)] {
            assert!(
                distance(layout.subthemes[&subtheme], layout.territories[&territory])
                    <= room + 1e-6,
                "sub-theme {subtheme}"
            );
        }
        for (item, territory) in [(101, 1), (102, 1), (103, 2), (104, 2)] {
            assert!(
                distance(layout.items[&item], layout.territories[&territory]) < gap / 2.0,
                "item {item}"
            );
        }
        assert!(layout.warnings.is_empty(), "{:?}", layout.warnings);
    }

    #[test]
    fn title_only_items_neither_weigh_nor_gather() {
        // Backlog 17 (f), the spec's synthetic gate: ten title-only items spread over sub-themes.
        let mut with_title_only = base_items();
        for (offset, subtheme) in [11, 12, 21, 11, 12, 21, 11, 12, 21, 21]
            .into_iter()
            .enumerate()
        {
            with_title_only.push(item(200 + offset as u64, Some(subtheme), &[]));
        }
        let without = layout_of(THEMES, &base_items(), "null");
        let with = layout_of(THEMES, &with_title_only, "null");

        assert_eq!(
            with.territories, without.territories,
            "title-only items moved a territory"
        );
        assert_eq!(
            with.subthemes, without.subthemes,
            "title-only items moved a sub-theme"
        );
        for (offset, subtheme) in [11, 12, 21, 11, 12, 21, 11, 12, 21, 21]
            .into_iter()
            .enumerate()
        {
            let position = with.items[&(200 + offset as u64)];
            let own = distance(position, with.subthemes[&subtheme]);
            let nearest_other = with
                .subthemes
                .iter()
                .filter(|(id, _)| **id != subtheme)
                .map(|(_, other)| distance(position, *other))
                .fold(f64::INFINITY, f64::min);
            assert!(
                own < nearest_other,
                "title-only item {} sits nearer another sub-theme",
                200 + offset
            );
        }
    }

    #[test]
    fn unassigned_items_wait_in_a_row_below_the_map() {
        let mut items = base_items();
        items.push(item(301, None, &["beef"]));
        items.push(item(300, None, &[]));
        let layout = layout_of(THEMES, &items, "null");

        let lowest_placed = [101, 102, 103, 104]
            .iter()
            .map(|id| layout.items[id][1])
            .fold(f64::INFINITY, f64::min);
        let (first, second) = (layout.items[&300], layout.items[&301]);
        assert!(first[1] < lowest_placed && (first[1] - second[1]).abs() < 1e-9);
        assert!(first[0] < second[0], "the row is ordered by id");
    }

    #[test]
    fn a_theme_without_flavour_goes_on_the_ring_with_a_warning() {
        let themes = THEMES.replace(
            r#"{"id":21,"#,
            r#"{"id":31,"parent_id":null,"name":"Doces","terms":[]},{"id":21,"#,
        );
        let layout = layout_of(&themes, &base_items(), "null");

        let farthest = [1, 2]
            .iter()
            .map(|id| distance(layout.territories[id], [0.0, 0.0]))
            .fold(0.0, f64::max);
        assert!(distance(layout.territories[&31], [0.0, 0.0]) > farthest);
        assert!(
            layout
                .warnings
                .iter()
                .any(|warning| warning.kind == WarningKind::NoFlavour
                    && warning.theme_id == Some(31))
        );
    }

    #[test]
    fn a_theme_falls_back_to_its_own_terms() {
        let themes = THEMES.replace(
            r#"{"id":21,"#,
            r#"{"id":31,"parent_id":null,"name":"Doces","terms":["flour"]},{"id":21,"#,
        );
        let layout = layout_of(&themes, &base_items(), "null");
        assert!(
            !layout
                .warnings
                .iter()
                .any(|warning| warning.theme_id == Some(31)),
            "{:?}",
            layout.warnings
        );
    }

    #[test]
    fn a_theme_that_falls_back_to_its_terms_is_centred_like_its_siblings() {
        // Sub-theme 11 holds one beef item; theme 31 has no items and
        // the term beef, so both flavours must be the same direction.
        let themes = THEMES.replace(
            r#"{"id":21,"#,
            r#"{"id":31,"parent_id":1,"name":"Brasa","terms":["beef"]},{"id":21,"#,
        );
        let text = format!(
            r#"{{"schema":"corpus-cartographer/engine-export","version":3,"taxonomy_version":1,"themes":{themes},"items":[{}],"previous":null,"rejected_pairs":[]}}"#,
            base_items().join(",")
        );
        let (export, _format) = parse(&text).unwrap();
        let embeddings = embeddings();
        let space = FlavourSpace::build(&export.items, &embeddings);
        assert!(space.mean.is_some(), "four items are centred");
        let corpus = Corpus::new(&export, &space, &embeddings);
        let theme = |id: u64| *corpus.themes.iter().find(|theme| theme.id == id).unwrap();

        let from_items = corpus.sibling(theme(11), &[11]).flavour.unwrap();
        let from_terms = corpus.sibling(theme(31), &[31]).flavour.unwrap();
        assert!(
            cosine(&from_items, &from_terms) > 1.0 - 1e-9,
            "{from_items:?} {from_terms:?}"
        );
    }

    #[test]
    fn a_theme_that_falls_back_to_several_terms_weighs_each_term_once() {
        // Beef is in three items and lime in one, so their rarity weights
        // differ; the theme's own terms still count one each.
        let themes = THEMES.replace(
            r#"{"id":21,"#,
            r#"{"id":31,"parent_id":1,"name":"Brasa","terms":["beef","lime"]},{"id":21,"#,
        );
        let mut items = base_items();
        items.push(item(105, Some(21), &["beef"]));
        let text = format!(
            r#"{{"schema":"corpus-cartographer/engine-export","version":3,"taxonomy_version":1,"themes":{themes},"items":[{}],"previous":null,"rejected_pairs":[]}}"#,
            items.join(",")
        );
        let (export, _format) = parse(&text).unwrap();
        let embeddings = embeddings();
        let space = FlavourSpace::build(&export.items, &embeddings);
        assert!(space.weight("beef") < space.weight("lime"));
        let corpus = Corpus::new(&export, &space, &embeddings);
        let theme = *corpus.themes.iter().find(|theme| theme.id == 31).unwrap();

        let flavour = corpus.sibling(theme, &[31]).flavour.unwrap();
        let half = std::f64::consts::FRAC_1_SQRT_2;
        let expected = space.centre(vec![half, half, 0.0]);
        assert!(
            cosine(&flavour, &expected) > 1.0 - 1e-9,
            "{flavour:?} {expected:?}"
        );
    }

    #[test]
    fn the_previous_run_fixes_the_orientation() {
        let first = layout_of(THEMES, &base_items(), "null");
        // Rotate the first map by 90 degrees and hand it back as the previous run.
        let turned: Vec<String> = first
            .territories
            .iter()
            .map(|(id, point)| format!(r#""{id}":[{},{}]"#, -point[1], point[0]))
            .collect();
        let previous = format!(
            r#"{{"territories":{{{}}},"subthemes":{{}}}}"#,
            turned.join(",")
        );

        let second = layout_of(THEMES, &base_items(), &previous);

        for (id, point) in &first.territories {
            let expected = [-point[1], point[0]];
            assert!(
                distance(second.territories[id], expected) < 1e-5,
                "territory {id}"
            );
        }
    }

    #[test]
    fn stale_previous_ids_are_ignored() {
        let previous =
            r#"{"territories":{"1":[0.0,0.0],"99":[50.0,50.0]},"subthemes":{"98":[1.0,1.0]}}"#;
        let layout = layout_of(THEMES, &base_items(), previous);
        assert!(
            layout
                .warnings
                .iter()
                .any(|warning| warning.kind == WarningKind::NoAlignment
                    && warning.theme_id.is_none())
        );
        assert_eq!(layout.territories.len(), 2);
    }

    #[test]
    fn a_previous_run_with_no_positions_is_a_first_run_not_a_misalignment() {
        let layout = layout_of(
            THEMES,
            &base_items(),
            r#"{"territories":{},"subthemes":{}}"#,
        );
        assert!(
            layout
                .warnings
                .iter()
                .all(|warning| warning.kind != WarningKind::NoAlignment),
            "{:?}",
            layout.warnings
        );
    }

    #[test]
    fn edge_corpora_lay_out_without_panicking() {
        let empty = layout_of("[]", &[], "null");
        assert!(empty.territories.is_empty() && empty.items.is_empty());

        let one = layout_of(THEMES, &[item(101, Some(11), &["beef"])], "null");
        assert_eq!(one.items.len(), 1);

        let waiting = layout_of(
            THEMES,
            &[item(301, None, &["beef"]), item(302, None, &[])],
            "null",
        );
        assert_eq!(waiting.items.len(), 2);
        assert!(
            waiting
                .warnings
                .iter()
                .all(|warning| warning.kind == WarningKind::NoFlavour)
        );
    }

    #[test]
    fn input_order_does_not_change_the_layout() {
        let mut reversed = base_items();
        reversed.reverse();
        let themes_reversed = {
            let themes: Vec<serde_json::Value> = serde_json::from_str(THEMES).unwrap();
            serde_json::to_string(&themes.into_iter().rev().collect::<Vec<_>>()).unwrap()
        };
        assert_eq!(
            layout_of(THEMES, &base_items(), "null"),
            layout_of(&themes_reversed, &reversed, "null")
        );
    }

    /// How far the given items reach from `center`.
    fn reach(layout: &Layout, center: [f64; 2], item_ids: &[u64]) -> f64 {
        item_ids
            .iter()
            .map(|id| distance(layout.items[id], center))
            .fold(0.0, f64::max)
    }

    /// Two territories of three sub-themes with four items each; in the
    /// first, sub-themes 11 and 12 have almost the same flavour.
    fn close_pair_corpus() -> (&'static str, Vec<String>, BTreeMap<u64, Vec<u64>>) {
        let themes = r#"[
            {"id":1,"parent_id":null,"name":"Close pair","terms":[]},
            {"id":2,"parent_id":null,"name":"Far pair","terms":[]},
            {"id":11,"parent_id":1,"name":"Beef","terms":[]},
            {"id":12,"parent_id":1,"name":"Beef again","terms":[]},
            {"id":13,"parent_id":1,"name":"Flour","terms":[]},
            {"id":21,"parent_id":2,"name":"Flour","terms":[]},
            {"id":22,"parent_id":2,"name":"Lime","terms":[]},
            {"id":23,"parent_id":2,"name":"Beef","terms":[]}
        ]"#;
        let members: [(u64, [&[&str]; 4]); 6] = [
            (
                11,
                [
                    &["beef"],
                    &["beef", "flour"],
                    &["beef", "lime"],
                    &["beef", "flour", "lime"],
                ],
            ),
            (
                12,
                [&["beef"], &["beef", "flour"], &["beef", "lime"], &["beef"]],
            ),
            (
                13,
                [
                    &["flour"],
                    &["flour", "beef"],
                    &["flour", "lime"],
                    &["flour", "beef", "lime"],
                ],
            ),
            (
                21,
                [
                    &["flour"],
                    &["flour", "beef"],
                    &["flour", "lime"],
                    &["flour", "beef", "lime"],
                ],
            ),
            (
                22,
                [
                    &["lime"],
                    &["lime", "beef"],
                    &["lime", "flour"],
                    &["lime", "beef", "flour"],
                ],
            ),
            (
                23,
                [
                    &["beef"],
                    &["beef", "flour"],
                    &["beef", "lime"],
                    &["beef", "flour", "lime"],
                ],
            ),
        ];
        let mut items = Vec::new();
        let mut ids_of: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
        for (subtheme, term_lists) in members {
            for (position, terms) in term_lists.iter().enumerate() {
                let id = subtheme * 10 + position as u64;
                items.push(item(id, Some(subtheme), terms));
                ids_of.entry(subtheme).or_default().push(id);
            }
        }
        (themes, items, ids_of)
    }

    #[test]
    fn room_follows_size_and_not_the_closest_sibling() {
        // The arXiv run packed 600 items into a radius of 1.2 on a map 100 wide,
        // because one close pair of sub-themes shrank the room of every other.
        let (themes, items, ids_of) = close_pair_corpus();
        let layout = layout_of(themes, &items, "null");

        let spread: BTreeMap<u64, f64> = ids_of
            .iter()
            .map(|(subtheme, ids)| (*subtheme, reach(&layout, layout.subthemes[subtheme], ids)))
            .collect();
        for (subtheme, value) in &spread {
            assert!(
                (value - spread[&21]).abs() <= 1e-6 * spread[&21],
                "sub-theme {subtheme} spreads {value}, sub-theme 21 spreads {}",
                spread[&21]
            );
        }
        for (left, right) in [(11, 12), (11, 13), (12, 13), (21, 22), (21, 23), (22, 23)] {
            assert!(
                distance(layout.subthemes[&left], layout.subthemes[&right])
                    >= spread[&left] + spread[&right],
                "sub-themes {left} and {right} overlap"
            );
        }
        let territory_reach = |territory: u64, subthemes: [u64; 3]| {
            let ids: Vec<u64> = subthemes
                .iter()
                .flat_map(|subtheme| ids_of[subtheme].clone())
                .collect();
            reach(&layout, layout.territories[&territory], &ids)
        };
        assert!(
            distance(layout.territories[&1], layout.territories[&2])
                >= territory_reach(1, [11, 12, 13]) + territory_reach(2, [21, 22, 23]),
            "territories overlap"
        );
    }
}
