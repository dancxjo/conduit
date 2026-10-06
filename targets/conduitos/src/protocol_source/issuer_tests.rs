//! Cooperative issuance fixtures; deterministic keys establish no native entropy claim.
use super::*;
use crate::usb_base::{
    endpoint_read_factory::ENDPOINT_READ_IMPLEMENTATION,
    endpoint_read_proof_plan::EndpointReadProofSubject, hid_endpoint_proof_plan,
};
use alloc::vec::Vec;

fn fixture_issuer(plan: &Plan, key: u8) -> NativeProtocolIssuer {
    let gear = plan.fragments[0]
        .placements
        .iter()
        .find(|gear| gear.implementation_id.as_str() == ENDPOINT_READ_IMPLEMENTATION)
        .unwrap();
    let base = gear.base.as_ref().unwrap();
    let grant = &gear.authority[0];
    let authority = BaseCapabilityAuthority {
        grant: AuthorityGrant {
            grant_id: grant.grant_id.clone(),
            contract_id: grant.contract_id.clone(),
            host_call_contract_id: grant.host_call_contract_id.clone(),
            subject_kind: grant.subject_kind.clone(),
            host_id: grant.host_id.clone(),
            boot_id: grant.boot_id.clone(),
            capability_id: grant.capability_id.clone(),
        },
        base_instance_id: base.provider_instance_id.clone(),
        base_provider_generation: base.provider_generation,
        resource_pool_id: gear.resources[0].pool_id.clone(),
        resource_generation_id: ResourceGenerationId("fixture/capture-generation".into()),
        operation_contract_id: gear.host_calls[0].contract_id.clone(),
        envelope_id: "fixture/capture-envelope".into(),
        maximum_parameter_bytes: 4096,
        maximum_result_bytes: 4096,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 128,
    };
    let table = BaseCapabilityTable::new(
        authority.grant.host_id.clone(),
        authority.grant.boot_id.clone(),
        authority.base_instance_id.clone(),
        authority.base_provider_generation,
        [key; 32],
        1,
    )
    .unwrap();
    NativeProtocolIssuer { authority, table }
}

#[test]
fn exact_window_issuance_preserves_independent_possession_and_all_selection_fences() {
    for (entry, members) in [
        ("usb-hid-keyboard-capture-window", 8),
        ("usb-hid-mouse-capture-window", 2),
    ] {
        let artifact = hid_endpoint_proof_plan::plan(
            &EndpointReadProofSubject {
                host_id: "fixture/host",
                boot_id: "fixture/boot",
                controller_base_id: "fixture/controller",
                device_instance_id: "fixture/device",
                root_port: 1,
                slot: 1,
                attachment_epoch: 1,
                endpoint_dci: 3,
                endpoint_epoch: 1,
            },
            entry,
        )
        .unwrap();
        let plan = &artifact.artifact().definition().internal_plan;
        let fragment = &plan.fragments[0];
        let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        let implementation = ImplementationId::from(ENDPOINT_READ_IMPLEMENTATION);
        let placements: Vec<_> = fragment
            .placements
            .iter()
            .filter(|gear| gear.implementation_id == implementation)
            .map(|gear| &gear.placement_id)
            .collect();
        assert_eq!(placements.len(), members);
        assert!(
            fixture_issuer(plan, 1)
                .issue(plan, &active, &implementation, 1)
                .is_err()
        );
        let mut issued: Vec<_> = placements
            .iter()
            .enumerate()
            .map(|(index, placement)| {
                fixture_issuer(plan, 31 + index as u8)
                    .issue_selected(plan, &active, &implementation, placement, 1)
                    .unwrap()
            })
            .collect();
        for index in 0..members {
            let (before, rest) = issued.split_at_mut(index);
            let (owner, after) = rest.split_first_mut().unwrap();
            for foreign in before.iter().chain(after.iter()) {
                assert!(
                    owner
                        .table
                        .authorize(&foreign.handle, &owner.claim)
                        .is_err()
                );
            }
            let lease = owner.table.authorize(&owner.handle, &owner.claim).unwrap();
            owner.table.complete(&owner.handle, lease, 0).unwrap();
        }
        let placement = placements[0];
        for field in ["boot", "host", "plan", "play"] {
            let mut stale = active.clone();
            match field {
                "boot" => stale.boot_id = "fixture/stale-boot".into(),
                "host" => stale.host_id = "fixture/other-host".into(),
                "plan" => stale.plan_id = "fixture/other-plan".into(),
                "play" => stale.active_play_id = "fixture/other-play".into(),
                _ => unreachable!(),
            }
            assert!(
                fixture_issuer(plan, 1)
                    .issue_selected(plan, &stale, &implementation, placement, 1)
                    .is_err(),
                "{field}"
            );
        }
        for work in [0, 2] {
            assert!(
                fixture_issuer(plan, 1)
                    .issue_selected(plan, &active, &implementation, placement, work)
                    .is_err()
            );
        }
        assert!(
            fixture_issuer(plan, 1)
                .issue_selected(plan, &active, &implementation, &"fixture/absent".into(), 1)
                .is_err()
        );
        let pure = fragment
            .placements
            .iter()
            .find(|gear| gear.implementation_id != implementation)
            .unwrap();
        assert!(
            fixture_issuer(plan, 1)
                .issue_selected(plan, &active, &implementation, &pure.placement_id, 1)
                .is_err()
        );
        let mut wrong = fixture_issuer(plan, 1);
        wrong.authority.base_provider_generation += 1;
        assert!(
            wrong
                .issue_selected(plan, &active, &implementation, placement, 1)
                .is_err()
        );
        let mut wrong = fixture_issuer(plan, 1);
        wrong.authority.grant.grant_id = "fixture/other-grant".into();
        assert!(
            wrong
                .issue_selected(plan, &active, &implementation, placement, 1)
                .is_err()
        );
    }
}
