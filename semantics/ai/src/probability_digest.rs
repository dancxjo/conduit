//! Canonical identities for finite probabilistic claims.

use alloc::vec::Vec;
use conduit_core::semantic_digest;

use crate::{
    DrawRelationship, LogProbability, LogScoreKind, MeanCovariance, MeanVariance,
    ProbabilisticDisposition, ProbabilityRefusal, ProbabilitySample, ProbabilitySampleSet,
    RandomnessProfile, StochasticProvenance, TrajectoryAlternatives, WeightedSamples,
};

impl StochasticProvenance {
    pub fn semantic_digest(&self) -> Result<[u8; 32], ProbabilityRefusal> {
        self.validate()?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.model_artifact_identity);
        push_optional_digest(&mut bytes, self.checkpoint_identity);
        bytes.extend_from_slice(&self.query_identity);
        match &self.randomness {
            RandomnessProfile::Deterministic => bytes.push(0),
            RandomnessProfile::ExplicitSeed(seed) => {
                bytes.push(1);
                bytes.extend_from_slice(&seed.to_le_bytes());
            }
            RandomnessProfile::ProviderChosen { seed, nonce } => {
                bytes.push(2);
                bytes.extend_from_slice(&seed.to_le_bytes());
                push_text(&mut bytes, nonce);
            }
        }
        match &self.draws {
            DrawRelationship::Independent => bytes.push(0),
            DrawRelationship::Correlated { profile } => {
                bytes.push(1);
                push_text(&mut bytes, profile);
            }
        }
        Ok(semantic_digest(
            "probability/stochastic-provenance@1",
            &bytes,
        ))
    }
}

impl ProbabilitySample {
    pub fn semantic_digest(&self) -> Result<[u8; 32], ProbabilityRefusal> {
        self.validate()?;
        probabilistic_digest(
            "probability/sample@1",
            &[self
                .value
                .semantic_digest()
                .map_err(|_| ProbabilityRefusal::InvalidSample)?],
            &self.provenance,
            &self.disposition,
            &[],
        )
    }
}

impl ProbabilitySampleSet {
    pub fn semantic_digest(&self) -> Result<[u8; 32], ProbabilityRefusal> {
        self.validate()?;
        probabilistic_digest(
            "probability/samples@1",
            &tensor_digests(&self.alternatives)?,
            &self.provenance,
            &self.disposition,
            &[],
        )
    }
}

impl WeightedSamples {
    pub fn semantic_digest(&self) -> Result<[u8; 32], ProbabilityRefusal> {
        self.validate()?;
        let mut extra = Vec::new();
        for weight in &self.weights {
            extra.extend_from_slice(&weight.to_le_bytes());
        }
        probabilistic_digest(
            "probability/weighted-samples@1",
            &tensor_digests(&self.alternatives)?,
            &self.provenance,
            &self.disposition,
            &extra,
        )
    }
}

impl MeanVariance {
    pub fn semantic_digest(&self) -> Result<[u8; 32], ProbabilityRefusal> {
        self.validate()?;
        probabilistic_digest(
            "probability/mean-variance@1",
            &[
                self.mean
                    .semantic_digest()
                    .map_err(|_| ProbabilityRefusal::InvalidSample)?,
                self.variance
                    .semantic_digest()
                    .map_err(|_| ProbabilityRefusal::InvalidVariance)?,
            ],
            &self.provenance,
            &self.disposition,
            &[],
        )
    }
}

impl MeanCovariance {
    pub fn semantic_digest(&self) -> Result<[u8; 32], ProbabilityRefusal> {
        self.validate()?;
        probabilistic_digest(
            "probability/mean-covariance@1",
            &[
                self.mean
                    .semantic_digest()
                    .map_err(|_| ProbabilityRefusal::InvalidSample)?,
                self.covariance
                    .semantic_digest()
                    .map_err(|_| ProbabilityRefusal::InvalidCovariance)?,
            ],
            &self.provenance,
            &self.disposition,
            &[],
        )
    }
}

impl LogProbability {
    pub fn semantic_digest(&self) -> Result<[u8; 32], ProbabilityRefusal> {
        self.validate()?;
        let mut extra = Vec::new();
        extra.extend_from_slice(&self.natural_log_millionths.to_le_bytes());
        extra.push(match self.score_kind {
            LogScoreKind::ProbabilityMass => 0,
            LogScoreKind::Density => 1,
            LogScoreKind::UnnormalizedScore => 2,
        });
        extra.extend_from_slice(&self.support_identity);
        probabilistic_digest(
            "probability/log-score@1",
            &[],
            &self.provenance,
            &self.disposition,
            &extra,
        )
    }
}

impl TrajectoryAlternatives {
    pub fn semantic_digest(&self) -> Result<[u8; 32], ProbabilityRefusal> {
        self.validate()?;
        let signals = self
            .plausible_alternatives
            .iter()
            .map(|signal| {
                signal
                    .semantic_digest()
                    .map_err(|_| ProbabilityRefusal::InvalidSample)
            })
            .collect::<Result<Vec<_>, _>>()?;
        probabilistic_digest(
            "probability/trajectory-alternatives@1",
            &signals,
            &self.provenance,
            &self.disposition,
            &self.observation_identity,
        )
    }
}

fn tensor_digests(
    tensors: &[conduit_data::TensorValue],
) -> Result<Vec<[u8; 32]>, ProbabilityRefusal> {
    tensors
        .iter()
        .map(|tensor| {
            tensor
                .semantic_digest()
                .map_err(|_| ProbabilityRefusal::InvalidSample)
        })
        .collect()
}

fn probabilistic_digest(
    domain: &str,
    values: &[[u8; 32]],
    provenance: &StochasticProvenance,
    disposition: &ProbabilisticDisposition,
    extra: &[u8],
) -> Result<[u8; 32], ProbabilityRefusal> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(values.len() as u16).to_le_bytes());
    for value in values {
        bytes.extend_from_slice(value);
    }
    bytes.extend_from_slice(&provenance.semantic_digest()?);
    encode_disposition(&mut bytes, disposition);
    bytes.extend_from_slice(extra);
    Ok(semantic_digest(domain, &bytes))
}

fn encode_disposition(output: &mut Vec<u8>, disposition: &ProbabilisticDisposition) {
    match disposition {
        ProbabilisticDisposition::Exact => output.push(0),
        ProbabilisticDisposition::Approximate { method_profile } => {
            output.push(1);
            push_text(output, method_profile);
        }
        ProbabilisticDisposition::Truncated {
            retained_samples,
            requested_samples,
        } => {
            output.push(2);
            output.extend_from_slice(&retained_samples.to_le_bytes());
            output.extend_from_slice(&requested_samples.to_le_bytes());
        }
    }
}

fn push_optional_digest(output: &mut Vec<u8>, value: Option<[u8; 32]>) {
    match value {
        None => output.push(0),
        Some(value) => {
            output.push(1);
            output.extend_from_slice(&value);
        }
    }
}

fn push_text(output: &mut Vec<u8>, value: &str) {
    output.extend_from_slice(&(value.len() as u16).to_le_bytes());
    output.extend_from_slice(value.as_bytes());
}
