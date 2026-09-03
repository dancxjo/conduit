//! Scientific observations retain measurement identity, clocks, and lineage.

use alloc::{boxed::Box, string::String, vec::Vec};
use conduit_core::BoundedResourceRef;

use crate::{SampledSignal, TensorElement, TensorValue};

pub const MAXIMUM_OBSERVATIONS_PER_SET: usize = 64;
pub const MAXIMUM_OBSERVATION_SOURCES: usize = 16;
pub const MAXIMUM_SCIENTIFIC_IDENTITY_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservationValue {
    Tensor(Box<TensorValue>),
    SampledSignal(Box<SampledSignal>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservationProvenance {
    Measured {
        source: BoundedResourceRef,
        measurement_profile: String,
    },
    Derived {
        source_observations: Vec<[u8; 32]>,
        transform_identity: String,
        realization_profile: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScientificObservation {
    pub identity: [u8; 32],
    pub semantic_kind: String,
    pub clock_identity: Option<String>,
    pub coordinate_frame: Option<String>,
    pub value: ObservationValue,
    pub provenance: ObservationProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingDataMask {
    pub observation_identity: [u8; 32],
    /// U8 values: 0 observed, 1 not observed, 2 invalid/clipped, 3 discarded.
    pub mask: TensorValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationSet {
    pub identity: [u8; 32],
    pub session_identity: String,
    pub subject_identity: Option<String>,
    pub observations: Vec<ScientificObservation>,
    pub missing_data: Vec<MissingDataMask>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ScientificObservationRefusal {
    MissingIdentity,
    InvalidIdentity,
    InvalidValue,
    ClockMismatch,
    MissingSource,
    TooManySources,
    EmptyObservationSet,
    TooManyObservations,
    DuplicateObservation,
    InvalidMask,
    MaskShapeMismatch,
    UnknownMaskedObservation,
    DuplicateMask,
}

impl ScientificObservation {
    pub fn validate(&self) -> Result<(), ScientificObservationRefusal> {
        nonzero(self.identity)?;
        text(&self.semantic_kind)?;
        if let Some(clock) = &self.clock_identity {
            text(clock)?;
        }
        if let Some(frame) = &self.coordinate_frame {
            text(frame)?;
        }
        match &self.value {
            ObservationValue::Tensor(value) => value
                .validate()
                .map_err(|_| ScientificObservationRefusal::InvalidValue)?,
            ObservationValue::SampledSignal(value) => {
                value
                    .validate()
                    .map_err(|_| ScientificObservationRefusal::InvalidValue)?;
                if self.clock_identity.as_deref() != Some(&value.clock_identity) {
                    return Err(ScientificObservationRefusal::ClockMismatch);
                }
            }
        }
        match &self.provenance {
            ObservationProvenance::Measured {
                source,
                measurement_profile,
            } => {
                source
                    .validate()
                    .map_err(|_| ScientificObservationRefusal::MissingSource)?;
                text(measurement_profile)?;
            }
            ObservationProvenance::Derived {
                source_observations,
                transform_identity,
                realization_profile,
            } => {
                if source_observations.is_empty() {
                    return Err(ScientificObservationRefusal::MissingSource);
                }
                if source_observations.len() > MAXIMUM_OBSERVATION_SOURCES {
                    return Err(ScientificObservationRefusal::TooManySources);
                }
                if source_observations.contains(&[0; 32]) {
                    return Err(ScientificObservationRefusal::MissingSource);
                }
                text(transform_identity)?;
                text(realization_profile)?;
            }
        }
        Ok(())
    }

    pub fn shape(&self) -> &[u64] {
        match &self.value {
            ObservationValue::Tensor(value) => &value.dimensions,
            ObservationValue::SampledSignal(value) => &value.samples.dimensions,
        }
    }
}

impl MissingDataMask {
    pub fn validate_for(
        &self,
        observation: &ScientificObservation,
    ) -> Result<(), ScientificObservationRefusal> {
        nonzero(self.observation_identity)?;
        self.mask
            .validate()
            .map_err(|_| ScientificObservationRefusal::InvalidMask)?;
        if self.observation_identity != observation.identity {
            return Err(ScientificObservationRefusal::UnknownMaskedObservation);
        }
        if self.mask.element != TensorElement::U8 {
            return Err(ScientificObservationRefusal::InvalidMask);
        }
        if self.mask.dimensions != observation.shape() {
            return Err(ScientificObservationRefusal::MaskShapeMismatch);
        }
        if let crate::TensorBacking::Inline(values) = &self.mask.backing {
            if values.iter().any(|value| *value > 3) {
                return Err(ScientificObservationRefusal::InvalidMask);
            }
        }
        Ok(())
    }
}

impl ObservationSet {
    pub fn validate(&self) -> Result<(), ScientificObservationRefusal> {
        nonzero(self.identity)?;
        text(&self.session_identity)?;
        if let Some(subject) = &self.subject_identity {
            text(subject)?;
        }
        if self.observations.is_empty() {
            return Err(ScientificObservationRefusal::EmptyObservationSet);
        }
        if self.observations.len() > MAXIMUM_OBSERVATIONS_PER_SET {
            return Err(ScientificObservationRefusal::TooManyObservations);
        }
        for observation in &self.observations {
            observation.validate()?;
        }
        if duplicate(self.observations.iter().map(|value| value.identity)) {
            return Err(ScientificObservationRefusal::DuplicateObservation);
        }
        if duplicate(
            self.missing_data
                .iter()
                .map(|value| value.observation_identity),
        ) {
            return Err(ScientificObservationRefusal::DuplicateMask);
        }
        for mask in &self.missing_data {
            let observation = self
                .observations
                .iter()
                .find(|value| value.identity == mask.observation_identity)
                .ok_or(ScientificObservationRefusal::UnknownMaskedObservation)?;
            mask.validate_for(observation)?;
        }
        Ok(())
    }

    pub fn observation(&self, identity: [u8; 32]) -> Option<&ScientificObservation> {
        self.observations
            .iter()
            .find(|value| value.identity == identity)
    }
}

pub(crate) fn text(value: &str) -> Result<(), ScientificObservationRefusal> {
    if value.is_empty() || value.len() > MAXIMUM_SCIENTIFIC_IDENTITY_BYTES {
        Err(ScientificObservationRefusal::InvalidIdentity)
    } else {
        Ok(())
    }
}

pub(crate) fn nonzero(value: [u8; 32]) -> Result<(), ScientificObservationRefusal> {
    if value == [0; 32] {
        Err(ScientificObservationRefusal::MissingIdentity)
    } else {
        Ok(())
    }
}

pub(crate) fn duplicate(values: impl Iterator<Item = [u8; 32]>) -> bool {
    let values = values.collect::<Vec<_>>();
    values
        .iter()
        .enumerate()
        .any(|(index, value)| values[index + 1..].contains(value))
}
