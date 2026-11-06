//! Flavour links between recipes: cosine between recipe vectors, at most
//! `max_links` per recipe above `floor`, other territories first. Each link
//! says why, in the ingredient names the cook used. A pair the owner hid is
//! dropped before the quota cut, so its slot goes to the next candidate.

use serde::Serialize;

use crate::epicure::Epicure;
use crate::export::{EngineExport, Status};
use crate::vectors::{FlavourSpace, RecipeFlavour, cosine, term_vector};

pub const DEFAULT_MAX_LINKS: usize = 3;
/// The owner's floor on the rarity-weighted, centred scale, chosen on the
/// real corpus on 2026-11-01 (the app passes the same value explicitly).
pub const DEFAULT_FLOOR: f64 = 0.4;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct LinkOptions {
    pub max_links: usize,
    pub floor: f64,
}

impl Default for LinkOptions {
    fn default() -> Self {
        Self {
            max_links: DEFAULT_MAX_LINKS,
            floor: DEFAULT_FLOOR,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SharedTerm {
    pub term: String,
    pub source_names: Vec<String>,
    pub target_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClosestPair {
    pub source_term: String,
    pub source_name: String,
    pub target_term: String,
    pub target_name: String,
    pub similarity: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Link {
    pub source: u64,
    pub target: u64,
    pub similarity: f64,
    pub crosses_theme: bool,
    pub shared: Vec<SharedTerm>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closest: Option<ClosestPair>,
}

struct Participant<'a> {
    id: u64,
    territory: Option<u64>,
    vector: &'a [f64],
    flavour: &'a RecipeFlavour,
}

pub fn find_links(
    export: &EngineExport,
    space: &FlavourSpace,
    epicure: &Epicure,
    options: &LinkOptions,
) -> Vec<Link> {
    let mut participants: Vec<Participant> = export
        .recipes
        .iter()
        .filter(|recipe| recipe.status == Status::Assigned)
        .filter_map(|recipe| {
            let flavour = space.recipes.get(&recipe.id)?;
            let vector = flavour.vector.as_deref()?;
            let territory = recipe
                .subtheme_id
                .and_then(|subtheme_id| export.top_level_of(subtheme_id));
            Some(Participant {
                id: recipe.id,
                territory,
                vector,
                flavour,
            })
        })
        .collect();
    participants.sort_by_key(|participant| participant.id);

    let rejected = export.rejected_set();
    let mut links = Vec::new();
    for source in &participants {
        let mut candidates: Vec<(&Participant, f64, bool)> = participants
            .iter()
            .filter(|target| target.id != source.id)
            .filter(|target| {
                !rejected.contains(&(source.id.min(target.id), source.id.max(target.id)))
            })
            .map(|target| {
                (
                    target,
                    cosine(source.vector, target.vector),
                    target.territory != source.territory,
                )
            })
            .filter(|(_, similarity, _)| *similarity >= options.floor)
            .collect();
        candidates.sort_by(|left, right| {
            right
                .2
                .cmp(&left.2)
                .then(right.1.total_cmp(&left.1))
                .then(left.0.id.cmp(&right.0.id))
        });
        for (target, similarity, crosses_theme) in candidates.into_iter().take(options.max_links) {
            let shared = shared_terms(space, source.flavour, target.flavour);
            let closest = if shared.is_empty() && !epicure.is_one_axis_per_term() {
                closest_pair(source.flavour, target.flavour, epicure)
            } else {
                None
            };
            links.push(Link {
                source: source.id,
                target: target.id,
                similarity,
                crosses_theme,
                shared,
                closest,
            });
        }
    }
    links
}

/// The terms both recipes use, rarest first (highest weight, then name), so
/// the reason a link exists reads before the staples.
fn shared_terms(
    space: &FlavourSpace,
    source: &RecipeFlavour,
    target: &RecipeFlavour,
) -> Vec<SharedTerm> {
    let mut shared: Vec<SharedTerm> = source
        .names_by_term
        .iter()
        .filter_map(|(term, source_names)| {
            let target_names = target.names_by_term.get(term)?;
            Some(SharedTerm {
                term: term.clone(),
                source_names: source_names.iter().cloned().collect(),
                target_names: target_names.iter().cloned().collect(),
            })
        })
        .collect();
    shared.sort_by(|left, right| {
        space
            .weight(&right.term)
            .total_cmp(&space.weight(&left.term))
            .then_with(|| left.term.cmp(&right.term))
    });
    shared
}

fn closest_pair(
    source: &RecipeFlavour,
    target: &RecipeFlavour,
    epicure: &Epicure,
) -> Option<ClosestPair> {
    let mut best: Option<(f64, &String, &String)> = None;
    for source_term in source.names_by_term.keys() {
        let Some(source_vector) = term_vector(epicure, source_term) else {
            continue;
        };
        for target_term in target.names_by_term.keys() {
            let Some(target_vector) = term_vector(epicure, target_term) else {
                continue;
            };
            let similarity = cosine(&source_vector, &target_vector);
            if best.is_none_or(|(best_similarity, _, _)| similarity > best_similarity) {
                best = Some((similarity, source_term, target_term));
            }
        }
    }
    let (similarity, source_term, target_term) = best?;
    Some(ClosestPair {
        source_term: source_term.clone(),
        source_name: source.names_by_term[source_term].iter().next()?.clone(),
        target_term: target_term.clone(),
        target_name: target.names_by_term[target_term].iter().next()?.clone(),
        similarity,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::epicure::Epicure;
    use crate::export::parse;
    use crate::vectors::FlavourSpace;

    fn epicure() -> Epicure {
        Epicure::from_rows(
            vec![
                "beef".into(),
                "garlic".into(),
                "lime".into(),
                "onion".into(),
            ],
            vec![1.0, 0.0, 0.0, 0.8, 0.6, 0.0, 0.0, 0.0, 1.0, 0.7, 0.7, 0.1],
            3,
        )
    }

    const THEMES: &str = r#"[
        {"id":1,"parent_id":null,"name":"A","epicure_terms":[]},
        {"id":2,"parent_id":null,"name":"B","epicure_terms":[]},
        {"id":11,"parent_id":1,"name":"A1","epicure_terms":[]},
        {"id":21,"parent_id":2,"name":"B1","epicure_terms":[]}
    ]"#;

    fn recipe(id: u64, subtheme: Option<u64>, ingredients: &[(&str, &str)]) -> String {
        let (subtheme, status) = subtheme.map_or(("null".to_string(), "needs_owner"), |id| {
            (id.to_string(), "assigned")
        });
        let ingredients: Vec<String> = ingredients
            .iter()
            .map(|(name, term)| format!(r#"{{"name":"{name}","epicure_term":"{term}"}}"#))
            .collect();
        format!(
            r#"{{"id":{id},"title":"Synthetic","subtheme_id":{subtheme},"status":"{status}","seal":"complete","ingredients":[{}]}}"#,
            ingredients.join(",")
        )
    }

    /// Six unassigned lime recipes: they move the corpus mean towards lime
    /// but never link, so a selection test on a handful of single-term
    /// recipes keeps them apart the way their terms are, instead of
    /// centring three recipes into opposite directions.
    fn with_ballast(recipes: &[String]) -> Vec<String> {
        let mut weighted = recipes.to_vec();
        weighted.extend((100..106).map(|id| recipe(id, None, &[("Limão", "lime")])));
        weighted
    }

    fn links_of(recipes: &[String], options: &LinkOptions) -> Vec<Link> {
        links_rejecting(recipes, "[]", options)
    }

    fn links_rejecting(
        recipes: &[String],
        rejected_pairs: &str,
        options: &LinkOptions,
    ) -> Vec<Link> {
        let text = format!(
            r#"{{"schema":"recipes-cartographer/engine-export","version":2,"taxonomy_version":1,"themes":{THEMES},"recipes":[{}],"previous":null,"rejected_pairs":{rejected_pairs}}}"#,
            recipes.join(",")
        );
        let export = parse(&text).unwrap();
        let epicure = epicure();
        let space = FlavourSpace::build(&export.recipes, &epicure);
        find_links(&export, &space, &epicure, options)
    }

    fn targets_of(links: &[Link], source: u64) -> Vec<u64> {
        links
            .iter()
            .filter(|link| link.source == source)
            .map(|link| link.target)
            .collect()
    }

    #[test]
    fn the_default_floor_is_the_one_the_owner_chose_on_the_weighted_scale() {
        assert_eq!(LinkOptions::default().floor, 0.4);
    }

    #[test]
    fn cross_theme_comes_before_a_closer_same_theme_recipe() {
        let recipes = with_ballast(&[
            recipe(1, Some(11), &[("Picanha", "beef")]),
            recipe(2, Some(11), &[("Fraldinha", "beef")]),
            recipe(3, Some(21), &[("Alho", "garlic")]),
        ]);
        let links = links_of(
            &recipes,
            &LinkOptions {
                max_links: 1,
                floor: 0.5,
            },
        );
        assert_eq!(targets_of(&links, 1), [3]);
        assert!(links[0].crosses_theme);
    }

    #[test]
    fn the_quota_fills_with_same_theme_and_the_floor_cuts() {
        let recipes = with_ballast(&[
            recipe(1, Some(11), &[("Picanha", "beef")]),
            recipe(2, Some(11), &[("Fraldinha", "beef")]),
            recipe(3, Some(21), &[("Alho", "garlic")]),
            recipe(4, Some(21), &[("Limão", "lime")]),
        ]);
        let links = links_of(
            &recipes,
            &LinkOptions {
                max_links: 3,
                floor: 0.5,
            },
        );
        assert_eq!(targets_of(&links, 1), [3, 2], "lime is below the floor");
    }

    #[test]
    fn ties_go_to_the_lower_target_id() {
        let recipes = [
            recipe(1, Some(11), &[("Picanha", "beef")]),
            recipe(5, Some(21), &[("Acém", "beef")]),
            recipe(4, Some(21), &[("Cupim", "beef")]),
        ];
        assert_eq!(
            targets_of(
                &links_of(
                    &recipes,
                    &LinkOptions {
                        max_links: 1,
                        floor: 0.5
                    }
                ),
                1
            ),
            [4]
        );
    }

    #[test]
    fn vectorless_and_unassigned_recipes_get_no_links() {
        let recipes = [
            recipe(1, Some(11), &[("Picanha", "beef")]),
            recipe(2, None, &[("Picanha", "beef")]),
            recipe(3, Some(21), &[]),
            recipe(4, Some(21), &[("Sal", "salt")]),
        ];
        let links = links_of(
            &recipes,
            &LinkOptions {
                max_links: 3,
                floor: -1.0,
            },
        );
        assert!(links.is_empty(), "{links:?}");
    }

    #[test]
    fn shared_terms_carry_each_side_s_original_names() {
        let recipes = [
            recipe(1, Some(11), &[("Alho", "garlic"), ("Picanha", "beef")]),
            recipe(
                2,
                Some(21),
                &[("Alho picado", "garlic"), ("Dente de alho", "garlic")],
            ),
        ];
        let link = links_of(&recipes, &LinkOptions::default())
            .into_iter()
            .find(|link| link.source == 1)
            .unwrap();
        assert_eq!(link.shared.len(), 1);
        assert_eq!(link.shared[0].term, "garlic");
        assert_eq!(link.shared[0].source_names, ["Alho"]);
        assert_eq!(
            link.shared[0].target_names,
            ["Alho picado", "Dente de alho"]
        );
        assert!(link.closest.is_none());
    }

    #[test]
    fn shared_terms_list_the_rarest_first() {
        // Garlic is in every recipe, lime only in these two.
        let recipes = [
            recipe(1, Some(11), &[("Alho", "garlic"), ("Limão", "lime")]),
            recipe(2, Some(21), &[("Alho", "garlic"), ("Limão taiti", "lime")]),
            recipe(3, Some(21), &[("Alho", "garlic"), ("Picanha", "beef")]),
        ];
        let link = links_of(
            &recipes,
            &LinkOptions {
                max_links: 3,
                floor: -1.0,
            },
        )
        .into_iter()
        .find(|link| link.source == 1 && link.target == 2)
        .unwrap();
        let terms: Vec<&str> = link
            .shared
            .iter()
            .map(|shared| shared.term.as_str())
            .collect();
        assert_eq!(terms, ["lime", "garlic"]);
    }

    #[test]
    fn without_shared_terms_the_closest_pair_explains_the_link() {
        let recipes = [
            recipe(1, Some(11), &[("Alho", "garlic")]),
            recipe(2, Some(21), &[("Cebola", "onion")]),
        ];
        let link = links_of(&recipes, &LinkOptions::default())
            .into_iter()
            .find(|link| link.source == 1)
            .unwrap();
        let closest = link.closest.unwrap();
        assert_eq!(
            (closest.source_name.as_str(), closest.target_name.as_str()),
            ("Alho", "Cebola")
        );
        assert_eq!(link.shared, Vec::<SharedTerm>::new());
    }

    #[test]
    fn a_hidden_pair_gives_its_slot_to_the_next_candidate_other_territories_first() {
        let recipes = with_ballast(&[
            recipe(1, Some(11), &[("Picanha", "beef")]),
            recipe(2, Some(11), &[("Fraldinha", "beef")]),
            recipe(3, Some(21), &[("Alho", "garlic")]),
            recipe(4, Some(21), &[("Cebola", "onion")]),
        ]);
        let options = LinkOptions {
            max_links: 2,
            floor: 0.6,
        };
        assert_eq!(targets_of(&links_of(&recipes, &options), 1), [3, 4]);
        let links = links_rejecting(&recipes, "[[1,3]]", &options);
        assert_eq!(targets_of(&links, 1), [4, 2]);
        assert_eq!(
            targets_of(&links, 3),
            [2, 4],
            "the pair is hidden from both ends"
        );
    }

    #[test]
    fn with_no_other_candidate_above_the_floor_a_recipe_keeps_fewer_links() {
        let recipes = [
            recipe(1, Some(11), &[("Picanha", "beef")]),
            recipe(2, Some(11), &[("Fraldinha", "beef")]),
            recipe(5, Some(21), &[("Limão", "lime")]),
        ];
        let links = links_rejecting(
            &recipes,
            "[[1,2]]",
            &LinkOptions {
                max_links: 3,
                floor: 0.6,
            },
        );
        assert_eq!(targets_of(&links, 1), Vec::<u64>::new());
        assert_eq!(targets_of(&links, 2), Vec::<u64>::new());
    }
}
