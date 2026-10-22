//! Inert transcript provider retained through Ready -> Offer -> Plan -> Play.
use conduit_core::*;
use conduit_kernel::NodeId;
use conduit_plan_lowering::{
    lowering::LoweredPlanFragment, selected_operation::SelectedOperationPlan,
};
use conduit_pulse_capture::{installation::*, owner::*, *};
use std::{cell::Cell, rc::Rc};

pub struct Provider {
    pub scenario: usize,
    pub effects: Rc<Cell<usize>>,
}
impl PulseReadProvider for Provider {
    async fn read(
        &mut self,
        request: CheckedPulseRead,
    ) -> Result<PulseReadCapture, PulseReadRefusal> {
        assert_eq!(request.request().octets, 5);
        self.effects.set(self.effects.get() + 1);
        let mut wire = if self.scenario == 5 {
            [53, 0, 10, 130, 193]
        } else {
            [56, 0, 19, 8, 83]
        };
        if self.scenario == 1 {
            wire[4] ^= 1;
        }
        Ok(match self.scenario {
            2 => PulseReadCapture::from_complete_octets(PulseReadDisposition::TimedOut, &wire[..2]),
            3 => PulseReadCapture::from_complete_octets(PulseReadDisposition::ProviderFailed, &[]),
            6 => PulseReadCapture::from_complete_octets(PulseReadDisposition::Complete, &wire[..4]),
            _ => PulseReadCapture::from_complete_octets(PulseReadDisposition::Complete, &wire),
        }
        .unwrap())
    }
}
pub fn identity() -> PulseNativeIdentity {
    PulseNativeIdentity {
        host_id: super::plan::PREFIX.into(),
        boot_id: "fixture/environment/boot".into(),
        base_id: "fixture/pulse-base".into(),
        provider_instance_id: "fixture/pulse-provider".into(),
        provider_generation: 1,
        resource_pool_id: "fixture/pulse-resource".into(),
        resource_generation_id: ResourceGenerationId("fixture/attachment/1".into()),
        envelope_id: "fixture/pulse-envelope".into(),
        artifact_id: "fixture/pulse-artifact".into(),
        implementation_id: "fixture/pulse-read@1".into(),
        base_implementation_id: "fixture/pulse-base@1".into(),
    }
}
pub fn ready(scenario: usize) -> (ReadyPulseBase<Provider>, Rc<Cell<usize>>) {
    ready_with_identity(scenario, identity())
}
pub fn ready_with_identity(
    scenario: usize,
    native: PulseNativeIdentity,
) -> (ReadyPulseBase<Provider>, Rc<Cell<usize>>) {
    let effects = Rc::new(Cell::new(0));
    let attachment = PulseAttachment {
        generation: 1,
        resource_bytes: 64,
        limits: PulseReadLimits {
            maximum_hold_low_us: 50_000,
            maximum_skipped_pulses: 4,
            minimum_sample_offset_us: 10,
            maximum_sample_offset_us: 200,
            maximum_octets: if scenario == 4 { 4 } else { 8 },
            maximum_capture_timeout_us: 50_000,
        },
    };
    // SAFETY: inert fixture owns only this finite transcript, no physical resources.
    let ready = unsafe {
        ReadyPulseBase::new(
            native,
            attachment,
            Provider {
                scenario,
                effects: effects.clone(),
            },
        )
    }
    .unwrap();
    (ready, effects)
}
pub fn grant() -> AuthorityGrant {
    AuthorityGrant {
        grant_id: "fixture/pulse-grant".into(),
        contract_id: PULSE_AUTHORITY.into(),
        host_call_contract_id: PULSE_READ_CALL.into(),
        subject_kind: PULSE_READ_KIND.into(),
        host_id: identity().host_id,
        boot_id: identity().boot_id,
        capability_id: PULSE_CAPABILITY.into(),
    }
}
pub fn bind(
    ready: ReadyPulseBase<Provider>,
    plan: &Plan,
    lowered: &LoweredPlanFragment,
) -> Result<(PulseHostCall<Provider>, NodeId), PulseCallRefusal> {
    bind_mode(ready, plan, lowered, None)
}
pub fn bind_assigned(
    ready: ReadyPulseBase<Provider>,
    plan: &Plan,
    lowered: &LoweredPlanFragment,
    projection: &super::assignment::NativeProjection,
) -> Result<(PulseHostCall<Provider>, NodeId), PulseCallRefusal> {
    bind_mode(ready, plan, lowered, Some(projection))
}
fn bind_mode(
    ready: ReadyPulseBase<Provider>,
    plan: &Plan,
    lowered: &LoweredPlanFragment,
    projection: Option<&super::assignment::NativeProjection>,
) -> Result<(PulseHostCall<Provider>, NodeId), PulseCallRefusal> {
    let fragment = &plan.fragments[0];
    let gear = fragment
        .placements
        .iter()
        .find(|g| g.kind_id.as_str() == PULSE_READ_KIND)
        .unwrap();
    let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let identity = identity();
    let scope = BaseCapabilityScope {
        host_id: identity.host_id.clone(),
        boot_id: identity.boot_id.clone(),
        base_instance_id: identity.provider_instance_id.clone(),
        base_provider_generation: 1,
        plan_id: plan.plan_id.clone(),
        active_play_id: active.active_play_id.clone(),
        authority_grant_id: grant().grant_id,
        authority_contract_id: PULSE_AUTHORITY.into(),
        capability_id: PULSE_CAPABILITY.into(),
        implementation_id: identity.implementation_id.clone(),
        operation_contract_id: PULSE_READ_CALL.into(),
        subject_kind: PULSE_READ_KIND.into(),
        resource_pool_id: identity.resource_pool_id.clone(),
        resource_generation_id: identity.resource_generation_id.clone(),
        envelope_id: identity.envelope_id.clone(),
        maximum_parameter_bytes: 1024,
        maximum_result_bytes: 1024,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 1,
    };
    let authority = BaseCapabilityAuthority {
        grant: grant(),
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: 1,
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        maximum_parameter_bytes: 1024,
        maximum_result_bytes: 1024,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 1,
    };
    let claim = BaseOperationClaim {
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: 1,
        plan_id: scope.plan_id.clone(),
        active_play_id: scope.active_play_id.clone(),
        implementation_id: scope.implementation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        subject_kind: scope.subject_kind.clone(),
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        parameter_bytes: 1024,
        work_units: 1,
    };
    let mut table = BaseCapabilityTable::new(
        identity.host_id,
        identity.boot_id,
        identity.provider_instance_id,
        1,
        [7; 32],
        1,
    )
    .unwrap();
    let handle = table
        .issue(CapabilityIssueRequest { scope, authority })
        .unwrap();
    let node = lowered
        .nodes
        .iter()
        .find(|n| n.placement_id == gear.placement_id)
        .unwrap()
        .node;
    let owner = if let Some(projection) = projection {
        ready.bind_assigned(
            table,
            handle,
            claim,
            &PulseReadContract::prepare().unwrap(),
            conduit_plan_lowering::selected_assigned_operation::SelectedAssignedOperationPlan {
                bytes: &projection.bytes,
                maxima: super::assignment::NativeProjection::maxima(),
                requirements: projection.required(),
                computations: projection.profile(),
                fragment_id: &fragment.fragment_id,
                active: &active,
                node,
            },
        )?
    } else {
        ready.bind_selected(
            table,
            handle,
            claim,
            &PulseReadContract::prepare().unwrap(),
            SelectedOperationPlan {
                fragment,
                lowered,
                active: &active,
                placement_id: &gear.placement_id,
            },
        )?
    };
    Ok((owner, node))
}
