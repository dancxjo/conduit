//! Finite claims about uncertainty, samples, and stochastic provenance.

use alloc::{string::String, vec::Vec};
use conduit_data::{SampledSignal, TensorElement, TensorValue};

pub const MAXIMUM_PROBABILITY_SAMPLES: usize = 64;
pub const MAXIMUM_TRAJECTORY_ALTERNATIVES: usize = 32;
pub const MAXIMUM_COVARIANCE_DIMENSION: u64 = 256;
pub const NORMALIZED_WEIGHT_UNITS: u64 = 1_000_000_000;
pub const MAXIMUM_STOCHASTIC_IDENTITY_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RandomnessProfile {
    Deterministic,
    ExplicitSeed(u64),
    ProviderChosen { seed: u64, nonce: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrawRelationship {
    Independent,
    Correlated { profile: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StochasticProvenance {
    pub model_artifact_identity: [u8; 32],
    pub checkpoint_identity: Option<[u8; 32]>,
    pub query_identity: [u8; 32],
    pub randomness: RandomnessProfile,
    pub draws: DrawRelationship,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbabilisticDisposition {
    Exact,
    Approximate {
        method_profile: String,
    },
    Truncated {
        retained_samples: u32,
        requested_samples: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbabilitySample {
    pub value: TensorValue,
    pub provenance: StochasticProvenance,
    pub disposition: ProbabilisticDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbabilitySampleSet {
    pub alternatives: Vec<TensorValue>,
    pub provenance: StochasticProvenance,
    pub disposition: ProbabilisticDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeightedSamples {
    pub alternatives: Vec<TensorValue>,
    /// Non-negative fixed-point weights whose exact sum is one billion.
    pub weights: Vec<u64>,
    pub provenance: StochasticProvenance,
    pub disposition: ProbabilisticDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeanVariance {
    pub mean: TensorValue,
    pub variance: TensorValue,
    pub provenance: StochasticProvenance,
    pub disposition: ProbabilisticDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeanCovariance {
    pub mean: TensorValue,
    pub covariance: TensorValue,
    pub provenance: StochasticProvenance,
    pub disposition: ProbabilisticDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogProbability {
    /// Natural logarithm of probability/density, in exact millionths.
    pub natural_log_millionths: i64,
    pub score_kind: LogScoreKind,
    pub support_identity: [u8; 32],
    pub provenance: StochasticProvenance,
    pub disposition: ProbabilisticDisposition,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LogScoreKind {
    ProbabilityMass,
    Density,
    UnnormalizedScore,
}

/// Alternative inferred trajectories for one observation. These are model
/// beliefs, never evidence that any alternative physically occurred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrajectoryAlternatives {
    pub observation_identity: [u8; 32],
    pub plausible_alternatives: Vec<SampledSignal>,
    pub provenance: StochasticProvenance,
    pub disposition: ProbabilisticDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbabilitySummary {
    pub claim_profile: &'static str,
    pub result_count: u32,
    pub model_artifact_identity: [u8; 32],
    pub query_identity: [u8; 32],
    pub randomness: RandomnessProfile,
    pub disposition: ProbabilisticDisposition,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ProbabilityRefusal {
    MissingIdentity,
    InvalidStochasticProfile,
    InvalidDisposition,
    EmptySamples,
    SampleCountOverflow,
    InvalidSample,
    ShapeMismatch,
    WeightCountMismatch,
    InvalidWeightSum,
    InvalidVariance,
    InvalidCovariance,
    CovarianceDimensionOverflow,
    InvalidLogProbability,
}

impl StochasticProvenance {
    pub fn validate(&self) -> Result<(), ProbabilityRefusal> {
        nonzero(self.model_artifact_identity)?;
        nonzero(self.query_identity)?;
        if self.checkpoint_identity == Some([0; 32]) {
            return Err(ProbabilityRefusal::MissingIdentity);
        }
        if let RandomnessProfile::ProviderChosen { nonce, .. } = &self.randomness {
            identity(nonce)?;
        }
        if let DrawRelationship::Correlated { profile } = &self.draws {
            identity(profile)?;
        }
        Ok(())
    }
}

impl ProbabilisticDisposition {
    fn validate(&self, actual_samples: Option<usize>) -> Result<(), ProbabilityRefusal> {
        match self {
            Self::Exact => Ok(()),
            Self::Approximate { method_profile } => identity(method_profile),
            Self::Truncated {
                retained_samples,
                requested_samples,
            } if *retained_samples > 0
                && retained_samples < requested_samples
                && actual_samples == Some(*retained_samples as usize) =>
            {
                Ok(())
            }
            Self::Truncated { .. } => Err(ProbabilityRefusal::InvalidDisposition),
        }
    }
}

impl ProbabilitySample {
    pub fn validate(&self) -> Result<(), ProbabilityRefusal> {
        self.value
            .validate()
            .map_err(|_| ProbabilityRefusal::InvalidSample)?;
        self.provenance.validate()?;
        self.disposition.validate(Some(1))
    }
}

impl ProbabilitySampleSet {
    pub fn validate(&self) -> Result<(), ProbabilityRefusal> {
        validate_samples(&self.alternatives)?;
        self.provenance.validate()?;
        self.disposition.validate(Some(self.alternatives.len()))
    }

    pub fn summary(&self) -> Result<ProbabilitySummary, ProbabilityRefusal> {
        self.validate()?;
        Ok(summary(
            "samples",
            self.alternatives.len(),
            &self.provenance,
            &self.disposition,
        ))
    }
}

impl WeightedSamples {
    pub fn validate(&self) -> Result<(), ProbabilityRefusal> {
        validate_samples(&self.alternatives)?;
        if self.weights.len() != self.alternatives.len() {
            return Err(ProbabilityRefusal::WeightCountMismatch);
        }
        if self.weights.contains(&0)
            || self
                .weights
                .iter()
                .try_fold(0_u64, |sum, value| sum.checked_add(*value))
                != Some(NORMALIZED_WEIGHT_UNITS)
        {
            return Err(ProbabilityRefusal::InvalidWeightSum);
        }
        self.provenance.validate()?;
        self.disposition.validate(Some(self.alternatives.len()))
    }

    pub fn summary(&self) -> Result<ProbabilitySummary, ProbabilityRefusal> {
        self.validate()?;
        Ok(summary(
            "weighted-samples",
            self.alternatives.len(),
            &self.provenance,
            &self.disposition,
        ))
    }
}

impl MeanVariance {
    pub fn validate(&self) -> Result<(), ProbabilityRefusal> {
        validate_pair(&self.mean, &self.variance)?;
        if !matches!(
            self.variance.element,
            TensorElement::F32 | TensorElement::F64
        ) {
            return Err(ProbabilityRefusal::InvalidVariance);
        }
        self.provenance.validate()?;
        self.disposition.validate(None)
    }
}

impl MeanCovariance {
    pub fn validate(&self) -> Result<(), ProbabilityRefusal> {
        self.mean
            .validate()
            .map_err(|_| ProbabilityRefusal::InvalidSample)?;
        self.covariance
            .validate()
            .map_err(|_| ProbabilityRefusal::InvalidCovariance)?;
        let dimension = self
            .mean
            .element_count()
            .map_err(|_| ProbabilityRefusal::InvalidSample)?;
        if dimension > MAXIMUM_COVARIANCE_DIMENSION {
            return Err(ProbabilityRefusal::CovarianceDimensionOverflow);
        }
        if self.covariance.dimensions != [dimension, dimension]
            || !matches!(
                self.covariance.element,
                TensorElement::F32 | TensorElement::F64
            )
        {
            return Err(ProbabilityRefusal::InvalidCovariance);
        }
        self.provenance.validate()?;
        self.disposition.validate(None)
    }
}

impl LogProbability {
    pub fn validate(&self) -> Result<(), ProbabilityRefusal> {
        nonzero(self.support_identity).map_err(|_| ProbabilityRefusal::InvalidLogProbability)?;
        if self.score_kind == LogScoreKind::ProbabilityMass && self.natural_log_millionths > 0 {
            return Err(ProbabilityRefusal::InvalidLogProbability);
        }
        self.provenance.validate()?;
        self.disposition.validate(None)
    }
}

impl TrajectoryAlternatives {
    pub fn validate(&self) -> Result<(), ProbabilityRefusal> {
        nonzero(self.observation_identity)?;
        if self.plausible_alternatives.is_empty() {
            return Err(ProbabilityRefusal::EmptySamples);
        }
        if self.plausible_alternatives.len() > MAXIMUM_TRAJECTORY_ALTERNATIVES {
            return Err(ProbabilityRefusal::SampleCountOverflow);
        }
        for signal in &self.plausible_alternatives {
            signal
                .validate()
                .map_err(|_| ProbabilityRefusal::InvalidSample)?;
        }
        let first = &self.plausible_alternatives[0];
        if self.plausible_alternatives[1..].iter().any(|signal| {
            signal.clock_identity != first.clock_identity
                || signal.start != first.start
                || signal.cadence != first.cadence
                || signal.sample_count != first.sample_count
                || signal.continuity != first.continuity
                || signal.samples.element != first.samples.element
                || signal.samples.dimensions != first.samples.dimensions
                || signal.samples.axes != first.samples.axes
        }) {
            return Err(ProbabilityRefusal::ShapeMismatch);
        }
        self.provenance.validate()?;
        self.disposition
            .validate(Some(self.plausible_alternatives.len()))
    }

    pub fn summary(&self) -> Result<ProbabilitySummary, ProbabilityRefusal> {
        self.validate()?;
        Ok(summary(
            "trajectory-alternatives",
            self.plausible_alternatives.len(),
            &self.provenance,
            &self.disposition,
        ))
    }
}

fn summary(
    claim_profile: &'static str,
    count: usize,
    provenance: &StochasticProvenance,
    disposition: &ProbabilisticDisposition,
) -> ProbabilitySummary {
    ProbabilitySummary {
        claim_profile,
        result_count: count as u32,
        model_artifact_identity: provenance.model_artifact_identity,
        query_identity: provenance.query_identity,
        randomness: provenance.randomness.clone(),
        disposition: disposition.clone(),
    }
}

fn validate_samples(samples: &[TensorValue]) -> Result<(), ProbabilityRefusal> {
    if samples.is_empty() {
        return Err(ProbabilityRefusal::EmptySamples);
    }
    if samples.len() > MAXIMUM_PROBABILITY_SAMPLES {
        return Err(ProbabilityRefusal::SampleCountOverflow);
    }
    for sample in samples {
        sample
            .validate()
            .map_err(|_| ProbabilityRefusal::InvalidSample)?;
    }
    if samples[1..].iter().any(|sample| {
        sample.element != samples[0].element
            || sample.dimensions != samples[0].dimensions
            || sample.axes != samples[0].axes
    }) {
        return Err(ProbabilityRefusal::ShapeMismatch);
    }
    Ok(())
}

fn validate_pair(left: &TensorValue, right: &TensorValue) -> Result<(), ProbabilityRefusal> {
    left.validate()
        .map_err(|_| ProbabilityRefusal::InvalidSample)?;
    right
        .validate()
        .map_err(|_| ProbabilityRefusal::InvalidVariance)?;
    if left.dimensions != right.dimensions || left.axes != right.axes {
        return Err(ProbabilityRefusal::ShapeMismatch);
    }
    Ok(())
}

fn nonzero(value: [u8; 32]) -> Result<(), ProbabilityRefusal> {
    if value == [0; 32] {
        Err(ProbabilityRefusal::MissingIdentity)
    } else {
        Ok(())
    }
}

fn identity(value: &str) -> Result<(), ProbabilityRefusal> {
    if value.is_empty() || value.len() > MAXIMUM_STOCHASTIC_IDENTITY_BYTES {
        Err(ProbabilityRefusal::InvalidStochasticProfile)
    } else {
        Ok(())
    }
}
