//! A recipe's flavour is the weighted mean of its distinct Epicure terms'
//! unit vectors, re-normalized, minus the corpus mean: a term weighs more
//! the fewer recipes use it, and what every recipe shares counts for nothing.
//! A group's flavour is the mean direction of its recipes' flavours. Terms
//! outside the vocabulary are ignored and counted.

use std::collections::{BTreeMap, BTreeSet};

use crate::epicure::Epicure;
use crate::export::Recipe;

const MINIMUM_NORM: f64 = 1e-12;
/// A recipe closer than this to the corpus mean keeps its raw flavour. The
/// mean of N identical unit vectors drifts about N * 1e-17 off them, so the
/// absolute `MINIMUM_NORM` would turn that rounding into a flavour from about
/// 100k recipes on; real flavours differ from the mean by tenths.
const SAME_AS_MEAN: f64 = 1e-9;
/// Fewer recipes than this are not centred: one would centre to nothing,
/// two to opposite directions.
const MINIMUM_CENTRED_RECIPES: usize = 3;

#[derive(Debug, Clone, Default)]
pub struct RecipeFlavour {
    pub vector: Option<Vec<f64>>,
    /// Each vocabulary term the recipe uses, with the original ingredient
    /// names that mapped to it (sorted, so input order never shows).
    pub names_by_term: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Debug, Clone, Default)]
pub struct FlavourSpace {
    pub recipes: BTreeMap<u64, RecipeFlavour>,
    /// Distinct terms the export used that the vocabulary does not have.
    pub unknown_terms: usize,
    /// Smoothed inverse document frequency of each term the recipes use:
    /// ln((1 + N) / (1 + df)) + 1, never below 1.
    pub weights: BTreeMap<String, f64>,
    /// The mean of the raw recipe flavours, subtracted from every flavour;
    /// None below `MINIMUM_CENTRED_RECIPES` recipes with flavour.
    pub mean: Option<Vec<f64>>,
}

impl FlavourSpace {
    pub fn build(recipes: &[Recipe], epicure: &Epicure) -> Self {
        let mut unknown = BTreeSet::new();
        let mut flavours = BTreeMap::new();
        for recipe in recipes {
            let mut names_by_term: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
            for ingredient in &recipe.ingredients {
                let Some(term) = &ingredient.epicure_term else {
                    continue;
                };
                if epicure.vector(term).is_none() {
                    unknown.insert(term.clone());
                    continue;
                }
                names_by_term
                    .entry(term.clone())
                    .or_default()
                    .insert(ingredient.name.clone());
            }
            flavours.insert(
                recipe.id,
                RecipeFlavour {
                    vector: None,
                    names_by_term,
                },
            );
        }
        let weights = smoothed_idf(&flavours);
        for flavour in flavours.values_mut() {
            let weighted: Vec<(Vec<f64>, f64)> = flavour
                .names_by_term
                .keys()
                .filter_map(|term| Some((term_vector(epicure, term)?, weights[term])))
                .collect();
            flavour.vector = weighted_direction(
                weighted
                    .iter()
                    .map(|(vector, weight)| (vector.as_slice(), *weight)),
            );
        }
        let mut space = Self {
            recipes: flavours,
            unknown_terms: unknown.len(),
            weights,
            mean: None,
        };
        space.mean = space.raw_mean();
        if let Some(mean) = space.mean.as_deref() {
            for flavour in space.recipes.values_mut() {
                if let Some(raw) = flavour.vector.take() {
                    flavour.vector = Some(centred(raw, mean));
                }
            }
        }
        space
    }

    /// The term's weight; 1 for a term no recipe uses (a theme's own terms).
    pub fn weight(&self, term: &str) -> f64 {
        self.weights.get(term).copied().unwrap_or(1.0)
    }

    /// `raw` minus the corpus mean, re-normalized; `raw` itself when there is
    /// no mean or nothing is left of it.
    pub fn centre(&self, raw: Vec<f64>) -> Vec<f64> {
        match self.mean.as_deref() {
            Some(mean) => centred(raw, mean),
            None => raw,
        }
    }

    pub fn vector(&self, recipe_id: u64) -> Option<&[f64]> {
        self.recipes.get(&recipe_id)?.vector.as_deref()
    }

