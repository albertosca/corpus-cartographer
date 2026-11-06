//! Deterministic small-matrix geometry: a cyclic Jacobi eigen-solver,
//! classical multidimensional scaling to the plane, and the rigid
//! (rotation, optional reflection, translation) alignment of one point set
//! onto another. Sizes here are tens of points, so clarity wins over speed.

const MAXIMUM_SWEEPS: usize = 100;
const CONVERGENCE: f64 = 1e-22;
const TIE_TOLERANCE: f64 = 1e-12;

#[derive(Debug, Clone, PartialEq)]
pub struct Eigenpair {
    pub value: f64,
    pub vector: Vec<f64>,
}

/// Eigenpairs of a symmetric matrix, largest value first (ties by original
/// position), each vector with its largest-magnitude component positive
/// (ties: the first such component), so the result never depends on luck.
pub fn symmetric_eigen(matrix: &[Vec<f64>]) -> Vec<Eigenpair> {
    let size = matrix.len();
    let mut working = matrix.to_vec();
    let mut basis: Vec<Vec<f64>> = (0..size)
        .map(|row| {
            (0..size)
                .map(|column| if row == column { 1.0 } else { 0.0 })
                .collect()
        })
        .collect();
    for _sweep in 0..MAXIMUM_SWEEPS {
        let off_diagonal: f64 = (0..size)
            .flat_map(|row| {
                (0..size)
                    .filter(move |column| *column != row)
                    .map(move |column| (row, column))
            })
            .map(|(row, column)| working[row][column] * working[row][column])
            .sum();
        if off_diagonal < CONVERGENCE {
            break;
        }
        for pivot_row in 0..size {
            for pivot_column in pivot_row + 1..size {
                rotate(&mut working, &mut basis, pivot_row, pivot_column);
            }
        }
    }
    let mut pairs: Vec<(usize, Eigenpair)> = (0..size)
        .map(|index| {
            let mut vector: Vec<f64> = basis.iter().map(|basis_row| basis_row[index]).collect();
            fix_sign(&mut vector);
            (
                index,
                Eigenpair {
                    value: working[index][index],
                    vector,
                },
            )
        })
        .collect();
    pairs.sort_by(|left, right| {
        right
            .1
            .value
            .total_cmp(&left.1.value)
            .then(left.0.cmp(&right.0))
    });
    pairs.into_iter().map(|(_, pair)| pair).collect()
}

/// One Jacobi rotation zeroing `working[pivot_row][pivot_column]`
/// (Numerical Items' tangent form), applied to both sides of `working`
/// and to the columns of `basis`.
fn rotate(working: &mut [Vec<f64>], basis: &mut [Vec<f64>], pivot_row: usize, pivot_column: usize) {
    let pivot = working[pivot_row][pivot_column];
    if pivot.abs() < f64::MIN_POSITIVE {
        return;
    }
    let theta =
        (working[pivot_column][pivot_column] - working[pivot_row][pivot_row]) / (2.0 * pivot);
    let tangent = theta.signum() / (theta.abs() + theta.mul_add(theta, 1.0).sqrt());
    let cosine = 1.0 / tangent.mul_add(tangent, 1.0).sqrt();
    let sine = tangent * cosine;
    for matrix_row in working.iter_mut() {
        let (left, right) = (matrix_row[pivot_row], matrix_row[pivot_column]);
        matrix_row[pivot_row] = cosine * left - sine * right;
        matrix_row[pivot_column] = sine * left + cosine * right;
    }
    // pivot_row < pivot_column, so the two rows live in different halves.
    let (upper_rows, lower_rows) = working.split_at_mut(pivot_column);
    for (top, bottom) in upper_rows[pivot_row]
        .iter_mut()
        .zip(lower_rows[0].iter_mut())
    {
        let (top_value, bottom_value) = (*top, *bottom);
        *top = cosine * top_value - sine * bottom_value;
        *bottom = sine * top_value + cosine * bottom_value;
    }
    for basis_row in basis.iter_mut() {
        let (left, right) = (basis_row[pivot_row], basis_row[pivot_column]);
        basis_row[pivot_row] = cosine * left - sine * right;
        basis_row[pivot_column] = sine * left + cosine * right;
    }
}

fn fix_sign(vector: &mut [f64]) {
    let pivot = (0..vector.len()).fold(0, |best, index| {
        if vector[index].abs() > vector[best].abs() + TIE_TOLERANCE {
            index
        } else {
            best
        }
    });
    if vector.get(pivot).is_some_and(|value| *value < 0.0) {
        for value in vector.iter_mut() {
            *value = -*value;
        }
    }
}

