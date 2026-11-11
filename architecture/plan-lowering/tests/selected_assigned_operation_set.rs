//! Preparation-only checks: no provider or admission is manufactured here.
extern crate alloc;
use conduit_core::*;
use conduit_kernel::NodeId;
use conduit_plan_lowering::{
    assigned_play_scope::AssignedPlayScope,
    lowering::{lower_plan_fragment, LoweredPlanFragment},
    selected_assigned_operation::{AssignedOperationInput, SelectedAssignedOperationPlan},
    selected_assigned_operation_set::*,
    selected_operation::SelectedOperationPlan,
    selected_operation_set::*,
};
#[allow(dead_code)]
#[path = "selected_operation_set/fixture.rs"]
mod fixture;
#[path = "selected_assigned_operation_set/projection.rs"]
mod projection;
#[path = "../../../targets/conduitos/src/machine_membrane/selection_fixture.rs"]
mod selection_fixture;
use fixture::*;
use projection::*;

fn check(
    f: &Fixture,
    bytes: &[u8],
    operations: &[&BoundBaseOperation],
    borrowed: bool,
) -> Result<SelectedOperationSet, SelectedOperationSetRefusal> {
    let records: Vec<_> = AssignedPlanRecords::new(bytes)
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let input = if borrowed {
        AssignedOperationInput::Records {
            header: &bytes[..ASSIGNED_PLAN_HEADER_BYTES],
            records: &records,
        }
    } else {
        AssignedOperationInput::Contiguous(bytes)
    };
    let calls = f.calls();
    let call_pins: Vec<_> = f
        .scopes
        .iter()
        .map(|s| AssignedIdentity::from_text(s.operation_contract_id.as_str()))
        .collect();
    let authorities: Vec<_> = f
        .scopes
        .iter()
        .map(|s| s.authority_contract_id.as_str())
        .collect();
    let gear = &f.fragment.placements[0];
    let base = gear.base.as_ref().unwrap();
    check_selected_assigned_operation_set(
        operations,
        SelectedServiceContract {
            kind: &f.kind,
            configuration: &[],
            resource: &f.resource,
            requirement: &f.requirement,
            resource_generation: &f.scopes[0].resource_generation_id,
            envelope: &f.scopes[0].envelope_id,
            calls: &calls,
        },
        AssignedServiceNativeIdentity {
            capability: gear.capability_id.as_str(),
            execution_profile: gear.execution_profile_id.as_str(),
            artifact: gear.artifact_id.as_str(),
            base: base.base_id.as_str(),
            base_implementation: base.implementation_id.as_str(),
            mechanism: base.mechanism_family.as_str(),
            resource_class: f.resource.class_id.as_str(),
            authorities: &authorities,
        },
        SelectedAssignedServicePlan {
            operation: SelectedAssignedOperationPlan {
                input,
                maxima: AssignedPlanMaxima {
                    encoded_bytes: 4096,
                    runtime_state_bytes: 4096,
                    counts: [32; ASSIGNED_PLAN_COUNT_KINDS],
                },
                requirements: AssignedPlanRequirements {
                    host: AssignedIdentity::from_text(f.fragment.host_id.as_str()),
                    boot: AssignedIdentity::from_text(f.fragment.boot_id.as_str()),
                    host_calls: &call_pins,
                    resources: &[0],
                    remote_bindings: &[],
                },
                computations: AssignedComputationProfile {
                    maximum_text_bytes: 0,
                    maximum_total_text_bytes: 0,
                    requirements: &[],
                },
                fragment_id: f.fragment.fragment_id.as_str(),
                scope: AssignedPlayScope::Plot(&f.active),
                node: NodeId(0),
            },
            fore: None,
            resource: 0,
        },
    )
}
#[test]
fn both_storage_forms_check_two_targets_without_claiming_reservation() {
    let mut f = Fixture::new();
    f.owner.release(&f.fragment.plan_id, &f.placement).unwrap();
    assert!(f.owner.admissions().is_empty());
    let a = operation(f.scopes[0].clone());
    let r = operation(f.scopes[1].clone());
    let before = [a.inspection().clone(), r.inspection().clone()];
    let bytes = project(&f, |_, _| {});
    for borrowed in [false, true] {
        let result = check(&f, &bytes, &[&a, &r], borrowed).unwrap();
        assert_eq!(result.routes[0].unwrap().call.0, 0);
        assert_eq!(result.routes[1].unwrap().call.0, 1);
        assert_eq!(result.routes[1].unwrap().input.0, 1);
        assert_eq!(result.routes[1].unwrap().output.0, 1);
    }
    assert_eq!([a.inspection().clone(), r.inspection().clone()], before);
    assert!(f.owner.admissions().is_empty());
}
#[test]
fn independently_resealed_wrong_grant_target_call_and_native_pool_refuse() {
    let f = Fixture::new();
    let a = operation(f.scopes[0].clone());
    let r = operation(f.scopes[1].clone());
    for edit in 0..6 {
        let bytes = project(&f, |prefix, b| match edit {
            0 => b.grant = AssignedIdentity::from_text("foreign/grant"),
            1 => prefix[20..36].copy_from_slice(&AssignedIdentity::from_text("foreign/target").0),
            2 => prefix[2..4].copy_from_slice(&7u16.to_le_bytes()),
            3 => b.pool = AssignedIdentity::from_text("foreign/pool"),
            4 => b.authority = AssignedIdentity::from_text("foreign/authority"),
            _ => b.resource = 1,
        });
        for borrowed in [false, true] {
            assert!(
                check(&f, &bytes, &[&a, &r], borrowed).is_err(),
                "edit {edit}"
            );
        }
    }
}
#[test]
fn actual_operations_cannot_substitute_stale_scope_or_spent_possession() {
    let f = Fixture::new();
    let a = operation(f.scopes[0].clone());
    let bytes = project(&f, |_, _| {});
    for edit in 0..6 {
        let mut s = f.scopes[1].clone();
        match edit {
            0 => s.authority_grant_id = "different/grant".into(),
            1 => s.resource_generation_id = ResourceGenerationId("different/generation".into()),
            2 => s.envelope_id = "different/envelope".into(),
            3 => s.boot_id = "different/boot".into(),
            4 => s.base_provider_generation += 1,
            _ => s.active_play_id = "different/play".into(),
        }
        let r = operation(s);
        assert!(check(&f, &bytes, &[&a, &r], false).is_err());
    }
    let mut r = operation(f.scopes[1].clone());
    let lease = r.authorize().unwrap();
    assert_eq!(
        check(&f, &bytes, &[&a, &r], false),
        Err(SelectedOperationSetRefusal::Possession)
    );
    r.complete(lease, 8).unwrap();
    assert_eq!(
        check(&f, &bytes, &[&a, &r], false),
        Err(SelectedOperationSetRefusal::Possession)
    );
    let mut r = operation(f.scopes[1].clone());
    r.revoke().unwrap();
    assert_eq!(
        check(&f, &bytes, &[&a, &r], false),
        Err(SelectedOperationSetRefusal::Possession)
    );
}
#[test]
fn original_narrow_claim_and_stale_active_identity_refuse() {
    let mut f = Fixture::new();
    let a = operation(f.scopes[0].clone());
    let bytes = project(&f, |_, _| {});
    let r = operation_with_claim(f.scopes[1].clone(), |c| c.work_units = 1)
        .ok()
        .unwrap();
    assert_eq!(
        check(&f, &bytes, &[&a, &r], false),
        Err(SelectedOperationSetRefusal::Bounds)
    );
    let r = operation(f.scopes[1].clone());
    f.active.play_sequence += 1;
    assert!(check(&f, &bytes, &[&a, &r], false).is_err());
}
