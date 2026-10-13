//! Real issuer/table possession with a sealed selected Plan. Fixture authority
//! is independently retained by each test issuer; no physical Root admission,
//! entropy-source admission or device/controller execution is claimed.
use super::*;
use crate::usb_base::endpoint_read_owner::tests::fixture as read;
use alloc::vec;

fn authority(second: bool) -> BaseCapabilityAuthority {
    let mut authority = read::request().authority;
    if second {
        authority.grant.capability_id = "machine/usb/endpoint-read/second".into();
        authority.grant.grant_id = "grant/second".into();
        authority.base_instance_id = "base/usb/endpoint-read/second".into();
        authority.resource_pool_id = "usb/endpoint-read/second".into();
    }
    authority
}
fn issuer(authority: BaseCapabilityAuthority, key: u8) -> NativeProtocolIssuer {
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
fn plan(duplicate_capability: bool) -> Plan {
    let (mut fragment, _, _, _) = read::selected();
    let mut second = fragment.placements[0].clone();
    second.placement_id = "placement/endpoint-read/second".into();
    second.gear_id = "gear/endpoint-read/second".into();
    let authority = authority(true);
    second.capability_id = if duplicate_capability {
        fragment.placements[0].capability_id.clone()
    } else {
        authority.grant.capability_id.clone()
    };
    second.base.as_mut().unwrap().provider_instance_id = authority.base_instance_id;
    second.resources[0].pool_id = authority.resource_pool_id;
    second.authority[0].grant_id = authority.grant.grant_id;
    second.authority[0].capability_id = second.capability_id.clone();
    fragment.startup_order.push(second.placement_id.clone());
    fragment.expected_sign.extend([
        ExpectedSign::PlacementPrepared(second.placement_id.clone()),
        ExpectedSign::PlacementTerminal(second.placement_id.clone()),
    ]);
    fragment.placements.push(second);
    fragment.sign_storage_budget =
        mandatory_sign_storage_requirement(&fragment.expected_sign).unwrap();
    seal_plan(
        PlotIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_plot_id: fragment.checked_plot_id.clone(),
            expanded_plot_id: fragment.expanded_plot_id.clone(),
        },
        vec![fragment],
    )
}
fn active(plan: &Plan) -> ActivePlayIdentity {
    let fragment = &plan.fragments[0];
    bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0)
}
fn issue(
    plan: &Plan,
    authority: BaseCapabilityAuthority,
    key: u8,
) -> Result<NativeProtocolPossession, NativeProtocolIssueRefusal> {
    issuer(authority, key).issue(
        plan,
        &active(plan),
        &plan.fragments[0].placements[0].implementation_id,
        1,
    )
}
#[test]
fn each_root_held_issuer_selects_its_independent_same_implementation_capability() {
    let plan = plan(false);
    assert!(verify_plan(&plan));
    assert_eq!(
        plan.fragments[0].placements[0].implementation_id,
        plan.fragments[0].placements[1].implementation_id
    );
    for second in [false, true] {
        let expected = authority(second);
        let mut possession = issue(&plan, expected.clone(), if second { 32 } else { 31 }).unwrap();
        let entry = possession.table.inspections().next().unwrap();
        assert_eq!(entry.scope.capability_id, expected.grant.capability_id);
        assert_eq!(entry.scope.authority_grant_id, expected.grant.grant_id);
        assert_eq!(entry.scope.plan_id, plan.plan_id);
        assert_eq!(entry.scope.active_play_id, active(&plan).active_play_id);
        assert_eq!(possession.claim.base_instance_id, expected.base_instance_id);
        assert_eq!(possession.claim.resource_pool_id, expected.resource_pool_id);
        // Authenticate actual possession, not merely descriptive claim metadata.
        let lease = possession
            .table
            .authorize(&possession.handle, &possession.claim)
            .unwrap();
        assert_eq!(
            possession
                .table
                .inspections()
                .next()
                .unwrap()
                .in_flight_operations,
            1
        );
        possession
            .table
            .complete(&mut possession.handle, lease, 0)
            .unwrap();
    }
}
#[test]
fn missing_root_granted_capability_does_not_select_another_same_implementation() {
    let plan = plan(false);
    let mut missing = authority(false);
    missing.grant.capability_id = "machine/usb/endpoint-read/missing".into();
    assert!(matches!(
        issue(&plan, missing, 33),
        Err(NativeProtocolIssueRefusal::WrongSelection)
    ));
}
#[test]
fn matching_capability_with_other_provider_or_grant_still_refuses() {
    let plan = plan(false);
    for change in 0..3 {
        let mut wrong = authority(false);
        match change {
            0 => wrong.base_instance_id = authority(true).base_instance_id,
            1 => wrong.grant.grant_id = authority(true).grant.grant_id,
            _ => wrong.grant.capability_id = authority(true).grant.capability_id,
        }
        assert!(matches!(
            issue(&plan, wrong, 34),
            Err(NativeProtocolIssueRefusal::WrongSelection)
        ));
    }
}
#[test]
fn duplicate_exact_capability_never_mints_possession() {
    let plan = plan(true);
    // Even when a sealed Plan accepts duplicate descriptive capabilities, the
    // actual issuer must reject ambiguity. If Plan law rejects it, issue also
    // refuses before table issuance; neither route grants possession.
    assert!(matches!(
        issue(&plan, authority(false), 35),
        Err(NativeProtocolIssueRefusal::WrongSelection)
    ));
}
#[test]
fn identical_metadata_from_another_issuer_cannot_substitute_possession() {
    let plan = plan(false);
    let original = issue(&plan, authority(false), 36).unwrap();
    let mut replacement = issue(&plan, authority(false), 37).unwrap();
    assert_eq!(original.claim, replacement.claim);
    assert_eq!(
        replacement
            .table
            .authorize(&original.handle, &original.claim)
            .err(),
        Some(BaseCapabilityRefusal::UnknownCapability)
    );
    assert_eq!(
        replacement
            .table
            .inspections()
            .next()
            .unwrap()
            .in_flight_operations,
        0
    );
    let lease = replacement
        .table
        .authorize(&replacement.handle, &replacement.claim)
        .unwrap();
    replacement
        .table
        .complete(&mut replacement.handle, lease, 0)
        .unwrap();
}