/// Places points in the plane so their distances match `distances` as well
/// as two dimensions allow. One point sits at the origin; two sit on the x
/// axis around it.
pub fn classical_mds(distances: &[Vec<f64>]) -> Vec<[f64; 2]> {
    let size = distances.len();
    match size {
        0 => return Vec::new(),
        1 => return vec![[0.0, 0.0]],
        2 => {
            let half = distances[0][1] / 2.0;
            return vec![[-half, 0.0], [half, 0.0]];
        }
        _ => {}
    }
    let squared: Vec<Vec<f64>> = distances
        .iter()
        .map(|row| row.iter().map(|value| value * value).collect())
        .collect();
    let row_means: Vec<f64> = squared
        .iter()
        .map(|row| row.iter().sum::<f64>() / size as f64)
        .collect();
    let grand_mean = row_means.iter().sum::<f64>() / size as f64;
    let centered: Vec<Vec<f64>> = (0..size)
        .map(|row| {
            (0..size)
                .map(|column| {
                    -0.5 * (squared[row][column] - row_means[row] - row_means[column] + grand_mean)
                })
                .collect()
        })
        .collect();
    let pairs = symmetric_eigen(&centered);
    let scales: Vec<f64> = pairs
        .iter()
        .take(2)
        .map(|pair| pair.value.max(0.0).sqrt())
        .collect();
    (0..size)
        .map(|point| {
            let coordinate = |axis: usize| {
                pairs
                    .get(axis)
                    .map_or(0.0, |pair| pair.vector[point] * scales[axis])
            };
            [coordinate(0), coordinate(1)]
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigidTransform {
    rotation: [[f64; 2]; 2],
    translation: [f64; 2],
}

impl RigidTransform {
    pub fn apply(&self, point: [f64; 2]) -> [f64; 2] {
        [
            self.rotation[0][0] * point[0] + self.rotation[0][1] * point[1] + self.translation[0],
            self.rotation[1][0] * point[0] + self.rotation[1][1] * point[1] + self.translation[1],
        ]
    }
}

fn centroid(points: &[[f64; 2]]) -> [f64; 2] {
    let count = points.len() as f64;
    let sum = points.iter().fold([0.0, 0.0], |total, point| {
        [total[0] + point[0], total[1] + point[1]]
    });
    [sum[0] / count, sum[1] / count]
}

/// The rotation (with a reflection when that fits strictly better) and
/// translation that carry `current` onto `previous` with the least squared
/// error. None with fewer than two pairs, unequal lengths, or all points
/// coincident.
pub fn align(current: &[[f64; 2]], previous: &[[f64; 2]]) -> Option<RigidTransform> {
    if current.len() < 2 || current.len() != previous.len() {
        return None;
    }
    let current_center = centroid(current);
    let previous_center = centroid(previous);
    let (mut same_horizontal, mut horizontal_vertical, mut vertical_horizontal, mut same_vertical) =
        (0.0, 0.0, 0.0, 0.0);
    for (origin, destination) in current.iter().zip(previous) {
        let origin = [origin[0] - current_center[0], origin[1] - current_center[1]];
        let destination = [
            destination[0] - previous_center[0],
            destination[1] - previous_center[1],
        ];
        same_horizontal += origin[0] * destination[0];
        horizontal_vertical += origin[0] * destination[1];
        vertical_horizontal += origin[1] * destination[0];
        same_vertical += origin[1] * destination[1];
    }
    let proper = (same_horizontal + same_vertical).hypot(horizontal_vertical - vertical_horizontal);
    let mirrored =
        (same_horizontal - same_vertical).hypot(horizontal_vertical + vertical_horizontal);
    if proper.max(mirrored) < TIE_TOLERANCE {
        return None;
    }
    let rotation = if mirrored > proper + TIE_TOLERANCE {
        let angle =
            (horizontal_vertical + vertical_horizontal).atan2(same_horizontal - same_vertical);
        [[angle.cos(), angle.sin()], [angle.sin(), -angle.cos()]]
    } else {
        let angle =
            (horizontal_vertical - vertical_horizontal).atan2(same_horizontal + same_vertical);
        [[angle.cos(), -angle.sin()], [angle.sin(), angle.cos()]]
    };
    let without_translation = RigidTransform {
        rotation,
        translation: [0.0, 0.0],
    }
    .apply(current_center);
    let translation = [
        previous_center[0] - without_translation[0],
        previous_center[1] - without_translation[1],
    ];
    Some(RigidTransform {
        rotation,
        translation,
    })
}

/// Rounds to `decimals` places; -0.0 comes out as 0.0 so output bytes do
/// not depend on the sign of a vanishing value.
pub fn round_to(value: f64, decimals: i32) -> f64 {
    let factor = 10_f64.powi(decimals);
    (value * factor).round() / factor + 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(left: f64, right: f64) -> bool {
        (left - right).abs() < 1e-9
    }

    fn distance(left: [f64; 2], right: [f64; 2]) -> f64 {
        (left[0] - right[0]).hypot(left[1] - right[1])
    }

    #[test]
    fn eigenpairs_of_a_small_matrix_are_sorted_and_sign_fixed() {
        let pairs = symmetric_eigen(&[vec![2.0, 1.0], vec![1.0, 2.0]]);
        let root_half = 0.5_f64.sqrt();
        assert!(close(pairs[0].value, 3.0) && close(pairs[1].value, 1.0));
        assert!(close(pairs[0].vector[0], root_half) && close(pairs[0].vector[1], root_half));
        // A tie in magnitude: the first component is the one made positive.
        assert!(close(pairs[1].vector[0], root_half) && close(pairs[1].vector[1], -root_half));
    }

    #[test]
    fn every_eigenpair_satisfies_the_definition() {
        let matrix = vec![
            vec![4.0, 1.0, 0.5, 0.0],
            vec![1.0, 3.0, 0.2, 0.1],
            vec![0.5, 0.2, 2.0, 0.3],
            vec![0.0, 0.1, 0.3, 1.0],
        ];
        for pair in symmetric_eigen(&matrix) {
            for (row, matrix_row) in matrix.iter().enumerate() {
                let product: f64 = matrix_row
                    .iter()
                    .zip(&pair.vector)
                    .map(|(entry, component)| entry * component)
                    .sum();
                assert!(
                    close(product, pair.value * pair.vector[row]),
                    "row {row} of {pair:?}"
                );
            }
        }
    }

    #[test]
    fn mds_recovers_the_distances_of_a_square() {
        let corners = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let distances: Vec<Vec<f64>> = corners
            .iter()
            .map(|origin| {
                corners
                    .iter()
                    .map(|destination| distance(*origin, *destination))
                    .collect()
            })
            .collect();

        let points = classical_mds(&distances);

        for (row, origin) in points.iter().enumerate() {
            for (column, destination) in points.iter().enumerate() {
                assert!(
                    close(distance(*origin, *destination), distances[row][column]),
                    "{row},{column}"
                );
            }
        }
    }

    #[test]
    fn mds_handles_zero_one_and_two_points() {
        assert_eq!(classical_mds(&[]), Vec::<[f64; 2]>::new());
        assert_eq!(classical_mds(&[vec![0.0]]), vec![[0.0, 0.0]]);
        assert_eq!(
            classical_mds(&[vec![0.0, 4.0], vec![4.0, 0.0]]),
            vec![[-2.0, 0.0], [2.0, 0.0]]
        );
    }

    #[test]
    fn mds_of_identical_points_puts_them_together() {
        let points = classical_mds(&[vec![0.0; 3], vec![0.0; 3], vec![0.0; 3]]);
        assert!(
            points
                .iter()
                .all(|point| close(point[0], 0.0) && close(point[1], 0.0)),
            "{points:?}"
        );
    }

    fn rotated(points: &[[f64; 2]], angle: f64, shift: [f64; 2], mirror: bool) -> Vec<[f64; 2]> {
        points
            .iter()
            .map(|point| {
                let flipped = if mirror {
                    [point[0], -point[1]]
                } else {
                    *point
                };
                [
                    flipped[0] * angle.cos() - flipped[1] * angle.sin() + shift[0],
                    flipped[0] * angle.sin() + flipped[1] * angle.cos() + shift[1],
                ]
            })
            .collect()
    }

    #[test]
    fn alignment_recovers_a_rotation_and_a_reflection() {
        let current = [[0.0, 0.0], [3.0, 0.0], [0.0, 1.0], [2.0, 2.0]];
        for mirror in [false, true] {
            let previous = rotated(&current, 0.7, [5.0, -2.0], mirror);
            let transform = align(&current, &previous).unwrap();
            for (point, target) in current.iter().zip(&previous) {
                let moved = transform.apply(*point);
                assert!(
                    close(moved[0], target[0]) && close(moved[1], target[1]),
                    "mirror {mirror}: {moved:?} vs {target:?}"
                );
            }
        }
    }

    #[test]
    fn alignment_needs_two_distinct_points() {
        assert!(align(&[[1.0, 1.0]], &[[2.0, 2.0]]).is_none());
        assert!(align(&[[1.0, 1.0], [1.0, 1.0]], &[[0.0, 0.0], [3.0, 0.0]]).is_none());
        assert!(align(&[[0.0, 0.0], [1.0, 0.0]], &[[0.0, 0.0]]).is_none());
    }

    #[test]
    fn rounding_never_gives_negative_zero() {
        assert_eq!(round_to(-0.000_000_1, 6).to_bits(), 0.0_f64.to_bits());
        assert!(close(round_to(1.234_567_89, 6), 1.234_568));
    }
}
