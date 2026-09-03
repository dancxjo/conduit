//! Small deterministic numerical helpers for the bounded analysis.

use crate::AnalysisError;

pub(super) fn library(z: &[f64]) -> Vec<f64> {
    vec![1.0, z[0], z[1], z[0] * z[1], z[0] * z[0], z[1] * z[1]]
}

pub(super) fn nearest(value: &[f64], centers: &[Vec<f64>; 3]) -> usize {
    centers
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| squared(value, a).total_cmp(&squared(value, b)))
        .map(|(index, _)| index)
        .unwrap_or(0)
}

pub(super) fn plv(phases: &[f64]) -> f64 {
    let (x, y) = phases.iter().fold((0.0, 0.0), |(x, y), phase| {
        (x + phase.cos(), y + phase.sin())
    });
    (x.hypot(y) / phases.len() as f64).clamp(0.0, 1.0)
}

pub(super) fn ridge(x: &[Vec<f64>], y: &[f64], lambda: f64) -> Result<Vec<f64>, AnalysisError> {
    let dimensions = x[0].len();
    let mut augmented = vec![vec![0.0; dimensions + 1]; dimensions];
    for row in 0..dimensions {
        for column in 0..dimensions {
            augmented[row][column] = x
                .iter()
                .map(|value| value[row] * value[column])
                .sum::<f64>()
                + if row == column { lambda } else { 0.0 };
        }
        augmented[row][dimensions] = x
            .iter()
            .zip(y)
            .map(|(value, target)| value[row] * target)
            .sum();
    }
    for pivot in 0..dimensions {
        let best = (pivot..dimensions)
            .max_by(|left, right| {
                augmented[*left][pivot]
                    .abs()
                    .total_cmp(&augmented[*right][pivot].abs())
            })
            .ok_or(AnalysisError::InvalidData)?;
        augmented.swap(pivot, best);
        if augmented[pivot][pivot].abs() < 1e-12 {
            return Err(AnalysisError::InvalidData);
        }
        let pivot_values = augmented[pivot].clone();
        for row in pivot + 1..dimensions {
            let factor = augmented[row][pivot] / augmented[pivot][pivot];
            for (value, pivot_value) in augmented[row][pivot..]
                .iter_mut()
                .zip(&pivot_values[pivot..])
            {
                *value -= factor * pivot_value;
            }
        }
    }
    let mut result = vec![0.0; dimensions];
    for row in (0..dimensions).rev() {
        result[row] = (augmented[row][dimensions]
            - (row + 1..dimensions)
                .map(|column| augmented[row][column] * result[column])
                .sum::<f64>())
            / augmented[row][row];
    }
    Ok(result)
}

pub(super) fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter().zip(right).map(|(a, b)| a * b).sum()
}

pub(super) fn millionths(value: f64) -> u64 {
    (value.max(0.0) * 1_000_000.0).round() as u64
}

pub(super) fn signed_millionths(value: f64) -> i64 {
    (value * 1_000_000.0).round() as i64
}

fn squared(left: &[f64], right: &[f64]) -> f64 {
    left.iter().zip(right).map(|(a, b)| (a - b).powi(2)).sum()
}
