//! Preparation verifies selected Plan truth; it never grants mapping authority.

use conduit_core::{ActivePlayIdentity, PlacementId, PlanFragment};
use conduit_kernel::{HostCallId, PortId};
use conduit_plan_lowering::lowering::LoweredPlanFragment;

use super::{
    REGISTER_CALL, REGISTER_KIND, RegisterLeaf, RegisterRefusal,
    register_call::{REGISTER_REQUEST_BYTES, REGISTER_RESULT_BYTES, RegisterHostCall, contract},
};

impl RegisterHostCall {
    /// Bind existing opaque possession to an exact selected, lowered operation.
    /// The caller must have checked/admitted the fragment through the ordinary
    /// planning boundary. A serialized Plan is not authority: every invocation
    /// still requires the leaf's unforgeable current Base possession.
    pub fn bind_selected(
        leaf: RegisterLeaf,
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
        active: &ActivePlayIdentity,
        placement_id: &PlacementId,
    ) -> Result<Self, RegisterRefusal> {
        let claim = &leaf.claim;
        if !conduit_core::verify_plan_fragment(fragment)
            || fragment.plan_id != claim.plan_id
            || fragment.host_id != claim.host_id
            || fragment.boot_id != claim.boot_id
            || active.plan_id != claim.plan_id
            || active.host_id != claim.host_id
            || active.boot_id != claim.boot_id
            || active.active_play_id != claim.active_play_id
            || conduit_core::bind_active_play(
                &claim.plan_id,
                &claim.host_id,
                &claim.boot_id,
                active.play_sequence,
            ) != *active
            || lowered.identity.plan_id != fragment.plan_id
            || lowered.identity.fragment_id != fragment.fragment_id
        {
            return Err(RegisterRefusal::WrongBinding);
        }
        // Matching IDs alone do not prove that numeric routing/storage/terminal
        // tables came from this fragment. Preparation may allocate; Play may
        // only use the exact ordinary lowering, with no substituted tables.
        if conduit_plan_lowering::lowering::lower_plan_fragment(fragment)
            .map_err(|_| RegisterRefusal::WrongBinding)?
            != *lowered
        {
            return Err(RegisterRefusal::WrongBinding);
        }
        let mut matches = fragment
            .placements
            .iter()
            .filter(|gear| &gear.placement_id == placement_id);
        let gear = matches.next().ok_or(RegisterRefusal::WrongBinding)?;
        if matches.next().is_some() {
            return Err(RegisterRefusal::WrongBinding);
        }
        let expected = contract();
        let base = gear.base.as_ref().ok_or(RegisterRefusal::WrongBinding)?;
        if gear.kind_id != expected.kind_id
            || gear.kind_contract_revision != expected.kind_contract_revision
            || gear.inputs != expected.inputs
            || gear.outputs != expected.outputs
            || !gear.configuration.is_empty()
            || gear.host_id != claim.host_id
            || gear.boot_id != claim.boot_id
            || gear.implementation_id != claim.implementation_id
            || base.provider_instance_id != claim.base_instance_id
            || base.provider_generation != claim.base_provider_generation
            || claim.subject_kind.as_str() != REGISTER_KIND
            || claim.operation_contract_id.as_str() != REGISTER_CALL
        {
            return Err(RegisterRefusal::WrongBinding);
        }
        let inspection = leaf
            .table
            .inspections()
            .next()
            .ok_or(RegisterRefusal::Possession)?;
        if inspection.lifecycle != conduit_core::CapabilityLifecycle::Issued
            || inspection.in_flight_operations != 0
        {
            return Err(RegisterRefusal::Possession);
        }
        let scope = &inspection.scope;
        if scope.host_id != claim.host_id
            || scope.boot_id != claim.boot_id
            || scope.plan_id != claim.plan_id
            || scope.active_play_id != claim.active_play_id
            || scope.base_instance_id != claim.base_instance_id
            || scope.base_provider_generation != claim.base_provider_generation
            || scope.implementation_id != claim.implementation_id
            || scope.operation_contract_id != claim.operation_contract_id
            || scope.subject_kind != claim.subject_kind
            || scope.resource_pool_id != claim.resource_pool_id
            || scope.resource_generation_id != claim.resource_generation_id
            || scope.envelope_id != claim.envelope_id
            || gear.capability_id != scope.capability_id
            || !gear.authority.iter().any(|authority| {
                authority.grant_id == scope.authority_grant_id
                    && authority.contract_id == scope.authority_contract_id
                    && authority.host_call_contract_id == scope.operation_contract_id
                    && authority.subject_kind == scope.subject_kind
                    && authority.host_id == scope.host_id
                    && authority.boot_id == scope.boot_id
                    && authority.capability_id == scope.capability_id
            })
            || !gear.resources.iter().any(|resource| {
                resource.pool_id == scope.resource_pool_id
                    && resource.units > 0
                    && resource.protected.as_ref().is_none_or(|protected| {
                        protected.maximum_bytes >= u64::from(leaf.envelope.bytes)
                    })
            })
        {
            return Err(RegisterRefusal::WrongBinding);
        }
        let [required] = gear.host_calls.as_slice() else {
            return Err(RegisterRefusal::WrongBinding);
        };
        if required.contract_id.as_str() != REGISTER_CALL
            || required.target_kind.as_ref() != Some(&expected.kind_id)
            || required.maximum_in_flight != 1
            || required.maximum_input_bytes != REGISTER_REQUEST_BYTES
            || required.maximum_output_bytes != REGISTER_RESULT_BYTES
        {
            return Err(RegisterRefusal::WrongBinding);
        }
        let mut nodes = lowered
            .nodes
            .iter()
            .filter(|node| &node.placement_id == placement_id);
        let node = nodes.next().ok_or(RegisterRefusal::WrongBinding)?;
        if nodes.next().is_some()
            || !lowered
                .identity
                .placements
                .contains(&(node.node, placement_id.clone()))
            || node.inputs.len() != 1
            || node.outputs.len() != 1
            || node.inputs[0].port != PortId(0)
            || node.outputs[0].port != PortId(0)
            || node.inputs[0].value_kind != expected.inputs[0].value_kind
            || node.outputs[0].value_kind != expected.outputs[0].value_kind
        {
            return Err(RegisterRefusal::WrongBinding);
        }
        let mut calls = lowered
            .host_calls
            .iter()
            .filter(|call| call.node == node.node);
        let call = calls.next().ok_or(RegisterRefusal::WrongBinding)?;
        if calls.next().is_some()
            || call.call != HostCallId(0)
            || call.binding.call != call.call
            || call.contract_id != required.contract_id
            || call.target_kind != required.target_kind
            || call.maximum_in_flight != 1
            || call.binding.maximum_input_bytes != REGISTER_REQUEST_BYTES
            || call.binding.maximum_output_bytes != REGISTER_RESULT_BYTES
        {
            return Err(RegisterRefusal::WrongBinding);
        }
        let node = node.node;
        Ok(Self { node, leaf })
    }
}