    /// The mean of the raw flavours, summed in recipe id order.
    fn raw_mean(&self) -> Option<Vec<f64>> {
        let vectors: Vec<&[f64]> = self
            .recipes
            .values()
            .filter_map(|flavour| flavour.vector.as_deref())
            .collect();
        if vectors.len() < MINIMUM_CENTRED_RECIPES {
            return None;
        }
        let mut sum = vec![0.0; vectors[0].len()];
        for vector in &vectors {
            for (total, value) in sum.iter_mut().zip(*vector) {
                *total += value;
            }
        }
        let count = vectors.len() as f64;
        Some(sum.into_iter().map(|total| total / count).collect())
    }
}

/// N counts the recipes with at least one vocabulary term; df the recipes
/// that use the term.
fn smoothed_idf(flavours: &BTreeMap<u64, RecipeFlavour>) -> BTreeMap<String, f64> {
    let mut document_frequency: BTreeMap<String, usize> = BTreeMap::new();
    let mut documents = 0usize;
    for flavour in flavours.values() {
        if flavour.names_by_term.is_empty() {
            continue;
        }
        documents += 1;
        for term in flavour.names_by_term.keys() {
            *document_frequency.entry(term.clone()).or_default() += 1;
        }
    }
    document_frequency
        .into_iter()
        .map(|(term, frequency)| {
            let weight = ((1.0 + documents as f64) / (1.0 + frequency as f64)).ln() + 1.0;
            (term, weight)
        })
        .collect()
}

fn centred(raw: Vec<f64>, mean: &[f64]) -> Vec<f64> {
    let difference: Vec<f64> = raw
        .iter()
        .zip(mean)
        .map(|(value, average)| value - average)
        .collect();
    let norm = difference
        .iter()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt();
    if norm < SAME_AS_MEAN {
        return raw;
    }
    difference.into_iter().map(|value| value / norm).collect()
}

pub fn term_vector(epicure: &Epicure, term: &str) -> Option<Vec<f64>> {
    epicure
        .vector(term)
        .map(|row| row.iter().map(|&value| f64::from(value)).collect())
}

/// The normalized sum of `vectors`; None when there are none or they cancel out.
pub fn mean_direction<'a>(vectors: impl IntoIterator<Item = &'a [f64]>) -> Option<Vec<f64>> {
    weighted_direction(vectors.into_iter().map(|vector| (vector, 1.0)))
}

/// The normalized weighted sum of `pairs`; None when there are none or they cancel out.
pub fn weighted_direction<'a>(
    pairs: impl IntoIterator<Item = (&'a [f64], f64)>,
) -> Option<Vec<f64>> {
    let mut sum: Vec<f64> = Vec::new();
    for (vector, weight) in pairs {
        if sum.is_empty() {
            sum = vec![0.0; vector.len()];
        }
        for (total, value) in sum.iter_mut().zip(vector) {
            *total += weight * value;
        }
    }
    let norm = sum.iter().map(|value| value * value).sum::<f64>().sqrt();
    if norm < MINIMUM_NORM {
        return None;
    }
    Some(sum.into_iter().map(|value| value / norm).collect())
}

