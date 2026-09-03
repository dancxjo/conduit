//! Explicit clock relations, coordinate frames, and derived alignment views.

use alloc::{string::String, vec::Vec};
use conduit_core::{Quantity, QuantityUnit};

use crate::{
    nonzero, text, ObservationProvenance, ObservationSet, ObservationValue, ScientificObservation,
    ScientificObservationRefusal, TensorElement, TensorValue,
};

pub const MAXIMUM_COORDINATE_DIMENSIONS: usize = 4;
pub const MAXIMUM_ALIGNMENT_SOURCES: usize = 16;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ClockRelationQuality {
    Exact,
    Estimated { maximum_error: Quantity },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClockRelation {
    pub identity: String,
    pub source_clock: String,
    pub target_clock: String,
    pub source_anchor: u64,
    pub target_anchor: u64,
    /// Positive rational scale: `source_ticks` map to `target_ticks`.
    pub source_ticks: u64,
    pub target_ticks: u64,
    pub quality: ClockRelationQuality,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoordinateFrame {
    pub identity: String,
    pub axes: Vec<String>,
    pub unit: QuantityUnit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalibrationTransform {
    pub identity: String,
    pub source_frame: String,
    pub target_frame: String,
    pub linear: TensorValue,
    pub translation: TensorValue,
    pub calibration_sources: Vec<[u8; 32]>,
    pub method_profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlignedTrainingView {
    pub source_set_identity: [u8; 32],
    pub source_observation_identity: [u8; 32],
    pub clock_relation_identity: String,
    pub calibration_identity: Option<String>,
    pub target_clock: String,
    pub derived_observation: ScientificObservation,
}

pub struct AlignmentDerivation<'a> {
    pub set: &'a ObservationSet,
    pub source_observation_identity: [u8; 32],
    pub relation: &'a ClockRelation,
    pub calibration: Option<(
        &'a CalibrationTransform,
        &'a CoordinateFrame,
        &'a CoordinateFrame,
    )>,
    pub target_clock: &'a str,
    pub derived_identity: [u8; 32],
    pub derived_value: ObservationValue,
    pub resampling_profile: &'a str,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ScientificAlignmentRefusal {
    Observation(ScientificObservationRefusal),
    InvalidRelation,
    IncompatibleClockRelation,
    UnknownSourceObservation,
    InvalidCoordinateFrame,
    InvalidCalibration,
    CalibrationFrameMismatch,
    CalibrationShapeMismatch,
    MissingCalibrationSource,
    DerivedClockMismatch,
    DerivedProvenanceMismatch,
}

impl ClockRelation {
    pub fn validate(&self) -> Result<(), ScientificAlignmentRefusal> {
        text(&self.identity).map_err(ScientificAlignmentRefusal::Observation)?;
        text(&self.source_clock).map_err(ScientificAlignmentRefusal::Observation)?;
        text(&self.target_clock).map_err(ScientificAlignmentRefusal::Observation)?;
        if self.source_clock == self.target_clock
            || self.source_ticks == 0
            || self.target_ticks == 0
        {
            return Err(ScientificAlignmentRefusal::InvalidRelation);
        }
        if let ClockRelationQuality::Estimated { maximum_error } = self.quality {
            if maximum_error.value() <= 0
                || !matches!(
                    maximum_error.unit(),
                    QuantityUnit::Second
                        | QuantityUnit::Millisecond
                        | QuantityUnit::Microsecond
                        | QuantityUnit::Nanosecond
                )
            {
                return Err(ScientificAlignmentRefusal::InvalidRelation);
            }
        }
        Ok(())
    }
}

impl CoordinateFrame {
    pub fn validate(&self) -> Result<(), ScientificAlignmentRefusal> {
        text(&self.identity).map_err(ScientificAlignmentRefusal::Observation)?;
        if self.axes.is_empty() || self.axes.len() > MAXIMUM_COORDINATE_DIMENSIONS {
            return Err(ScientificAlignmentRefusal::InvalidCoordinateFrame);
        }
        for axis in &self.axes {
            text(axis).map_err(ScientificAlignmentRefusal::Observation)?;
        }
        if self
            .axes
            .iter()
            .enumerate()
            .any(|(index, axis)| self.axes[index + 1..].contains(axis))
            || !matches!(
                self.unit,
                QuantityUnit::Micrometer
                    | QuantityUnit::Millimeter
                    | QuantityUnit::Centimeter
                    | QuantityUnit::Meter
            )
        {
            return Err(ScientificAlignmentRefusal::InvalidCoordinateFrame);
        }
        Ok(())
    }
}

impl CalibrationTransform {
    pub fn validate(
        &self,
        source: &CoordinateFrame,
        target: &CoordinateFrame,
    ) -> Result<(), ScientificAlignmentRefusal> {
        source.validate()?;
        target.validate()?;
        text(&self.identity).map_err(ScientificAlignmentRefusal::Observation)?;
        text(&self.method_profile).map_err(ScientificAlignmentRefusal::Observation)?;
        if self.source_frame != source.identity || self.target_frame != target.identity {
            return Err(ScientificAlignmentRefusal::CalibrationFrameMismatch);
        }
        self.linear
            .validate()
            .map_err(|_| ScientificAlignmentRefusal::InvalidCalibration)?;
        self.translation
            .validate()
            .map_err(|_| ScientificAlignmentRefusal::InvalidCalibration)?;
        if !matches!(self.linear.element, TensorElement::F32 | TensorElement::F64)
            || self.linear.dimensions != [target.axes.len() as u64, source.axes.len() as u64]
            || self.translation.element != self.linear.element
            || self.translation.dimensions != [target.axes.len() as u64]
        {
            return Err(ScientificAlignmentRefusal::CalibrationShapeMismatch);
        }
        if self.calibration_sources.is_empty()
            || self.calibration_sources.len() > MAXIMUM_ALIGNMENT_SOURCES
            || self.calibration_sources.contains(&[0; 32])
        {
            return Err(ScientificAlignmentRefusal::MissingCalibrationSource);
        }
        Ok(())
    }
}

impl AlignedTrainingView {
    pub fn validate(&self) -> Result<(), ScientificAlignmentRefusal> {
        nonzero(self.source_set_identity).map_err(ScientificAlignmentRefusal::Observation)?;
        nonzero(self.source_observation_identity)
            .map_err(ScientificAlignmentRefusal::Observation)?;
        text(&self.clock_relation_identity).map_err(ScientificAlignmentRefusal::Observation)?;
        text(&self.target_clock).map_err(ScientificAlignmentRefusal::Observation)?;
        if let Some(identity) = &self.calibration_identity {
            text(identity).map_err(ScientificAlignmentRefusal::Observation)?;
        }
        self.derived_observation
            .validate()
            .map_err(ScientificAlignmentRefusal::Observation)?;
        if self.derived_observation.clock_identity.as_deref() != Some(&self.target_clock) {
            return Err(ScientificAlignmentRefusal::DerivedClockMismatch);
        }
        let expected_transform = self
            .calibration_identity
            .as_deref()
            .unwrap_or(&self.clock_relation_identity);
        match &self.derived_observation.provenance {
            ObservationProvenance::Derived {
                source_observations,
                transform_identity,
                ..
            } if source_observations.contains(&self.source_observation_identity)
                && transform_identity == expected_transform => {}
            _ => return Err(ScientificAlignmentRefusal::DerivedProvenanceMismatch),
        }
        Ok(())
    }

    pub fn derive(request: AlignmentDerivation<'_>) -> Result<Self, ScientificAlignmentRefusal> {
        let AlignmentDerivation {
            set,
            source_observation_identity,
            relation,
            calibration,
            target_clock,
            derived_identity,
            derived_value,
            resampling_profile,
        } = request;
        set.validate()
            .map_err(ScientificAlignmentRefusal::Observation)?;
        relation.validate()?;
        nonzero(derived_identity).map_err(ScientificAlignmentRefusal::Observation)?;
        text(target_clock).map_err(ScientificAlignmentRefusal::Observation)?;
        text(resampling_profile).map_err(ScientificAlignmentRefusal::Observation)?;
        let source = set
            .observation(source_observation_identity)
            .ok_or(ScientificAlignmentRefusal::UnknownSourceObservation)?;
        if source.clock_identity.as_deref() != Some(&relation.source_clock)
            || relation.target_clock != target_clock
        {
            return Err(ScientificAlignmentRefusal::IncompatibleClockRelation);
        }
        if let Some((transform, source_frame, target_frame)) = calibration {
            transform.validate(source_frame, target_frame)?;
            if source.coordinate_frame.as_deref() != Some(&source_frame.identity) {
                return Err(ScientificAlignmentRefusal::CalibrationFrameMismatch);
            }
        }
        let derived = ScientificObservation {
            identity: derived_identity,
            semantic_kind: source.semantic_kind.clone(),
            clock_identity: Some(target_clock.into()),
            coordinate_frame: calibration
                .map(|(value, _, _)| value.target_frame.clone())
                .or_else(|| source.coordinate_frame.clone()),
            value: derived_value,
            provenance: ObservationProvenance::Derived {
                source_observations: alloc::vec![source.identity],
                transform_identity: calibration
                    .map(|(value, _, _)| value.identity.clone())
                    .unwrap_or_else(|| relation.identity.clone()),
                realization_profile: resampling_profile.into(),
            },
        };
        derived
            .validate()
            .map_err(ScientificAlignmentRefusal::Observation)?;
        if derived.clock_identity.as_deref() != Some(target_clock) {
            return Err(ScientificAlignmentRefusal::DerivedClockMismatch);
        }
        let view = Self {
            source_set_identity: set.identity,
            source_observation_identity,
            clock_relation_identity: relation.identity.clone(),
            calibration_identity: calibration.map(|(value, _, _)| value.identity.clone()),
            target_clock: target_clock.into(),
            derived_observation: derived,
        };
        view.validate()?;
        Ok(view)
    }
}
