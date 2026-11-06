//! Places territories (top-level themes), sub-themes inside them and items
//! inside their sub-theme, each level by classical MDS on flavour distance
//! (1 − cosine); aligns the two theme levels to the previous run; puts what
//! has no flavour on a ring around its parent and the items waiting for
//! the owner in a row below the map.

use std::collections::BTreeMap;
use std::f64::consts::TAU;

use crate::embeddings::Embeddings;
use crate::export::{EngineExport, Positions, Status, Theme};
use crate::geometry::{align, classical_mds, round_to};
use crate::vectors::{FlavourSpace, cosine, mean_direction, term_vector};
use crate::warning::Warning;

pub const TERRITORY_SCALE: f64 = 100.0;
const ROOM_FRACTION: f64 = 0.35;
const FILL_FRACTION: f64 = 0.6;
const RING_GAP: f64 = 20.0;
const INNER_RING_FRACTION: f64 = 0.3;
const ROW_GAP: f64 = 20.0;
const ROW_STEP: f64 = 5.0;
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
enum Fit {
    Scale(f64),
    Radius(f64),
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
    let territory_siblings: Vec<Sibling> = corpus
        .themes
        .iter()
        .filter(|theme| theme.parent_id.is_none())
        .map(|theme| {
            let subtheme_ids: Vec<u64> = corpus
                .children(theme.id)
                .iter()
                .map(|subtheme| subtheme.id)
                .collect();
            corpus.sibling(theme, &subtheme_ids)
        })
        .collect();
    layout.territories = place_siblings(
        &territory_siblings,
        &Fit::Scale(TERRITORY_SCALE),
        RING_GAP,
        export.previous.is_some().then_some(&previous_territories),
        None,
        true,
        &mut layout.warnings,
    );
    let territory_room = room(&layout.territories, TERRITORY_SCALE * ROOM_FRACTION);
    for (territory_id, territory_point) in layout.territories.clone() {
        let territory = Placed {
            id: territory_id,
            point: territory_point,
            room: territory_room,
        };
        lay_out_territory(
            &corpus,
            &territory,
            previous_territories.get(&territory_id),
            &previous_subthemes,
            &mut layout,
        );
    }

    place_waiting_row(export, &mut layout);
    round_layout(&mut layout);
    layout.warnings.sort();
    layout
}

/// A parent already on the map: where it sits and how much room its children get.
struct Placed {
    id: u64,
    point: [f64; 2],
    room: f64,
}

fn lay_out_territory(
    corpus: &Corpus,
    territory: &Placed,
    previous_territory: Option<&[f64; 2]>,
    previous_subthemes: &BTreeMap<u64, [f64; 2]>,
    layout: &mut Layout,
) {
    let subtheme_siblings: Vec<Sibling> = corpus
        .children(territory.id)
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
    let offsets = place_siblings(
        &subtheme_siblings,
        &Fit::Radius(territory.room * FILL_FRACTION),
        territory.room * INNER_RING_FRACTION,
        (!previous_offsets.is_empty()).then_some(&previous_offsets),
        Some(territory.id),
        true,
        &mut layout.warnings,
    );
    let subtheme_room = room(&offsets, territory.room * ROOM_FRACTION);
    for (subtheme_id, offset) in offsets {
        let center = [
            territory.point[0] + offset[0],
            territory.point[1] + offset[1],
        ];
        layout.subthemes.insert(subtheme_id, center);
        lay_out_items(
            corpus,
            &Placed {
                id: subtheme_id,
                point: center,
                room: subtheme_room,
            },
            layout,
        );
    }
}

fn lay_out_items(corpus: &Corpus, subtheme: &Placed, layout: &mut Layout) {
    let item_siblings: Vec<Sibling> = corpus
        .members_of(subtheme.id)
        .iter()
        .map(|id| Sibling {
            id: *id,
            name: String::new(),
            flavour: corpus.space.vector(*id).map(<[f64]>::to_vec),
        })
        .collect();
    let item_offsets = place_siblings(
        &item_siblings,
        &Fit::Radius(subtheme.room * FILL_FRACTION),
        subtheme.room * INNER_RING_FRACTION,
        None,
        None,
        false,
        &mut layout.warnings,
    );
    for (item_id, offset) in item_offsets {
        layout.items.insert(
            item_id,
            [subtheme.point[0] + offset[0], subtheme.point[1] + offset[1]],
        );
    }
}

fn parse_keys(points: &BTreeMap<String, [f64; 2]>) -> BTreeMap<u64, [f64; 2]> {
    points
        .iter()
        .filter_map(|(key, point)| key.parse().ok().map(|id| (id, *point)))
        .collect()
}

/// `ROOM_FRACTION` of the smallest distance between `points`, or `fallback`
/// when there are fewer than two or two coincide.
fn room(points: &BTreeMap<u64, [f64; 2]>, fallback: f64) -> f64 {
    let values: Vec<[f64; 2]> = points.values().copied().collect();
    let smallest = values
        .iter()
        .enumerate()
        .flat_map(|(index, origin)| {
            values[index + 1..].iter().map(move |destination| {
                (origin[0] - destination[0]).hypot(origin[1] - destination[1])
            })
        })
        .fold(f64::INFINITY, f64::min);
    if smallest.is_finite() && smallest > 0.0 {
        smallest * ROOM_FRACTION
    } else {
        fallback
    }
}

/// Lays out one group of siblings around the origin: MDS on the flavoured
/// ones, fitted, aligned to `previous` (when given) on shared ids; the rest
/// on a ring outside, ordered by (name, id).
fn place_siblings(
    siblings: &[Sibling],
    fit: &Fit,
    ring_gap: f64,
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
    match fit {
        Fit::Scale(scale) => {
            for point in &mut points {
                *point = [point[0] * scale, point[1] * scale];
            }
        }
        Fit::Radius(radius) => {
            let farthest = points
                .iter()
                .map(|point| point[0].hypot(point[1]))
                .fold(0.0, f64::max);
            if farthest > 0.0 {
                for point in &mut points {
                    *point = [point[0] * radius / farthest, point[1] * radius / farthest];
                }
            }
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
    let center = if placed.is_empty() {
        [0.0, 0.0]
    } else {
        let count = placed.len() as f64;
        let sum = placed.values().fold([0.0, 0.0], |total, point| {
            [total[0] + point[0], total[1] + point[1]]
        });
        [sum[0] / count, sum[1] / count]
    };
    let radius = placed
        .values()
        .map(|point| (point[0] - center[0]).hypot(point[1] - center[1]))
        .fold(0.0, f64::max)
        + ring_gap;
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
        if warn_flavourless {
            warnings.push(Warning::no_flavour(sibling.id));
        }
    }
    placed
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
}
