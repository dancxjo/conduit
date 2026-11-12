use super::*;
use alloc::vec::Vec;
use conduit_core::{
    AssignedComputationProfile, AssignedIdentity, AssignedPlanMaxima, AssignedPlanRecords,
    AssignedPlanRequirements, AssignedRecord, ASSIGNED_PLAN_COUNT_KINDS,
    ASSIGNED_PLAN_HEADER_BYTES,
};
use conduit_plan_lowering::{
    assigned_play_scope::AssignedPlayScope,
    selected_assigned_operation::{AssignedOperationInput, SelectedAssignedOperationPlan},
};
#[path = "fixture.rs"]
mod fixture;
#[path = "rich.rs"]
mod rich;
use fixture::Storage;
use rich::{grant, operation, Fixture};
fn ready(f: &Fixture) -> ReadyRecordService<Storage> {
    let gear = &f.fragment.placements[0];
    let base = gear.base.as_ref().unwrap();
    let s = &f.scopes[0];
    let identity = AppendNativeIdentity {
        host_id: s.host_id.clone(),
        boot_id: s.boot_id.clone(),
        base_id: base.base_id.clone(),
        provider_instance_id: base.provider_instance_id.clone(),
        provider_generation: base.provider_generation,
        resource_pool_id: s.resource_pool_id.clone(),
        resource_generation_id: s.resource_generation_id.clone(),
        envelope_id: s.envelope_id.clone(),
        artifact_id: gear.artifact_id.clone(),
        implementation_id: gear.implementation_id.clone(),
        base_implementation_id: base.implementation_id.clone(),
    };
    // SAFETY: sole CPU residence, independently issued test grants and bounded IO.
    unsafe {
        ReadyRecordService::new_retained(
            identity,
            fixture::recovered_provider(),
            [grant(&f.scopes[0]), grant(&f.scopes[1])],
            ServiceLimits {
                append_operations: 16,
                read_operations: 16,
            },
        )
    }
    .unwrap_or_else(|_| panic!("ready"))
}
fn image(f: &Fixture) -> conduit_embedded_build::GeneratedEmbeddedPlan {
    use conduit_embedded_build::*;
    generate_embedded_plan(
        &f.fragment,
        &f.lowered,
        EmbeddedImageBounds {
            maximum_nodes: 1,
            maximum_cords: 0,
            maximum_routes: 4,
            maximum_route_targets: 4,
            maximum_host_calls: 2,
            maximum_resources: 1,
            maximum_sign_expectations: 32,
            maximum_configuration_entries: 0,
            maximum_ports_per_node: 8,
            maximum_remote_endpoints: 0,
            maximum_cord_value_slots: 4,
            maximum_cord_value_bytes: 4096,
            maximum_sign_items: 64,
            maximum_sign_bytes: 8192,
        },
    )
    .unwrap()
}
fn maxima() -> AssignedPlanMaxima {
    AssignedPlanMaxima {
        encoded_bytes: 8192,
        runtime_state_bytes: 16384,
        counts: [32; ASSIGNED_PLAN_COUNT_KINDS],
    }
}
fn selected<'a>(
    f: &'a Fixture,
    bytes: &'a [u8],
    records: Option<&'a [AssignedRecord<'a>]>,
    calls: &'a [AssignedIdentity],
    resource: u16,
) -> SelectedAssignedServicePlan<'a> {
    SelectedAssignedServicePlan {
        operation: SelectedAssignedOperationPlan {
            input: if let Some(records) = records {
                AssignedOperationInput::Records {
                    header: &bytes[..ASSIGNED_PLAN_HEADER_BYTES],
                    records,
                }
            } else {
                AssignedOperationInput::Contiguous(bytes)
            },
            maxima: maxima(),
            requirements: AssignedPlanRequirements {
                host: AssignedIdentity::from_text(f.fragment.host_id.as_str()),
                boot: AssignedIdentity::from_text(f.fragment.boot_id.as_str()),
                host_calls: calls,
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
        resource,
    }
}
fn calls(f: &Fixture) -> Vec<AssignedIdentity> {
    f.scopes
        .iter()
        .map(|s| AssignedIdentity::from_text(s.operation_contract_id.as_str()))
        .collect()
}
#[test]
fn actual_make_native_encoding_prepares_both_forms_and_binds_only_after_real_admission() {
    let f = Fixture::new();
    let image = image(&f);
    let bytes = conduit_embedded_build::encode_assigned_native_plan(&image, maxima()).unwrap();
    let records: Vec<_> = AssignedPlanRecords::new(&bytes)
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let calls = calls(&f);
    for borrowed in [false, true] {
        let mut slot = Some(ready(&f));
        let operations = [
            operation(f.scopes[0].clone()),
            operation(f.scopes[1].clone()),
        ];
        let prepared = ReadyRecordService::prepare_assigned_retained(
            &mut slot,
            operations,
            &f.resource,
            selected(
                &f,
                &bytes,
                if borrowed { Some(&records) } else { None },
                &calls,
                0,
            ),
        )
        .unwrap_or_else(|_| panic!("actual Make assigned service"));
        println!(
            "assigned service: ticket_inline={}, operation_retained_heap={}",
            core::mem::size_of::<PreparedAssignedRecordService<'_, Storage>>(),
            prepared.operation_backing_bytes().unwrap()
        );
        assert_eq!(f.owner.admissions().len(), 1);
        assert_eq!(f.owner.admissions()[0].items.len(), 1);
        // SAFETY: CPU test owns the exact currently admitted capacity-one ledger
        // proven by the production rich fixture and the sole Ready residence.
        let owner = unsafe { prepared.bind_after_resource_admission() };
        assert!(slot.is_none());
        assert_eq!(owner.node(), NodeId(0));
    }
}
#[test]
fn wrong_original_grant_call_and_resource_refuse_without_consuming_ready() {
    let f = Fixture::new();
    let calls = calls(&f);
    for edit in 0..3 {
        let image = image(&f);
        let bytes = conduit_embedded_build::encode_assigned_native_plan(&image, maxima()).unwrap();
        let mut read = f.scopes[1].clone();
        if edit == 0 {
            read.authority_grant_id = "grant/foreign".into();
        } else if edit == 1 {
            read.operation_contract_id = "operation/foreign".into();
        }
        let operations = [operation(f.scopes[0].clone()), operation(read)];
        let mut slot = Some(ready(&f));
        let (returned, _) = match ReadyRecordService::prepare_assigned_retained(
            &mut slot,
            operations,
            &f.resource,
            selected(&f, &bytes, None, &calls, if edit == 2 { 1 } else { 0 }),
        ) {
            Err(value) => value,
            Ok(_) => panic!("negative admitted {edit}"),
        };
        assert!(slot.is_some());
        assert_eq!(returned[0].inspection().completed_operations, 0);
        assert_eq!(returned[1].inspection().completed_operations, 0);
    }
}
#[test]
fn checked_assignment_does_not_supply_the_missing_start_reservation() {
    let mut f = Fixture::new();
    f.owner.release(&f.fragment.plan_id, &f.placement).unwrap();
    assert!(f.owner.admissions().is_empty());
    let bytes = conduit_embedded_build::encode_assigned_native_plan(&image(&f), maxima()).unwrap();
    let calls = calls(&f);
    let mut slot = Some(ready(&f));
    let prepared = ReadyRecordService::prepare_assigned_retained(
        &mut slot,
        [
            operation(f.scopes[0].clone()),
            operation(f.scopes[1].clone()),
        ],
        &f.resource,
        selected(&f, &bytes, None, &calls, 0),
    )
    .unwrap_or_else(|_| panic!("preparation independent of ledger"));
    // No safe bind exists on this ticket. Without actual Start admission it
    // must be dropped; the original physical owner remains retained.
    drop(prepared);
    assert!(slot.is_some());
    assert!(f.owner.admissions().is_empty());
}

#[test]
fn explicit_line_fore_service_keeps_ready_and_opaque_operations_on_refusal() {
    let f = Fixture::new();
    let calls = calls(&f);
    // Same native records, explicit schema8 with no external routes in this
    // CPU preparation fixture. This grants no live Line or Start authority.
    let mut bytes =
        conduit_embedded_build::encode_assigned_native_plan(&image(&f), maxima()).unwrap();
    bytes[8..10].copy_from_slice(&c::ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA.to_le_bytes());
    let descriptor = || SelectedAssignedLineForeServicePlan {
        service: selected(&f, &bytes, None, &calls, 0),
        fore: c::AssignedLineForeProfile {
            local: &[],
            lines: &[],
        },
    };
    let ops = || {
        [
            operation(f.scopes[0].clone()),
            operation(f.scopes[1].clone()),
        ]
    };
    let mut slot = Some(ready(&f));
    assert!(ReadyRecordService::prepare_assigned_retained(
        &mut slot,
        ops(),
        &f.resource,
        descriptor().service
    )
    .is_err());
    assert!(slot.is_some());
    for edit in 0..4 {
        let mut read = f.scopes[1].clone();
        if edit == 0 {
            read.authority_grant_id = "foreign-grant".into();
        }
        if edit == 1 {
            read.operation_contract_id = "foreign-operation".into();
        }
        let mut candidate = descriptor();
        if edit == 2 {
            candidate.service.resource = 1;
        }
        if edit == 3 {
            candidate.service.fore = Some(&[]);
        }
        let operations = [operation(f.scopes[0].clone()), operation(read)];
        let returned = match ReadyRecordService::prepare_assigned_line_fore_retained(
            &mut slot,
            operations,
            &f.resource,
            candidate,
        ) {
            Err((returned, _)) => returned,
            Ok(_) => panic!("foreign native preparation admitted"),
        };
        assert!(slot.is_some());
        assert_eq!(returned[0].inspection().completed_operations, 0);
        assert_eq!(returned[1].inspection().completed_operations, 0);
    }
    let prepared = ReadyRecordService::prepare_assigned_line_fore_retained(
        &mut slot,
        ops(),
        &f.resource,
        descriptor(),
    )
    .unwrap_or_else(|_| panic!("same checked service schema8"));
    assert_eq!(prepared.node(), NodeId(0));
    drop(prepared);
    assert!(slot.is_some());
    let prepared = ReadyRecordService::prepare_assigned_line_fore_retained(
        &mut slot,
        ops(),
        &f.resource,
        descriptor(),
    )
    .unwrap_or_else(|_| panic!("same checked service schema8 after drop"));
    // SAFETY: existing rich CPU fixture holds the actual exact capacity-one
    // reservation; no new authority/admission is inferred from schema8.
    let owner = unsafe { prepared.bind_after_resource_admission() };
    assert!(slot.is_none());
    assert_eq!(owner.node(), NodeId(0));
}
