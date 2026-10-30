//! Exact per-gear custody when a checked Source repeats one temporal Kind.
use super::*;
impl ReadyRecordingGenerations {
    pub fn prepare_generation_for(
        source: &conduit_plot::ExpandedCanonicalPlot,
        host: c::HostId,
        boot: c::BootId,
        gear: &c::GearId,
        reference: &c::BoundedResourceRef,
    ) -> Result<Self, RecoveryInstallationRefusal> {
        let back = RecordingReferenceSource::prepare(core::slice::from_ref(reference), 1)
            .map_err(RecoveryInstallationRefusal::References)?;
        Self::from_prepared_for(
            source,
            host,
            boot,
            generation_value_kind(),
            back,
            ("generation", "recording-generation"),
            Some(gear),
        )
    }
}
impl ReadyRecordingSingleton {
    pub fn prepare_singleton_for(
        source: &conduit_plot::ExpandedCanonicalPlot,
        host: c::HostId,
        boot: c::BootId,
        gear: &c::GearId,
    ) -> Result<Self, RecoveryInstallationRefusal> {
        Self::from_prepared_for(
            source,
            host,
            boot,
            recording_singleton_kind(),
            RecordingSingletonBack::default(),
            ("json-singleton", "recording-json-singleton"),
            Some(gear),
        )
    }
}
#[cfg(feature = "recording-kernel")]
pub type ReadyRecordingConcat = ReadyRecordingSoftware<conduit_composite::FlowConcatFiniteBack>;
#[cfg(feature = "recording-kernel")]
impl ReadyRecordingConcat {
    pub fn prepare_concat_for(
        source: &conduit_plot::ExpandedCanonicalPlot,
        host: c::HostId,
        boot: c::BootId,
        gear: &c::GearId,
    ) -> Result<Self, RecoveryInstallationRefusal> {
        let schema = c::StructuredInfoType::leaf(c::kind_id("text/json-utf8@1"))
            .map_err(|_| RecoveryInstallationRefusal::TemporalInitialization)?;
        let value = c::CheckedValueContract::new(
            c::kind_id("text/json-utf8@1"),
            MAXIMUM_CHUNK_BYTES as u32,
            vec![],
        )
        .map_err(|_| RecoveryInstallationRefusal::TemporalInitialization)?;
        let kind = conduit_semantic_catalog::flow_concat_finite_semantic_contract(&value, &schema)
            .map_err(|_| RecoveryInstallationRefusal::TemporalInitialization)?;
        let back =
            conduit_composite::FlowConcatFiniteBack::prepare(&schema, MAXIMUM_CHUNK_BYTES as u32)
                .map_err(|_| RecoveryInstallationRefusal::TemporalInitialization)?;
        Self::from_prepared_for(
            source,
            host,
            boot,
            kind,
            back,
            ("json-concat", "recording-json-concat"),
            Some(gear),
        )
    }
}
