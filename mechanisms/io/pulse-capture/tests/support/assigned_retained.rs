use super::*;
use conduit_plan_lowering::{
    assigned_play_scope::AssignedPlayScope,
    selected_assigned_operation::{AssignedOperationInput, SelectedAssignedOperationPlan},
};
use conduit_pulse_capture::installation::*;
pub(super) fn issued(
    fragment: &PlanFragment,
    active: &ActivePlayIdentity,
) -> (
    BaseCapabilityTable,
    BaseCapabilityHandle,
    BaseOperationClaim,
) {
    let mut scope = fixture::scope();
    scope.maximum_operations = 1;
    scope.capability_id = PULSE_CAPABILITY.into();
    scope.authority_contract_id = PULSE_AUTHORITY.into();
    scope.plan_id = fragment.plan_id.clone();
    scope.active_play_id = active.active_play_id.clone();
    let authority = BaseCapabilityAuthority {
        grant: AuthorityGrant {
            grant_id: scope.authority_grant_id.clone(),
            contract_id: scope.authority_contract_id.clone(),
            host_call_contract_id: scope.operation_contract_id.clone(),
            subject_kind: scope.subject_kind.clone(),
            host_id: scope.host_id.clone(),
            boot_id: scope.boot_id.clone(),
            capability_id: scope.capability_id.clone(),
        },
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: scope.base_provider_generation,
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        maximum_parameter_bytes: scope.maximum_parameter_bytes,
        maximum_result_bytes: scope.maximum_result_bytes,
        maximum_work_units: scope.maximum_work_units,
        maximum_in_flight: 1,
        maximum_operations: scope.maximum_operations,
    };
    let claim = BaseOperationClaim {
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: scope.base_provider_generation,
        plan_id: scope.plan_id.clone(),
        active_play_id: scope.active_play_id.clone(),
        implementation_id: scope.implementation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        subject_kind: scope.subject_kind.clone(),
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        parameter_bytes: PULSE_READ_MAXIMUM_BYTES,
        work_units: 1,
    };
    let mut table = BaseCapabilityTable::new(
        scope.host_id.clone(),
        scope.boot_id.clone(),
        scope.base_instance_id.clone(),
        4,
        [7; 32],
        1,
    )
    .unwrap();
    let handle = table
        .issue(CapabilityIssueRequest { scope, authority })
        .unwrap();
    (table, handle, claim)
}
#[test]
fn assigned_refusal_retains_exact_capture_provider_without_effects() {
    let state = Rc::new(State::default());
    let scope = fixture::scope();
    let identity = PulseNativeIdentity {
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        base_id: "base/pulse".into(),
        provider_instance_id: scope.base_instance_id.clone(),
        provider_generation: scope.base_provider_generation,
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        artifact_id: "artifact/pulse/fixture".into(),
        implementation_id: scope.implementation_id.clone(),
        base_implementation_id: "base/pulse/fixture@1".into(),
    };
    // SAFETY: owned inert provider fixture; it records effects and cancellation.
    let mut slot = Some(
        unsafe {
            ReadyPulseBase::new(
                identity,
                PulseAttachment {
                    generation: 7,
                    resource_bytes: 4096,
                    limits: limits(),
                },
                Provider(state.clone()),
            )
        }
        .unwrap(),
    );
    let (fragment, _, active, _) = fixture::selected();
    let contract = PulseReadContract::prepare().unwrap();
    for wrong_claim in [true, false] {
        let (table, handle, mut claim) = issued(&fragment, &active);
        if wrong_claim {
            claim.boot_id = "wrong-boot".into();
        }
        let selected = SelectedAssignedOperationPlan {
            input: AssignedOperationInput::Contiguous(&[]),
            maxima: AssignedPlanMaxima {
                encoded_bytes: 8192,
                runtime_state_bytes: 8192,
                counts: [32; ASSIGNED_PLAN_COUNT_KINDS],
            },
            requirements: AssignedPlanRequirements {
                host: AssignedIdentity::from_text(fragment.host_id.as_str()),
                boot: AssignedIdentity::from_text(fragment.boot_id.as_str()),
                host_calls: &[],
                resources: &[],
                remote_bindings: &[],
            },
            computations: AssignedComputationProfile {
                maximum_text_bytes: 0,
                maximum_total_text_bytes: 0,
                requirements: &[],
            },
            fragment_id: fragment.fragment_id.as_str(),
            scope: AssignedPlayScope::Plot(&active),
            node: NodeId(0),
        };
        assert!(ReadyPulseBase::bind_assigned_retained(
            &mut slot, table, handle, claim, &contract, selected
        )
        .is_err());
        assert!(slot.is_some());
        assert_eq!(state.effects.get(), 0);
        assert_eq!(state.cancelled.get(), 0);
        // The retained provider still projects its independently supplied identity.
        let mut ad = HostAdvertisement {
            declared_name: None,
            protocol_version: PROTOCOL_VERSION,
            host_id: scope.host_id.clone(),
            boot_id: scope.boot_id.clone(),
            offer_generation: OfferGeneration(1),
            profile: "fixture".into(),
            bases: vec![],
            capabilities: vec![],
            resources: vec![],
            planner_capabilities: vec![],
        };
        slot.as_ref()
            .unwrap()
            .append_to_advertisement(&mut ad, &contract)
            .unwrap();
        assert_eq!(ad.bases[0].provider_instance_id, scope.base_instance_id);
    }
}
