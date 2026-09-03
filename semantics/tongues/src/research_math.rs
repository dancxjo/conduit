//! Small linear-algebra implementation used by the bounded research model.

use crate::{Matrix, ResearchModelError};

pub(super) fn normalization(rows: &[Vec<f64>]) -> (Vec<f64>, Vec<f64>) {
    let columns = rows[0].len();
    let mean = (0..columns)
        .map(|column| rows.iter().map(|row| row[column]).sum::<f64>() / rows.len() as f64)
        .collect::<Vec<_>>();
    let scale = (0..columns)
        .map(|column| {
            (rows
                .iter()
                .map(|row| (row[column] - mean[column]).powi(2))
                .sum::<f64>()
                / rows.len() as f64)
                .sqrt()
                .max(1.0)
        })
        .collect();
    (mean, scale)
}

pub(super) fn normalize(rows: &[Vec<f64>], mean: &[f64], scale: &[f64]) -> Vec<Vec<f64>> {
    rows.iter()
        .map(|row| normalize_f64(row, mean, scale))
        .collect()
}

fn normalize_f64(values: &[f64], mean: &[f64], scale: &[f64]) -> Vec<f64> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| (value - mean[index]) / scale[index])
        .collect()
}

pub(super) fn normalize_one(
    values: &[i64],
    mean: &[f64],
    scale: &[f64],
) -> Result<Vec<f64>, ResearchModelError> {
    if values.len() != mean.len() || mean.len() != scale.len() {
        return Err(ResearchModelError::InvalidShape);
    }
    Ok(values
        .iter()
        .enumerate()
        .map(|(index, value)| (*value as f64 - mean[index]) / scale[index])
        .collect())
}

pub(super) fn denormalize(
    values: &[f64],
    mean: &[f64],
    scale: &[f64],
) -> Result<Vec<f64>, ResearchModelError> {
    if values.len() != mean.len() || mean.len() != scale.len() {
        return Err(ResearchModelError::InvalidShape);
    }
    Ok(values
        .iter()
        .enumerate()
        .map(|(index, value)| value * scale[index] + mean[index])
        .collect())
}

pub(super) fn principal_basis(
    rows: &[Vec<f64>],
    dimensions: usize,
    seed: u64,
    steps: usize,
) -> Result<Matrix, ResearchModelError> {
    let columns = rows[0].len();
    let mut covariance = vec![0.0; columns * columns];
    for row in rows {
        for left in 0..columns {
            for right in 0..columns {
                covariance[left * columns + right] += row[left] * row[right] / rows.len() as f64;
            }
        }
    }
    let mut vectors: Vec<Vec<f64>> = Vec::new();
    for component in 0..dimensions {
        let mut vector = (0..columns)
            .map(|index| {
                (((seed + (component * columns + index) as u64 * 7919) % 2001) as f64 - 1000.0)
                    / 1000.0
            })
            .collect::<Vec<_>>();
        for _ in 0..steps {
            let mut next = (0..columns)
                .map(|row| {
                    (0..columns)
                        .map(|column| covariance[row * columns + column] * vector[column])
                        .sum()
                })
                .collect::<Vec<f64>>();
            for prior in &vectors {
                let projection: f64 = next
                    .iter()
                    .zip(prior)
                    .map(|(left, right)| left * right)
                    .sum();
                for (value, basis) in next.iter_mut().zip(prior) {
                    *value -= projection * basis;
                }
            }
            let norm = next.iter().map(|value| value * value).sum::<f64>().sqrt();
            if norm <= f64::EPSILON {
                return Err(ResearchModelError::Singular);
            }
            vector = next.into_iter().map(|value| value / norm).collect();
        }
        vectors.push(vector);
    }
    Matrix::new(
        columns,
        dimensions,
        (0..columns)
            .flat_map(|row| vectors.iter().map(move |vector| vector[row]))
            .collect(),
    )
}

pub(super) fn ridge_regression(
    inputs: &[Vec<f64>],
    outputs: &[Vec<f64>],
    ridge: f64,
) -> Result<Matrix, ResearchModelError> {
    if inputs.is_empty() || inputs.len() != outputs.len() {
        return Err(ResearchModelError::InvalidShape);
    }
    let input_dimensions = inputs[0].len();
    let output_dimensions = outputs[0].len();
    let mut normal = vec![vec![0.0; input_dimensions]; input_dimensions];
    let mut cross = vec![vec![0.0; output_dimensions]; input_dimensions];
    for (input, output) in inputs.iter().zip(outputs) {
        for left in 0..input_dimensions {
            for right in 0..input_dimensions {
                normal[left][right] += input[left] * input[right];
            }
            for target in 0..output_dimensions {
                cross[left][target] += input[left] * output[target];
            }
        }
    }
    for (index, row) in normal.iter_mut().enumerate() {
        row[index] += ridge;
    }
    let inverse = invert(normal)?;
    let values = (0..input_dimensions)
        .flat_map(|row| {
            (0..output_dimensions).map({
                let inverse = &inverse;
                let cross = &cross;
                move |column| {
                    (0..input_dimensions)
                        .map(|inner| inverse[row][inner] * cross[inner][column])
                        .sum()
                }
            })
        })
        .collect();
    Matrix::new(input_dimensions, output_dimensions, values)
}

fn invert(mut value: Vec<Vec<f64>>) -> Result<Vec<Vec<f64>>, ResearchModelError> {
    let size = value.len();
    let mut inverse = vec![vec![0.0; size]; size];
    for (index, row) in inverse.iter_mut().enumerate() {
        row[index] = 1.0;
    }
    for pivot in 0..size {
        let best = (pivot..size)
            .max_by(|left, right| {
                value[*left][pivot]
                    .abs()
                    .total_cmp(&value[*right][pivot].abs())
            })
            .unwrap();
        if value[best][pivot].abs() < 1e-12 {
            return Err(ResearchModelError::Singular);
        }
        value.swap(pivot, best);
        inverse.swap(pivot, best);
        let divisor = value[pivot][pivot];
        for column in 0..size {
            value[pivot][column] /= divisor;
            inverse[pivot][column] /= divisor;
        }
        for row in 0..size {
            if row == pivot {
                continue;
            }
            let factor = value[row][pivot];
            for column in 0..size {
                value[row][column] -= factor * value[pivot][column];
                inverse[row][column] -= factor * inverse[pivot][column];
            }
        }
    }
    Ok(inverse)
}

pub(super) fn multiply(
    rows: &[Vec<f64>],
    matrix: &Matrix,
) -> Result<Vec<Vec<f64>>, ResearchModelError> {
    rows.iter().map(|row| matrix.apply(row)).collect()
}
