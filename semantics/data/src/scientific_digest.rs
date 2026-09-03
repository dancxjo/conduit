//! Canonical identities for scientific evidence descriptors.

use alloc::vec::Vec;
use conduit_core::semantic_digest;

use crate::{
    AlignedTrainingView, CalibrationTransform, ClockRelation, ClockRelationQuality,
    CoordinateFrame, DatasetDescriptor, DatasetSplitMembership, MissingDataMask,
    ObservationProvenance, ObservationSet, ObservationValue, ScientificAlignmentRefusal,
    ScientificCorpusRefusal, ScientificObservation, ScientificObservationRefusal,
};

impl ScientificObservation {
    pub fn semantic_digest(&self) -> Result<[u8; 32], ScientificObservationRefusal> {
        self.validate()?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.identity);
        push_text(&mut bytes, &self.semantic_kind);
        push_optional_text(&mut bytes, self.clock_identity.as_deref());
        push_optional_text(&mut bytes, self.coordinate_frame.as_deref());
        match &self.value {
            ObservationValue::Tensor(value) => {
                bytes.push(0);
                bytes.extend_from_slice(
                    &value
                        .semantic_digest()
                        .map_err(|_| ScientificObservationRefusal::InvalidValue)?,
                );
            }
            ObservationValue::SampledSignal(value) => {
                bytes.push(1);
                bytes.extend_from_slice(
                    &value
                        .semantic_digest()
                        .map_err(|_| ScientificObservationRefusal::InvalidValue)?,
                );
            }
        }
        match &self.provenance {
            ObservationProvenance::Measured {
                source,
                measurement_profile,
            } => {
                bytes.push(0);
                bytes.extend_from_slice(
                    &source
                        .semantic_digest()
                        .map_err(|_| ScientificObservationRefusal::MissingSource)?,
                );
                push_text(&mut bytes, measurement_profile);
            }
            ObservationProvenance::Derived {
                source_observations,
                transform_identity,
                realization_profile,
            } => {
                bytes.push(1);
                push_digests(&mut bytes, source_observations);
                push_text(&mut bytes, transform_identity);
                push_text(&mut bytes, realization_profile);
            }
        }
        Ok(semantic_digest("science/observation@1", &bytes))
    }
}

impl MissingDataMask {
    fn semantic_digest(&self) -> Result<[u8; 32], ScientificObservationRefusal> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.observation_identity);
        bytes.extend_from_slice(
            &self
                .mask
                .semantic_digest()
                .map_err(|_| ScientificObservationRefusal::InvalidMask)?,
        );
        Ok(semantic_digest("science/missing-data-mask@1", &bytes))
    }
}

impl ObservationSet {
    pub fn semantic_digest(&self) -> Result<[u8; 32], ScientificObservationRefusal> {
        self.validate()?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.identity);
        push_text(&mut bytes, &self.session_identity);
        push_optional_text(&mut bytes, self.subject_identity.as_deref());
        push_len(&mut bytes, self.observations.len());
        for observation in &self.observations {
            bytes.extend_from_slice(&observation.semantic_digest()?);
        }
        push_len(&mut bytes, self.missing_data.len());
        for mask in &self.missing_data {
            bytes.extend_from_slice(&mask.semantic_digest()?);
        }
        Ok(semantic_digest("science/observation-set@1", &bytes))
    }
}

impl ClockRelation {
    pub fn semantic_digest(&self) -> Result<[u8; 32], ScientificAlignmentRefusal> {
        self.validate()?;
        let mut bytes = Vec::new();
        push_text(&mut bytes, &self.identity);
        push_text(&mut bytes, &self.source_clock);
        push_text(&mut bytes, &self.target_clock);
        bytes.extend_from_slice(&self.source_anchor.to_le_bytes());
        bytes.extend_from_slice(&self.target_anchor.to_le_bytes());
        bytes.extend_from_slice(&self.source_ticks.to_le_bytes());
        bytes.extend_from_slice(&self.target_ticks.to_le_bytes());
        match self.quality {
            ClockRelationQuality::Exact => bytes.push(0),
            ClockRelationQuality::Estimated { maximum_error } => {
                bytes.push(1);
                bytes.extend_from_slice(&maximum_error.value().to_le_bytes());
                bytes.push(time_unit_tag(maximum_error.unit()));
            }
        }
        Ok(semantic_digest("science/clock-relation@1", &bytes))
    }
}

impl CoordinateFrame {
    pub fn semantic_digest(&self) -> Result<[u8; 32], ScientificAlignmentRefusal> {
        self.validate()?;
        let mut bytes = Vec::new();
        push_text(&mut bytes, &self.identity);
        push_len(&mut bytes, self.axes.len());
        for axis in &self.axes {
            push_text(&mut bytes, axis);
        }
        push_text(&mut bytes, quantity_unit_name(self.unit));
        Ok(semantic_digest("science/coordinate-frame@1", &bytes))
    }
}

