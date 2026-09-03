//! Small deterministic shared-latent model over the real PB2007 slice.

use crate::research_math::{
    denormalize, multiply, normalization, normalize, normalize_one, principal_basis,
    ridge_regression,
};
use crate::TrainingUtterance;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const ACOUSTIC_DIMENSIONS: usize = 4;
pub const ARTICULATORY_DIMENSIONS: usize = 6;
pub const LATENT_DIMENSIONS: usize = 2;
pub const TRAINING_WORK_BOUND: u64 = 1_000_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResearchCheckpoint {
    pub architecture: String,
    pub generation: u64,
    pub seed: u64,
    pub acoustic_mean: Vec<f64>,
    pub acoustic_scale: Vec<f64>,
    pub articulation_mean: Vec<f64>,
    pub articulation_scale: Vec<f64>,
    pub acoustic_encoder: Matrix,
    pub articulation_encoder: Matrix,
    pub acoustic_decoder: Matrix,
    pub articulation_decoder: Matrix,
    pub recurrent_dynamics: Matrix,
    pub articulation_residual_scale: Vec<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Matrix {
    pub rows: usize,
    pub columns: usize,
    pub values: Vec<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrainingEvidence {
    pub objective_profile: String,
    pub seed: u64,
    pub steps: u64,
    pub consumed_work_units: u64,
    pub checkpoint_identity: String,
    pub training_examples: usize,
    pub training_frames: usize,
    pub labels_visible_to_trainer: bool,
    pub final_objectives_millionths: ObjectiveMetrics,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ObjectiveMetrics {
    pub acoustic_reconstruction: u64,
    pub articulatory_reconstruction: u64,
    pub acoustic_to_articulatory: u64,
    pub articulatory_to_acoustic: u64,
    pub latent_agreement: u64,
    pub dynamics_prediction: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LatentFrame {
    pub mean: Vec<f64>,
    pub standard_deviation: Vec<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArticulationPosterior {
    pub mean: Vec<f64>,
    pub standard_deviation: Vec<f64>,
    pub alternatives: Vec<Vec<f64>>,
    pub disposition: String,
    pub seed: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResearchModelError {
    EmptyTrainingSplit,
    InvalidShape,
    WorkBoundExceeded,
    Singular,
}

pub fn train_shared_latent(
    utterances: &[TrainingUtterance],
    seed: u64,
) -> Result<(ResearchCheckpoint, TrainingEvidence), ResearchModelError> {
    let training = utterances
        .iter()
        .filter(|value| value.split == "train")
        .collect::<Vec<_>>();
    if training.is_empty() {
        return Err(ResearchModelError::EmptyTrainingSplit);
    }
    validate_shapes(&training)?;
    let acoustic_rows = frames(&training, true);
    let articulation_rows = frames(&training, false);
    let (acoustic_mean, acoustic_scale) = normalization(&acoustic_rows);
    let (articulation_mean, articulation_scale) = normalization(&articulation_rows);
    let acoustic = normalize(&acoustic_rows, &acoustic_mean, &acoustic_scale);
    let articulation = normalize(&articulation_rows, &articulation_mean, &articulation_scale);
    let joint = acoustic
        .iter()
        .zip(&articulation)
        .map(|(left, right)| left.iter().chain(right).copied().collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let basis = principal_basis(&joint, LATENT_DIMENSIONS, seed, 48)?;
    let latent = multiply(&joint, &basis)?;
    let acoustic_encoder = ridge_regression(&acoustic, &latent, 0.001)?;
    let articulation_encoder = ridge_regression(&articulation, &latent, 0.001)?;
    let acoustic_decoder = ridge_regression(&latent, &acoustic, 0.001)?;
    let articulation_decoder = ridge_regression(&latent, &articulation, 0.001)?;
    let (prior, next) = transition_pairs(&training, &latent);
    let recurrent_dynamics = ridge_regression(&prior, &next, 0.001)?;
    let predicted_articulation = multiply(
        &multiply(&acoustic, &acoustic_encoder)?,
        &articulation_decoder,
    )?;
    let articulation_residual_scale = residual_scale(&articulation, &predicted_articulation);
    let checkpoint = ResearchCheckpoint {
        architecture: "tongues/paired-pca-linear-dynamics@1".into(),
        generation: 1,
        seed,
        acoustic_mean,
        acoustic_scale,
        articulation_mean,
        articulation_scale,
        acoustic_encoder,
        articulation_encoder,
        acoustic_decoder,
        articulation_decoder,
        recurrent_dynamics,
        articulation_residual_scale,
    };
    let objectives = measure_objectives(&checkpoint, &training)?;
    let checkpoint_identity = checkpoint.identity()?;
    let work = (joint.len() * (10 * 10 * 48 + 4 * 2 + 6 * 2 + 2 * 10)) as u64;
    if work > TRAINING_WORK_BOUND {
        return Err(ResearchModelError::WorkBoundExceeded);
    }
    let evidence = TrainingEvidence {
        objective_profile: "same+cross-modal+agreement+recurrent@1".into(),
        seed,
        steps: 48,
        consumed_work_units: work,
        checkpoint_identity: hex_identity(checkpoint_identity),
        training_examples: training.len(),
        training_frames: joint.len(),
        labels_visible_to_trainer: false,
        final_objectives_millionths: objectives,
    };
    Ok((checkpoint, evidence))
}

impl ResearchCheckpoint {
    pub fn identity(&self) -> Result<[u8; 32], ResearchModelError> {
        let bytes = serde_json::to_vec(self).map_err(|_| ResearchModelError::InvalidShape)?;
        Ok(Sha256::digest(bytes).into())
    }

    pub fn encode_acoustic(&self, frame: &[i64]) -> Result<LatentFrame, ResearchModelError> {
        let input = normalize_one(frame, &self.acoustic_mean, &self.acoustic_scale)?;
        let mean = self.acoustic_encoder.apply(&input)?;
        Ok(LatentFrame {
            mean,
            standard_deviation: vec![0.08, 0.08],
        })
    }

    pub fn encode_articulation(&self, frame: &[i64]) -> Result<LatentFrame, ResearchModelError> {
        let input = normalize_one(frame, &self.articulation_mean, &self.articulation_scale)?;
        let mean = self.articulation_encoder.apply(&input)?;
        Ok(LatentFrame {
            mean,
            standard_deviation: vec![0.05, 0.05],
        })
    }

    pub fn decode_acoustic(&self, latent: &[f64]) -> Result<Vec<f64>, ResearchModelError> {
        denormalize(
            &self.acoustic_decoder.apply(latent)?,
            &self.acoustic_mean,
            &self.acoustic_scale,
        )
    }

    pub fn decode_articulation(&self, latent: &[f64]) -> Result<Vec<f64>, ResearchModelError> {
        denormalize(
            &self.articulation_decoder.apply(latent)?,
            &self.articulation_mean,
            &self.articulation_scale,
        )
    }

    pub fn infer_articulation(
        &self,
        acoustic: &[i64],
        seed: u64,
    ) -> Result<ArticulationPosterior, ResearchModelError> {
        let latent = self.encode_acoustic(acoustic)?;
        let mean = self.decode_articulation(&latent.mean)?;
        let alternatives = [-1.0, 1.0]
            .iter()
            .enumerate()
            .map(|(sample, direction)| {
                mean.iter()
                    .zip(&self.articulation_residual_scale)
                    .enumerate()
                    .map(|(index, (value, spread))| {
                        let sign = if (seed.wrapping_add((sample * 17 + index) as u64) & 1) == 0 {
                            *direction
                        } else {
                            -*direction
                        };
                        value + sign * spread
                    })
                    .collect()
            })
            .collect();
        Ok(ArticulationPosterior {
            mean,
            standard_deviation: self.articulation_residual_scale.clone(),
            alternatives,
            disposition: "inferred-not-observed".into(),
            seed,
        })
    }

    pub fn next_latent(&self, latent: &[f64]) -> Result<Vec<f64>, ResearchModelError> {
        self.recurrent_dynamics.apply(latent)
    }
}

impl Matrix {
    pub(super) fn new(
        rows: usize,
        columns: usize,
        values: Vec<f64>,
    ) -> Result<Self, ResearchModelError> {
        if rows == 0 || columns == 0 || values.len() != rows * columns {
            return Err(ResearchModelError::InvalidShape);
        }
        Ok(Self {
            rows,
            columns,
            values,
        })
    }

    pub fn apply(&self, input: &[f64]) -> Result<Vec<f64>, ResearchModelError> {
        if input.len() != self.rows {
            return Err(ResearchModelError::InvalidShape);
        }
        Ok((0..self.columns)
            .map(|column| {
                input
                    .iter()
                    .enumerate()
                    .map(|(row, value)| value * self.values[row * self.columns + column])
                    .sum()
            })
            .collect())
    }
}

fn validate_shapes(training: &[&TrainingUtterance]) -> Result<(), ResearchModelError> {
    if training.iter().any(|utterance| {
        utterance.acoustic.len() != utterance.articulation.len()
            || utterance
                .acoustic
                .iter()
                .any(|frame| frame.len() != ACOUSTIC_DIMENSIONS)
            || utterance
                .articulation
                .iter()
                .any(|frame| frame.len() != ARTICULATORY_DIMENSIONS)
    }) {
        return Err(ResearchModelError::InvalidShape);
    }
    Ok(())
}

fn frames(training: &[&TrainingUtterance], acoustic: bool) -> Vec<Vec<f64>> {
    training
        .iter()
        .flat_map(|utterance| {
            if acoustic {
                &utterance.acoustic
            } else {
                &utterance.articulation
            }
        })
        .map(|frame| frame.iter().map(|value| *value as f64).collect())
        .collect()
}

fn transition_pairs(
    training: &[&TrainingUtterance],
    latent: &[Vec<f64>],
) -> (Vec<Vec<f64>>, Vec<Vec<f64>>) {
    let mut prior = Vec::new();
    let mut next = Vec::new();
    let mut offset = 0;
    for utterance in training {
        let count = utterance.acoustic.len();
        for index in 0..count - 1 {
            prior.push(latent[offset + index].clone());
            next.push(latent[offset + index + 1].clone());
        }
        offset += count;
    }
    (prior, next)
}

fn residual_scale(expected: &[Vec<f64>], actual: &[Vec<f64>]) -> Vec<f64> {
    (0..expected[0].len())
        .map(|column| {
            (expected
                .iter()
                .zip(actual)
                .map(|(left, right)| (left[column] - right[column]).powi(2))
                .sum::<f64>()
                / expected.len() as f64)
                .sqrt()
        })
        .collect()
}

fn measure_objectives(
    checkpoint: &ResearchCheckpoint,
    training: &[&TrainingUtterance],
) -> Result<ObjectiveMetrics, ResearchModelError> {
    let mut totals: [f64; 6] = [0.0; 6];
    let mut count = 0;
    for utterance in training {
        let mut prior: Option<Vec<f64>> = None;
        for (acoustic, articulation) in utterance.acoustic.iter().zip(&utterance.articulation) {
            let zx = checkpoint.encode_acoustic(acoustic)?.mean;
            let za = checkpoint.encode_articulation(articulation)?.mean;
            totals[0] += mse_i64(&checkpoint.decode_acoustic(&zx)?, acoustic);
            totals[1] += mse_i64(&checkpoint.decode_articulation(&za)?, articulation);
            totals[2] += mse_i64(&checkpoint.decode_articulation(&zx)?, articulation);
            totals[3] += mse_i64(&checkpoint.decode_acoustic(&za)?, acoustic);
            totals[4] += mse(&zx, &za);
            if let Some(value) = prior {
                totals[5] += mse(&checkpoint.next_latent(&value)?, &zx);
            }
            prior = Some(zx);
            count += 1;
        }
    }
    let scaled = |index: usize| {
        (totals[index] / count as f64 * 1_000_000.0)
            .round()
            .max(0.0) as u64
    };
    Ok(ObjectiveMetrics {
        acoustic_reconstruction: scaled(0),
        articulatory_reconstruction: scaled(1),
        acoustic_to_articulatory: scaled(2),
        articulatory_to_acoustic: scaled(3),
        latent_agreement: scaled(4),
        dynamics_prediction: scaled(5),
    })
}

fn mse_i64(actual: &[f64], expected: &[i64]) -> f64 {
    actual
        .iter()
        .zip(expected)
        .map(|(left, right)| (left - *right as f64).powi(2))
        .sum::<f64>()
        / actual.len() as f64
}
fn mse(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>()
        / left.len() as f64
}
fn hex_identity(identity: [u8; 32]) -> String {
    format!(
        "sha256:{}",
        identity
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}
