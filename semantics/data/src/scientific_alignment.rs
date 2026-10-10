//! Explicit clock relations, coordinate frames, and derived alignment views.

use conduit_core::QuantityDimension;
use conduit_plot::rust_binding::BoundedSequence;

use crate::{
    nonzero, text, AlignedTrainingView, CalibrationTransform, ClockRelation, ClockRelationQuality,
    CoordinateFrame, ObservationProvenance, ObservationSet, ObservationValue,
    ScientificAlignmentRefusal, ScientificObservation, ScientificObservationIdentity,
    ScientificObservationRefusal, TensorElement,
};

pub const MAXIMUM_COORDINATE_DIMENSIONS: usize = 4;
pub const MAXIMUM_ALIGNMENT_SOURCES: usize = 16;

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

impl From<ScientificObservationRefusal> for ScientificAlignmentRefusal {
    fn from(refusal: ScientificObservationRefusal) -> Self {
        Self::observation(refusal).expect("an authored observation refusal is valid")
    }
}

impl ClockRelation {
    pub fn validate(&self) -> Result<(), ScientificAlignmentRefusal> {
        text(self.identity()).map_err(ScientificAlignmentRefusal::from)?;
        text(self.source_clock()).map_err(ScientificAlignmentRefusal::from)?;
        text(self.target_clock()).map_err(ScientificAlignmentRefusal::from)?;
        if self.source_clock() == self.target_clock()
            || *self.source_ticks() == 0
            || *self.target_ticks() == 0
        {
            return Err(ScientificAlignmentRefusal::InvalidRelation);
        }
        if let ClockRelationQuality::Estimated(estimated) = self.quality() {
            let maximum_error = estimated.maximum_error();
            if maximum_error.coefficient() <= 0
                || maximum_error.dimension() != QuantityDimension::Time
            {
                return Err(ScientificAlignmentRefusal::InvalidRelation);
            }
        }
        Ok(())
    }
}

impl CoordinateFrame {
    pub fn validate(&self) -> Result<(), ScientificAlignmentRefusal> {
        text(&self.identity).map_err(ScientificAlignmentRefusal::from)?;
        if self.axes.is_empty() || self.axes.len() > MAXIMUM_COORDINATE_DIMENSIONS {
            return Err(ScientificAlignmentRefusal::InvalidCoordinateFrame);
        }
        for axis in &self.axes {
            text(axis.get()).map_err(ScientificAlignmentRefusal::from)?;
        }
        if self
            .axes
            .iter()
            .enumerate()
            .any(|(index, axis)| self.axes.as_slice()[index + 1..].contains(axis))
            || self.unit.dimension() != QuantityDimension::Length
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
        text(&self.identity).map_err(ScientificAlignmentRefusal::from)?;
        text(&self.method_profile).map_err(ScientificAlignmentRefusal::from)?;
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
            || self.linear.dimensions.as_slice()
                != [target.axes.len() as u64, source.axes.len() as u64]
            || self.translation.element != self.linear.element
            || self.translation.dimensions.as_slice() != [target.axes.len() as u64]
        {
            return Err(ScientificAlignmentRefusal::CalibrationShapeMismatch);
        }
        if self.calibration_sources.is_empty()
            || self.calibration_sources.len() > MAXIMUM_ALIGNMENT_SOURCES
            || self
                .calibration_sources
                .iter()
                .any(|identity| identity.get() == &[0; 32])
        {
            return Err(ScientificAlignmentRefusal::MissingCalibrationSource);
        }
        Ok(())
    }
}

impl AlignedTrainingView {
    pub fn validate(&self) -> Result<(), ScientificAlignmentRefusal> {
        nonzero(self.source_set_identity).map_err(ScientificAlignmentRefusal::from)?;
        nonzero(self.source_observation_identity).map_err(ScientificAlignmentRefusal::from)?;
        text(&self.clock_relation_identity).map_err(ScientificAlignmentRefusal::from)?;
        text(&self.target_clock).map_err(ScientificAlignmentRefusal::from)?;
        if let Some(identity) = &self.calibration_identity {
            text(identity).map_err(ScientificAlignmentRefusal::from)?;
        }
        self.derived_observation
            .validate()
            .map_err(ScientificAlignmentRefusal::from)?;
        if self.derived_observation.clock_identity.as_deref() != Some(&self.target_clock) {
            return Err(ScientificAlignmentRefusal::DerivedClockMismatch);
        }
        let expected_transform = self
            .calibration_identity
            .as_deref()
            .unwrap_or(&self.clock_relation_identity);
        match &self.derived_observation.provenance {
            ObservationProvenance::Derived(provenance)
                if provenance
                    .source_observations()
                    .iter()
                    .any(|identity| identity.get() == &self.source_observation_identity)
                    && provenance.transform_identity() == expected_transform => {}
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
        set.validate().map_err(ScientificAlignmentRefusal::from)?;
        relation.validate()?;
        nonzero(derived_identity).map_err(ScientificAlignmentRefusal::from)?;
        text(target_clock).map_err(ScientificAlignmentRefusal::from)?;
        text(resampling_profile).map_err(ScientificAlignmentRefusal::from)?;
        let source = set
            .observation(source_observation_identity)
            .ok_or(ScientificAlignmentRefusal::UnknownSourceObservation)?;
        if source.clock_identity.as_deref() != Some(relation.source_clock())
            || relation.target_clock() != target_clock
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
            provenance: ObservationProvenance::derived(
                resampling_profile.into(),
                BoundedSequence::try_from_iter([ScientificObservationIdentity::new(
                    source.identity,
                )
                .expect("a scientific observation identity is exactly 32 bytes")])
                .expect("one source observation fits"),
                calibration
                    .map(|(value, _, _)| value.identity.clone())
                    .unwrap_or_else(|| relation.identity().clone()),
            )
            .map_err(|_| ScientificAlignmentRefusal::DerivedProvenanceMismatch)?,
        };
        derived
            .validate()
            .map_err(ScientificAlignmentRefusal::from)?;
        if derived.clock_identity.as_deref() != Some(target_clock) {
            return Err(ScientificAlignmentRefusal::DerivedClockMismatch);
        }
        let view = Self {
            source_set_identity: set.identity,
            source_observation_identity,
            clock_relation_identity: relation.identity().clone(),
            calibration_identity: calibration.map(|(value, _, _)| value.identity.clone()),
            target_clock: target_clock.into(),
            derived_observation: derived,
        };
        view.validate()?;
        Ok(view)
    }
}
