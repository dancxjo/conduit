//! Retained initialized recovery software offered to the ordinary planner.
//! This owns no store, Line, native effect, authority or Body lifecycle.
use super::*;
use alloc::{format, vec};
use conduit_core as c;

#[derive(Debug, PartialEq, Eq)]
pub enum RecoveryInstallationRefusal {
    Source,
    References(RecordingReferenceRefusal),
    Initialization(RecordingHistoryRefusal),
    Advertisement,
    Selection,
    AlreadySelected,
}
pub struct ReadyRecordingSoftware<B> {
    host: c::HostId,
    boot: c::BootId,
    source: c::SourceDocumentId,
    checked: c::CheckedPlotId,
    expanded: c::ExpandedPlotId,
    gear: c::GearId,
    generation: Option<c::OfferGeneration>,
    offer: c::CapabilityOffer,
    back: Option<B>,
}
pub type ReadyRecordingRecovery = ReadyRecordingSoftware<RecordingRecoveryBack>;
pub type ReadyRecordingGenerations = ReadyRecordingSoftware<RecordingReferenceSource>;
impl ReadyRecordingRecovery {
    /// Root supplies checked meaning, actual current recorder residence, and
    /// independently authenticated historical producer context. These are
    /// different domains: a new recorder Boot does not change recorded context.
    /// Complete memory/work admission remains Root's responsibility.
    pub fn prepare(
        source: &conduit_plot::ExpandedCanonicalPlot,
        host: c::HostId,
        boot: c::BootId,
        producer: RecordingContext<'_>,
        contract: &ObservationContract,
        maximum_chunks: u16,
    ) -> Result<Self, RecoveryInstallationRefusal> {
        let back = RecordingRecoveryBack::prepare(producer, contract, maximum_chunks)
            .map_err(RecoveryInstallationRefusal::Initialization)?;
        Self::from_prepared(
            source,
            host,
            boot,
            recovery_kind(),
            back,
            "recovery",
            "recording-recovery",
        )
    }
}
impl ReadyRecordingGenerations {
    pub fn prepare_generations(
        source: &conduit_plot::ExpandedCanonicalPlot,
        host: c::HostId,
        boot: c::BootId,
        references: &[c::BoundedResourceRef],
        maximum_generations: u16,
    ) -> Result<Self, RecoveryInstallationRefusal> {
        let back = RecordingReferenceSource::prepare(references, maximum_generations)
            .map_err(RecoveryInstallationRefusal::References)?;
        Self::from_prepared(
            source,
            host,
            boot,
            generation_source_kind(),
            back,
            "generations",
            "recording-generations",
        )
    }
}
impl<B> ReadyRecordingSoftware<B> {
    fn from_prepared(
        source: &conduit_plot::ExpandedCanonicalPlot,
        host: c::HostId,
        boot: c::BootId,
        kind: c::Kind,
        back: B,
        role: &str,
        artifact: &str,
    ) -> Result<Self, RecoveryInstallationRefusal> {
        use RecoveryInstallationRefusal as E;
        let mut matches = source
            .gears
            .iter()
            .filter(|gear| gear.kind_id == kind.kind_id);
        let gear = matches.next().ok_or(E::Source)?;
        if matches.next().is_some()
            || host.as_str().is_empty()
            || boot.as_str().is_empty()
            || gear.kind_contract_revision != kind.kind_contract_revision
            || gear.startup_parameters != kind.startup_parameters
            || gear.shorthand != kind.shorthand
            || gear.inputs != kind.inputs
            || gear.outputs != kind.outputs
            || gear.semantic_contract != kind.semantic_contract()
            || !gear.configuration.is_empty()
        {
            return Err(E::Source);
        }
        let id = format!("environment/{role}/{}", gear.gear_id.as_str());
        let profile = format!("conduit.environment/prepared-{role}@1");
        let artifact = format!("conduit-environment-runtime/{artifact}@1");
        let offer = c::BackOfferBuilder::new(
            kind,
            c::Back {
                capability_id: id.as_str().into(),
                execution_profile_id: profile.as_str().into(),
                implementation_id: id.as_str().into(),
                artifact_id: artifact.as_str().into(),
                host_calls: vec![],
                resource_requirements: vec![],
                authority_requirements: vec![],
            },
        )
        .build();
        Ok(Self {
            host,
            boot,
            source: source.source_document_id.clone(),
            checked: source.checked_plot_id.clone(),
            expanded: source.expanded_plot_id.clone(),
            gear: gear.gear_id.clone(),
            generation: None,
            offer,
            back: Some(back),
        })
    }
    /// Refusals preserve both arguments. Selected software is omitted from a
    /// fresh advertisement; Root must replace historical offer snapshots.
    pub fn append_to_advertisement(
        &mut self,
        host: &mut c::HostAdvertisement,
    ) -> Result<(), RecoveryInstallationRefusal> {
        if host.host_id != self.host
            || host.boot_id != self.boot
            || (self.back.is_some()
                && host
                    .capabilities
                    .iter()
                    .any(|offer| offer.capability_id == self.offer.capability_id))
        {
            return Err(RecoveryInstallationRefusal::Advertisement);
        }
        if self.back.is_some() {
            host.capabilities.push(self.offer.clone());
        }
        self.generation = Some(host.offer_generation);
        Ok(())
    }
    /// Ownership transfer after ordinary exact selection. This neither starts
    /// Play nor authorizes a snapshot read; those require their own admission.
    pub fn take_selected(
        &mut self,
        fragment: &c::PlanFragment,
        placement: &c::PlacementId,
    ) -> Result<B, RecoveryInstallationRefusal> {
        use RecoveryInstallationRefusal as E;
        if !c::verify_plan_fragment(fragment)
            || fragment.host_id != self.host
            || fragment.boot_id != self.boot
            || fragment.source_document_id != self.source
            || fragment.checked_plot_id != self.checked
            || fragment.expanded_plot_id != self.expanded
        {
            return Err(E::Selection);
        }
        let gear = fragment
            .placements
            .iter()
            .find(|gear| &gear.placement_id == placement)
            .ok_or(E::Selection)?;
        let offer = &self.offer;
        if gear.gear_id != self.gear
            || self.generation != Some(gear.offer_generation)
            || gear.host_id != self.host
            || gear.boot_id != self.boot
            || gear.limits != offer.limits
            || !gear.configuration.is_empty()
            || gear.kind_id != offer.kind_id
            || gear.kind_contract_revision != offer.kind_contract_revision
            || gear.semantic_contract != offer.semantic_contract
            || gear.inputs != offer.inputs
            || gear.outputs != offer.outputs
            || gear.capability_id != offer.capability_id
            || gear.execution_profile_id != offer.implementation.execution_profile_id
            || gear.implementation_id != offer.implementation.implementation_id
            || gear.artifact_id != offer.implementation.artifact_id
            || gear.host_calls != offer.host_calls
            || gear.base.is_some()
            || !gear.resources.is_empty()
            || !gear.authority.is_empty()
            || !gear.pool_references.is_empty()
            || !gear.terminal_transductions.is_empty()
            || !gear.realization_properties.is_empty()
            || !gear.realization_characteristics.is_empty()
        {
            return Err(E::Selection);
        }
        self.back.take().ok_or(E::AlreadySelected)
    }
}
