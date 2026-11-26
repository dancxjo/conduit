//! Custody of initialized Backs and exact one-time selected ownership transfer.
//! Application adapters supply their prepared operations and offer identities.
use alloc::vec::Vec;
use conduit_core as c;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoftwareRefusal {
    Source,
    Count,
    Unsupported,
    Contract,
    Initialization,
    Advertisement,
    Selection,
    AlreadySelected,
}
pub struct ReadyOperation<T> {
    pub gear: c::GearId,
    pub configuration: Vec<c::ConfigurationEntry>,
    pub offer: c::CapabilityOffer,
    pub prepared: Option<T>,
}
/// Initialized operation payloads remain owned here until exact selection.
/// Root supplies trusted checked Source and actual Host/Boot; it must separately
/// admit preparation memory, the complete kernel and work before starting Play.
/// This owner is not a Body, a provider for hardware effects or a scheduler.
pub struct ReadyBacks<T> {
    host: c::HostId,
    boot: c::BootId,
    source: c::SourceDocumentId,
    checked: c::CheckedPlotId,
    expanded: c::ExpandedPlotId,
    generation: Option<c::OfferGeneration>,
    operations: Vec<ReadyOperation<T>>,
    metadata_released: bool,
}
impl<T> ReadyBacks<T> {
    /// The trusted preparation owner supplies exact Source provenance and
    /// initialized operations after matching their contracts. This constructor
    /// grants no effect authority and does not validate received metadata.
    pub fn new(
        source: c::PlotIdentity,
        host: c::HostId,
        boot: c::BootId,
        operations: Vec<ReadyOperation<T>>,
    ) -> Self {
        Self {
            host,
            boot,
            source: source.source_document_id,
            checked: source.checked_plot_id,
            expanded: source.expanded_plot_id,
            generation: None,
            operations,
            metadata_released: false,
        }
    }
    /// A refusal leaves the advertisement and retained operations unchanged.
    /// Selected operations are omitted from newly built advertisements. Root
    /// must replace historical snapshots when availability changes.
    pub fn append_to_advertisement(
        &mut self,
        host: &mut c::HostAdvertisement,
    ) -> Result<(), SoftwareRefusal> {
        if host.host_id != self.host
            || host.boot_id != self.boot
            || self
                .operations
                .iter()
                .filter(|op| op.prepared.is_some())
                .any(|op| {
                    host.capabilities
                        .iter()
                        .any(|offer| offer.capability_id == op.offer.capability_id)
                })
        {
            return Err(SoftwareRefusal::Advertisement);
        }
        host.capabilities.extend(
            self.operations
                .iter()
                .filter(|op| op.prepared.is_some())
                .map(|op| op.offer.clone()),
        );
        self.generation = Some(host.offer_generation);
        Ok(())
    }
    /// Match the actual initialized object, not just a Kind descriptor. All
    /// checks precede ownership transfer. Root still owns current-Plan/Play,
    /// memory admission and any independent native Host Call authorization.
    pub fn take_selected(
        &mut self,
        fragment: &c::PlanFragment,
        placement: &c::PlacementId,
    ) -> Result<T, SoftwareRefusal> {
        let index = self.selected_index(fragment, placement)?;
        self.operations[index]
            .prepared
            .take()
            .ok_or(SoftwareRefusal::AlreadySelected)
    }
    /// Retire construction descriptors only after every initialized operation
    /// has transferred. Scope and withdrawn-offer inspection remain available;
    /// this owner can never supply another operation. Refusal changes nothing.
    /// No allocation, replacement implementation or readiness is introduced.
    pub fn release_transferred_metadata(&mut self) -> Result<(), SoftwareRefusal> {
        if self.operations.iter().any(|op| op.prepared.is_some()) {
            return Err(SoftwareRefusal::Selection);
        }
        self.operations = Vec::new();
        self.metadata_released = true;
        Ok(())
    }
    /// Preflight a whole closure before transferring any retained operation.
    pub fn check_selected(
        &self,
        fragment: &c::PlanFragment,
        placement: &c::PlacementId,
    ) -> Result<(), SoftwareRefusal> {
        self.selected_index(fragment, placement).map(|_| ())
    }
    fn selected_index(
        &self,
        fragment: &c::PlanFragment,
        placement: &c::PlacementId,
    ) -> Result<usize, SoftwareRefusal> {
        if !c::verify_plan_fragment(fragment)
            || fragment.host_id != self.host
            || fragment.boot_id != self.boot
            || fragment.source_document_id != self.source
            || fragment.checked_plot_id != self.checked
            || fragment.expanded_plot_id != self.expanded
        {
            return Err(SoftwareRefusal::Selection);
        }
        if self.metadata_released {
            return Err(SoftwareRefusal::AlreadySelected);
        }
        let gear = fragment
            .placements
            .iter()
            .find(|gear| &gear.placement_id == placement)
            .ok_or(SoftwareRefusal::Selection)?;
        let index = self
            .operations
            .iter()
            .position(|op| op.gear == gear.gear_id)
            .ok_or(SoftwareRefusal::Selection)?;
        let op = &self.operations[index];
        let offer = &op.offer;
        if self.generation != Some(gear.offer_generation)
            || gear.host_id != self.host
            || gear.boot_id != self.boot
            || gear.limits != offer.limits
            || gear.configuration != op.configuration
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
            return Err(SoftwareRefusal::Selection);
        }
        if op.prepared.is_none() {
            return Err(SoftwareRefusal::AlreadySelected);
        }
        Ok(index)
    }
}
mod offers;
pub use offers::ReadySoftwareOffers;