/// Cosine of two unit vectors.
pub fn cosine(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(left_value, right_value)| left_value * right_value)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::epicure::Epicure;
    use crate::export::{Ingredient, Recipe, Seal, Status};

    fn space() -> Epicure {
        Epicure::from_rows(
            vec!["garlic".into(), "lime".into()],
            vec![1.0, 0.0, 0.0, 1.0],
            2,
        )
    }

    fn recipe(ingredients: &[(&str, Option<&str>)]) -> Recipe {
        Recipe {
            id: 1,
            title: "Synthetic".into(),
            subtheme_id: None,
            status: Status::Unclassified,
            seal: Seal::Complete,
            ingredients: ingredients
                .iter()
                .map(|(name, term)| Ingredient {
                    name: (*name).into(),
                    epicure_term: term.map(Into::into),
                })
                .collect(),
        }
    }

    #[test]
    fn a_term_used_twice_counts_once() {
        let epicure = space();
        let twice = FlavourSpace::build(
            &[recipe(&[
                ("Alho", Some("garlic")),
                ("Alho picado", Some("garlic")),
                ("Limão", Some("lime")),
            ])],
            &epicure,
        );
        let once = FlavourSpace::build(
            &[recipe(&[("Alho", Some("garlic")), ("Limão", Some("lime"))])],
            &epicure,
        );

        assert_eq!(twice.vector(1), once.vector(1));
        let names: Vec<&str> = twice.recipes[&1].names_by_term["garlic"]
            .iter()
            .map(String::as_str)
            .collect();
        assert_eq!(names, ["Alho", "Alho picado"]);
    }

    #[test]
    fn the_vector_is_the_normalized_mean_of_the_terms() {
        let flavour = FlavourSpace::build(
            &[recipe(&[("Alho", Some("garlic")), ("Limão", Some("lime"))])],
            &space(),
        );
        let vector = flavour.vector(1).unwrap();
        let half_root = 0.5_f64.sqrt();
        assert!(
            (vector[0] - half_root).abs() < 1e-12 && (vector[1] - half_root).abs() < 1e-12,
            "{vector:?}"
        );
    }

    #[test]
    fn an_unknown_term_is_ignored_and_counted_once() {
        let mut second = recipe(&[("Sal", Some("salt"))]);
        second.id = 2;
        let recipes = [
            recipe(&[("Sal", Some("salt")), ("Alho", Some("garlic"))]),
            second,
        ];
        let flavour = FlavourSpace::build(&recipes, &space());
        assert_eq!(flavour.unknown_terms, 1);
        assert!(flavour.vector(1).is_some());
    }

    #[test]
    fn a_recipe_without_terms_has_no_vector() {
        let flavour = FlavourSpace::build(&[recipe(&[("Sal", None)])], &space());
        assert!(flavour.vector(1).is_none());
        assert_eq!(flavour.unknown_terms, 0);
    }

    /// One orthogonal axis per term, so every term vector is a unit basis vector.
    fn axes(terms: &[&str]) -> Epicure {
        let dimension = terms.len();
        let mut values = vec![0.0; dimension * dimension];
        for index in 0..dimension {
            values[index * dimension + index] = 1.0;
        }
        Epicure::from_rows(
            terms.iter().map(|term| (*term).to_string()).collect(),
            values,
            dimension,
        )
    }

    fn numbered(id: u64, terms: &[&str]) -> Recipe {
        let mut numbered = recipe(
            &terms
                .iter()
                .map(|term| (*term, Some(*term)))
                .collect::<Vec<_>>(),
        );
        numbered.id = id;
        numbered
    }

    fn close(left: &[f64], right: &[f64]) -> bool {
        left.iter()
            .zip(right)
            .all(|(left_value, right_value)| (left_value - right_value).abs() < 1e-12)
    }

    const PANTRY: [&str; 10] = [
        "garlic", "salt", "onion", "chili", "mint", "lime", "cilantro", "basil", "pepper", "thyme",
    ];

    /// Recipes 1 and 2 share only pantry staples; 3 and 4 share only lime.
    fn pantry_corpus() -> Vec<Recipe> {
        vec![
            numbered(1, &["garlic", "salt", "onion", "chili"]),
            numbered(2, &["garlic", "salt", "onion", "mint"]),
            numbered(3, &["lime", "cilantro", "garlic"]),
            numbered(4, &["lime", "basil", "salt"]),
            numbered(5, &["garlic", "salt", "onion", "pepper"]),
            numbered(6, &["garlic", "salt", "onion", "thyme"]),
        ]
    }

    #[test]
    fn a_common_term_weighs_less_than_a_rare_one() {
        let flavour = FlavourSpace::build(&pantry_corpus(), &axes(&PANTRY));
        assert!(flavour.weight("garlic") < flavour.weight("lime"));
        assert!(flavour.weight("lime") < flavour.weight("chili"));
        assert!(flavour.weight("garlic") >= 1.0);
        assert!((flavour.weight("never-used") - 1.0).abs() < 1e-12);
    }

    #[test]
    fn sharing_a_rare_term_beats_sharing_only_staples() {
        let recipes = pantry_corpus();
        let epicure = axes(&PANTRY);
        let flavour = FlavourSpace::build(&recipes, &epicure);
        let staples = cosine(flavour.vector(1).unwrap(), flavour.vector(2).unwrap());
        let rare = cosine(flavour.vector(3).unwrap(), flavour.vector(4).unwrap());
        assert!(rare > staples, "rare {rare} staples {staples}");

        // The plain mean of the terms ranks them the other way round.
        let plain = |id: u64| {
            let vectors: Vec<Vec<f64>> = recipes[id as usize - 1]
                .ingredients
                .iter()
                .filter_map(|ingredient| term_vector(&epicure, ingredient.epicure_term.as_ref()?))
                .collect();
            mean_direction(vectors.iter().map(Vec::as_slice)).unwrap()
        };
        assert!(cosine(&plain(1), &plain(2)) > cosine(&plain(3), &plain(4)));
    }

    #[test]
    fn three_or_more_recipes_are_centred_on_their_mean() {
        let flavour = FlavourSpace::build(&pantry_corpus(), &axes(&PANTRY));
        let mean = flavour.mean.as_ref().expect("six recipes are centred");
        let mut centred_sum = vec![0.0; PANTRY.len()];
        for id in 1..=6 {
            for (total, value) in centred_sum.iter_mut().zip(flavour.vector(id).unwrap()) {
                *total += value / 6.0;
            }
        }
        let norm = |vector: &[f64]| vector.iter().map(|value| value * value).sum::<f64>().sqrt();
        assert!(
            norm(&centred_sum) < norm(mean) / 4.0,
            "centred mean {} raw mean {}",
            norm(&centred_sum),
            norm(mean)
        );
    }

    #[test]
    fn one_or_two_recipes_are_weighted_but_not_centred() {
        let epicure = space();
        let recipes = [numbered(1, &["garlic", "lime"]), numbered(2, &["garlic"])];
        let flavour = FlavourSpace::build(&recipes, &epicure);
        assert!(flavour.mean.is_none());

        // N = 2: garlic is in both (weight 1), lime in one (weight ln(3/2) + 1).
        let lime_weight = 1.5_f64.ln() + 1.0;
        let length = (1.0 + lime_weight * lime_weight).sqrt();
        assert!(
            close(
                flavour.vector(1).unwrap(),
                &[1.0 / length, lime_weight / length]
            ),
            "{:?}",
            flavour.vector(1)
        );
        assert!(close(flavour.vector(2).unwrap(), &[1.0, 0.0]));
    }

    #[test]
    fn a_recipe_equal_to_the_mean_keeps_its_raw_flavour() {
        let epicure = space();
        let recipes = [
            numbered(1, &["garlic", "lime"]),
            numbered(2, &["garlic", "lime"]),
            numbered(3, &["garlic", "lime"]),
        ];
        let flavour = FlavourSpace::build(&recipes, &epicure);
        let half_root = 0.5_f64.sqrt();
        for id in 1..=3 {
            assert!(
                close(flavour.vector(id).unwrap(), &[half_root, half_root]),
                "{:?}",
                flavour.vector(id)
            );
        }
    }

    #[test]
    fn centring_skips_recipes_without_flavour() {
        // Two recipes with flavour and one without: still too few to centre.
        let epicure = space();
        let mut empty = recipe(&[("Sal", None)]);
        empty.id = 3;
        let recipes = [numbered(1, &["garlic"]), numbered(2, &["lime"]), empty];
        let flavour = FlavourSpace::build(&recipes, &epicure);
        assert!(flavour.mean.is_none());
        assert!(close(flavour.vector(1).unwrap(), &[1.0, 0.0]));
    }

    #[test]
    fn centre_leaves_a_vector_alone_without_a_mean_or_when_it_is_the_mean() {
        let epicure = space();
        let few = FlavourSpace::build(&[numbered(1, &["garlic"])], &epicure);
        assert_eq!(few.centre(vec![0.6, 0.8]), vec![0.6, 0.8]);

        let same = FlavourSpace::build(
            &[
                numbered(1, &["garlic"]),
                numbered(2, &["garlic"]),
                numbered(3, &["garlic"]),
            ],
            &epicure,
        );
        assert_eq!(same.centre(vec![1.0, 0.0]), vec![1.0, 0.0]);
    }

    #[test]
    fn centre_keeps_a_vector_that_differs_from_the_mean_only_by_rounding() {
        // A large corpus of identical recipes sits a rounding error off its
        // own mean; that error must not become the flavour.
        let epicure = space();
        let same = FlavourSpace::build(
            &[
                numbered(1, &["garlic"]),
                numbered(2, &["garlic"]),
                numbered(3, &["garlic"]),
            ],
            &epicure,
        );
        assert_eq!(same.centre(vec![1.0, 1e-10]), vec![1.0, 1e-10]);
    }

    #[test]
    fn opposite_vectors_have_no_direction() {
        assert!(mean_direction([[1.0, 0.0].as_slice(), [-1.0, 0.0].as_slice()]).is_none());
        assert!(mean_direction(std::iter::empty::<&[f64]>()).is_none());
    }
}
