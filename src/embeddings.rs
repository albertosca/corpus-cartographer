//! Reads Kaikaku/embeddings-core the way its Python loader does: one f32 tensor
//! named "embeddings" whose rows follow vocab.json, each row divided by
//! max(norm, 1e-9) so cosine similarity is a dot product.

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::Path;

use safetensors::{Dtype, SafeTensors};

const MINIMUM_NORM: f32 = 1e-9;
const TENSOR_NAME: &str = "embeddings";

#[derive(Debug)]
pub struct EmbeddingsError(pub String);

impl fmt::Display for EmbeddingsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "embeddings: {}", self.0)
    }
}

impl std::error::Error for EmbeddingsError {}

pub struct Embeddings {
    dimension: usize,
    terms: Vec<String>,
    index_by_term: HashMap<String, usize>,
    unit_rows: Vec<f32>,
    one_axis_per_term: bool,
}

impl Embeddings {
    pub fn load(directory: &Path) -> Result<Self, EmbeddingsError> {
        let vocabulary_path = directory.join("vocab.json");
        let vocabulary_text = fs::read_to_string(&vocabulary_path)
            .map_err(|error| EmbeddingsError(format!("{}: {error}", vocabulary_path.display())))?;
        let index_by_term: HashMap<String, usize> = serde_json::from_str(&vocabulary_text)
            .map_err(|error| EmbeddingsError(format!("vocab.json: {error}")))?;
        let mut slots: Vec<Option<String>> = vec![None; index_by_term.len()];
        for (term, &index) in &index_by_term {
            let slot = slots.get_mut(index).ok_or_else(|| {
                EmbeddingsError(format!("vocab.json: index {index} is out of range"))
            })?;
            if slot.replace(term.clone()).is_some() {
                return Err(EmbeddingsError(format!(
                    "vocab.json: index {index} is used twice"
                )));
            }
        }
        let terms: Vec<String> = slots.into_iter().map(Option::unwrap_or_default).collect();

        let weights_path = directory.join("embeddings.safetensors");
        let bytes = fs::read(&weights_path)
            .map_err(|error| EmbeddingsError(format!("{}: {error}", weights_path.display())))?;
        let tensors = SafeTensors::deserialize(&bytes)
            .map_err(|error| EmbeddingsError(format!("embeddings.safetensors: {error}")))?;
        let view = tensors
            .tensor(TENSOR_NAME)
            .map_err(|error| EmbeddingsError(format!("embeddings.safetensors: {error}")))?;
        if view.dtype() != Dtype::F32 {
            return Err(EmbeddingsError(format!(
                "expected f32 weights, found {:?}",
                view.dtype()
            )));
        }
        let &[rows, dimension] = view.shape() else {
            return Err(EmbeddingsError(format!(
                "expected a 2-d tensor, found shape {:?}",
                view.shape()
            )));
        };
        if rows != terms.len() || dimension == 0 {
            return Err(EmbeddingsError(format!(
                "the tensor has {rows} rows of {dimension} values but vocab.json has {} terms",
                terms.len()
            )));
        }
        let values = view
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|chunk| f32::from_le_bytes(*chunk))
            .collect();
        Ok(Self::from_rows(terms, values, dimension))
    }

    /// Builds a space from raw rows (`dimension` values per term, in term
    /// order) and normalizes each row. Tests use it for small spaces.
    pub fn from_rows(terms: Vec<String>, values: Vec<f32>, dimension: usize) -> Self {
        assert!(dimension > 0, "dimension must be positive");
        assert_eq!(
            values.len(),
            terms.len() * dimension,
            "one row of `dimension` values per term"
        );
        let mut unit_rows = values;
        for row in unit_rows.chunks_exact_mut(dimension) {
            let norm = row
                .iter()
                .map(|value| value * value)
                .sum::<f32>()
                .sqrt()
                .max(MINIMUM_NORM);
            for value in row.iter_mut() {
                *value /= norm;
            }
        }
        let index_by_term = terms
            .iter()
            .enumerate()
            .map(|(index, term)| (term.clone(), index))
            .collect();
        Self {
            dimension,
            terms,
            index_by_term,
            unit_rows,
            one_axis_per_term: false,
        }
    }

    /// A space with no embeddings behind it: each term is its own axis, so
    /// two terms are never similar and only shared terms bring items close.
    /// `terms` must be distinct; the axes follow their order.
    pub fn one_axis_per_term(terms: Vec<String>) -> Self {
        let dimension = terms.len().max(1);
        let mut values = vec![0.0_f32; terms.len() * dimension];
        for index in 0..terms.len() {
            values[index * dimension + index] = 1.0;
        }
        Self {
            one_axis_per_term: true,
            ..Self::from_rows(terms, values, dimension)
        }
    }

    /// True for a space built by `one_axis_per_term`, where "the closest
    /// pair of terms" carries no information.
    pub fn is_one_axis_per_term(&self) -> bool {
        self.one_axis_per_term
    }

    pub fn dimension(&self) -> usize {
        self.dimension
    }

    pub fn len(&self) -> usize {
        self.terms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    pub fn vector(&self, term: &str) -> Option<&[f32]> {
        let index = *self.index_by_term.get(term)?;
        Some(&self.unit_rows[index * self.dimension..(index + 1) * self.dimension])
    }

    /// The `count` terms most similar to `term`, most similar first, ties by
    /// vocabulary index; the term itself is never its own neighbour.
    pub fn neighbors(&self, term: &str, count: usize) -> Option<Vec<(String, f32)>> {
        let query_index = *self.index_by_term.get(term)?;
        let query = self.vector(term)?;
        let mut scored: Vec<(usize, f32)> = self
            .unit_rows
            .chunks_exact(self.dimension)
            .enumerate()
            .filter(|(index, _)| *index != query_index)
            .map(|(index, row)| {
                (
                    index,
                    row.iter()
                        .zip(query)
                        .map(|(left, right)| left * right)
                        .sum(),
                )
            })
            .collect();
        scored.sort_by(|left, right| right.1.total_cmp(&left.1).then(left.0.cmp(&right.0)));
        Some(
            scored
                .into_iter()
                .take(count)
                .map(|(index, similarity)| (self.terms[index].clone(), similarity))
                .collect(),
        )
    }
}