impl CalibrationTransform {
    pub fn semantic_digest(
        &self,
        source: &CoordinateFrame,
        target: &CoordinateFrame,
    ) -> Result<[u8; 32], ScientificAlignmentRefusal> {
        self.validate(source, target)?;
        let mut bytes = Vec::new();
        push_text(&mut bytes, &self.identity);
        bytes.extend_from_slice(&source.semantic_digest()?);
        bytes.extend_from_slice(&target.semantic_digest()?);
        bytes.extend_from_slice(
            &self
                .linear
                .semantic_digest()
                .map_err(|_| ScientificAlignmentRefusal::InvalidCalibration)?,
        );
        bytes.extend_from_slice(
            &self
                .translation
                .semantic_digest()
                .map_err(|_| ScientificAlignmentRefusal::InvalidCalibration)?,
        );
        push_digests(&mut bytes, &self.calibration_sources);
        push_text(&mut bytes, &self.method_profile);
        Ok(semantic_digest("science/calibration-transform@1", &bytes))
    }
}

impl AlignedTrainingView {
    pub fn semantic_digest(&self) -> Result<[u8; 32], ScientificAlignmentRefusal> {
        self.validate()?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.source_set_identity);
        bytes.extend_from_slice(&self.source_observation_identity);
        push_text(&mut bytes, &self.clock_relation_identity);
        push_optional_text(&mut bytes, self.calibration_identity.as_deref());
        push_text(&mut bytes, &self.target_clock);
        bytes.extend_from_slice(
            &self
                .derived_observation
                .semantic_digest()
                .map_err(ScientificAlignmentRefusal::Observation)?,
        );
        Ok(semantic_digest("science/aligned-training-view@1", &bytes))
    }
}

impl DatasetDescriptor {
    pub fn semantic_digest(&self) -> Result<[u8; 32], ScientificCorpusRefusal> {
        self.validate()?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.identity);
        push_text(&mut bytes, &self.schema_profile);
        push_optional_text(&mut bytes, self.citation_identity.as_deref());
        push_optional_text(&mut bytes, self.license_profile.as_deref());
        bytes.extend_from_slice(&self.example_count.to_le_bytes());
        bytes.extend_from_slice(
            &self
                .manifest
                .semantic_digest()
                .map_err(|_| ScientificCorpusRefusal::InvalidManifest)?,
        );
        push_len(&mut bytes, self.shards.len());
        for shard in &self.shards {
            bytes.extend_from_slice(
                &shard
                    .semantic_digest()
                    .map_err(|_| ScientificCorpusRefusal::InvalidManifest)?,
            );
        }
        push_len(&mut bytes, self.split_identities.len());
        for split in &self.split_identities {
            push_text(&mut bytes, split);
        }
        Ok(semantic_digest("science/dataset@1", &bytes))
    }
}

impl DatasetSplitMembership {
    pub fn semantic_digest(&self) -> Result<[u8; 32], ScientificCorpusRefusal> {
        self.validate()?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.dataset_identity);
        push_text(&mut bytes, &self.split_identity);
        push_digests(&mut bytes, &self.examples);
        Ok(semantic_digest(
            "science/dataset-split-membership@1",
            &bytes,
        ))
    }
}

fn push_digests(output: &mut Vec<u8>, values: &[[u8; 32]]) {
    push_len(output, values.len());
    for value in values {
        output.extend_from_slice(value);
    }
}

fn push_len(output: &mut Vec<u8>, value: usize) {
    output.extend_from_slice(&(value as u16).to_le_bytes());
}

fn push_optional_text(output: &mut Vec<u8>, value: Option<&str>) {
    match value {
        None => output.push(0),
        Some(value) => {
            output.push(1);
            push_text(output, value);
        }
    }
}

fn push_text(output: &mut Vec<u8>, value: &str) {
    output.extend_from_slice(&(value.len() as u16).to_le_bytes());
    output.extend_from_slice(value.as_bytes());
}

fn time_unit_tag(unit: conduit_core::QuantityUnit) -> u8 {
    match unit {
        conduit_core::QuantityUnit::Nanosecond => 0,
        conduit_core::QuantityUnit::Microsecond => 1,
        conduit_core::QuantityUnit::Millisecond => 2,
        conduit_core::QuantityUnit::Second => 3,
        _ => unreachable!("validated clock error is a time quantity"),
    }
}

fn quantity_unit_name(unit: conduit_core::QuantityUnit) -> &'static str {
    match unit {
        conduit_core::QuantityUnit::Micrometer => "micrometer",
        conduit_core::QuantityUnit::Millimeter => "millimeter",
        conduit_core::QuantityUnit::Centimeter => "centimeter",
        conduit_core::QuantityUnit::Meter => "meter",
        _ => unreachable!("validated coordinate frame is spatial"),
    }
}
