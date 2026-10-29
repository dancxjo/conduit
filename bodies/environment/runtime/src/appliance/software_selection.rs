//! Selection transfers actual initialized software only after exact scope,
//! offer and independently retained configuration checks. No native authority.
use super::{PreparedSoftwareOperation, ReadySoftwareBacks, SoftwareRefusal};
use conduit_core as c;
impl ReadySoftwareBacks {
    /// Transfer from an opaque whole-Plan verification scope without restoring
    /// retired configurations. Root still admits memory, native possession and Play.
    pub fn take_verified(
        &mut self,
        plan: &c::VerifiedPlanMetadata,
        fragment: &c::FragmentId,
        placement: &c::PlacementId,
    ) -> Result<PreparedSoftwareOperation, SoftwareRefusal> {
        let index = self.verified_index(plan, fragment, placement)?;
        self.operations[index]
            .prepared
            .take()
            .ok_or(SoftwareRefusal::AlreadySelected)
    }
    /// Preflight a complete local closure before transferring any software.
    pub fn check_verified(
        &self,
        plan: &c::VerifiedPlanMetadata,
        fragment: &c::FragmentId,
        placement: &c::PlacementId,
    ) -> Result<(), SoftwareRefusal> {
        self.verified_index(plan, fragment, placement).map(|_| ())
    }
    fn verified_index(
        &self,
        plan: &c::VerifiedPlanMetadata,
        fragment: &c::FragmentId,
        placement: &c::PlacementId,
    ) -> Result<usize, SoftwareRefusal> {
        let verified = plan.fragment(fragment).ok_or(SoftwareRefusal::Selection)?;
        self.selected_index_with(
            verified.lowering_metadata(),
            placement,
            |id, expected, _| verified.configuration_matches(id, expected),
        )
    }
    pub(super) fn selected_index(
        &self,
        fragment: &c::PlanFragment,
        placement: &c::PlacementId,
    ) -> Result<usize, SoftwareRefusal> {
        if !c::verify_plan_fragment(fragment) {
            return Err(SoftwareRefusal::Selection);
        }
        self.selected_index_with(fragment, placement, |_, expected, actual| {
            expected == actual
        })
    }
    fn selected_index_with(
        &self,
        fragment: &c::PlanFragment,
        placement: &c::PlacementId,
        configuration_matches: impl FnOnce(
            &c::PlacementId,
            &[c::ConfigurationEntry],
            &[c::ConfigurationEntry],
        ) -> bool,
    ) -> Result<usize, SoftwareRefusal> {
        if fragment.host_id != self.host
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
            || !configuration_matches(placement, &op.configuration, &gear.configuration)
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
