//! Immutable plain-local placement projection. No callbacks, mutable readers,
//! incoming serialization, verification witness, or execution authority.
use super::*;

/// Only placement-specific facts. Offer and Host fields remain in their original
/// immutable descriptions instead of being repeated in every temporary record.
/// Configuration, terminal transductions, pool references and realization
/// characteristics are empty in this explicitly plain-local representation.
#[derive(Clone, Copy)]
pub struct PlainLocalPlacementRef<'a> {
    pub placement_id: &'a str,
    pub gear_id: &'a str,
    pub source_span: Option<SourceSpan>,
    pub capability_index: usize,
    pub base_index: Option<usize>,
    pub resources: ResourceBindingRefs<'a>,
    pub authority: AuthorityBindingRefs<'a>,
}

/// Complete records are reconstructed by value from stable immutable inputs.
/// Construction validates indexing only; ordinary full-Plan verification and
/// actual Root/provider admission remain independent requirements.
pub struct PlainLocalPlannedGears<'a> {
    host: HostAdvertisementRef<'a>,
    entries: &'a [PlainLocalPlacementRef<'a>],
}
impl<'a> PlainLocalPlannedGears<'a> {
    pub fn new(
        host: HostAdvertisementRef<'a>,
        entries: &'a [PlainLocalPlacementRef<'a>],
    ) -> Result<Self, FlatPlanRefusal> {
        if entries.iter().any(|entry| {
            entry.capability_index >= host.capabilities.len()
                || entry
                    .base_index
                    .is_some_and(|index| index >= host.bases.len())
        }) {
            return Err(FlatPlanRefusal::InvalidPlan);
        }
        Ok(Self { host, entries })
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn get(&self, index: usize) -> Option<PlannedGearRef<'a>> {
        let entry = self.entries.get(index)?;
        let offer = self.host.capabilities.iter().nth(entry.capability_index)?;
        let base = entry.base_index.map(|index| {
            self.host
                .bases
                .iter()
                .nth(index)
                .expect("validated base index")
        });
        Some(PlannedGearRef {
            placement_id: entry.placement_id,
            gear_id: entry.gear_id,
            kind_id: offer.kind_id,
            kind_contract_revision: offer.kind_contract_revision,
            execution_profile_id: offer.execution_profile_id,
            host_id: self.host.host_id,
            boot_id: self.host.boot_id,
            capability_id: offer.capability_id,
            implementation_id: offer.implementation_id,
            artifact_id: offer.artifact_id,
            source_span: entry.source_span,
            offer_generation: self.host.offer_generation,
            configuration: &[],
            base: base.map(|base| BaseProviderBindingRef {
                base_id: base.base_id,
                provider_instance_id: base.provider_instance_id,
                provider_generation: base.provider_generation,
                implementation_id: base.implementation_id,
                mechanism_family: base.mechanism_family,
                enforcement_class: base.enforcement_class,
            }),
            realization_characteristics: &[],
            realization_properties: offer.realization_properties,
            limits: offer.limits,
            inputs: offer.inputs,
            outputs: offer.outputs,
            semantic_contract: offer.semantic_contract,
            terminal_transductions: &[],
            host_calls: offer.host_calls,
            resources: entry.resources,
            authority: entry.authority,
            pool_references: &[],
        })
    }
}
