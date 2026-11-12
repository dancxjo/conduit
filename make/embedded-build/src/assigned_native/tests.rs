use super::*;
use conduit_core as c;

fn grant(id: &str, operation: &str, target: &str) -> c::AuthorityBinding {
    c::AuthorityBinding {
        grant_id: c::AuthorityGrantId(id.into()),
        contract_id: c::AuthorityContractId(format!("authority/{operation}")),
        host_call_contract_id: c::HostCallContractId(operation.into()),
        subject_kind: c::kind_id(target),
        host_id: c::HostId("host/local".into()),
        boot_id: c::BootId("boot/local".into()),
        capability_id: c::CapabilityId("capability/residence-service".into()),
    }
}
fn call(index: u16, operation: &str, target: Option<&str>) -> GeneratedHostCall {
    GeneratedHostCall {
        node: 0,
        call: index,
        contract_id: operation.into(),
        target_kind: target.map(str::to_owned),
        maximum_in_flight: 1,
        maximum_input_bytes: 200,
        maximum_output_bytes: 837,
    }
}

#[test]
fn each_call_selects_its_own_original_authority_independent_of_order() {
    let append = grant("grant/append", "operation/append", "resource/append");
    let read = grant("grant/read", "operation/read", "resource/read");
    for authorities in [vec![append.clone(), read.clone()], vec![read, append]] {
        let a = selected_authority(
            &authorities,
            &call(1, "operation/append", Some("resource/append")),
        )
        .unwrap();
        let r = selected_authority(
            &authorities,
            &call(0, "operation/read", Some("resource/read")),
        )
        .unwrap();
        assert_eq!(a.grant_id.as_str(), "grant/append");
        assert_eq!(r.grant_id.as_str(), "grant/read");
    }
}

#[test]
fn wrong_target_missing_operation_and_ambiguous_authority_refuse() {
    let original = grant("grant/read", "operation/read", "resource/read");
    let call = call(0, "operation/read", Some("resource/read"));
    assert!(selected_authority(&[], &call).is_none());
    assert!(selected_authority(
        &[grant("wrong", "operation/append", "resource/read")],
        &call
    )
    .is_none());
    assert!(selected_authority(
        &[grant("wrong", "operation/read", "resource/append")],
        &call
    )
    .is_none());
    assert!(selected_authority(&[original.clone(), original], &call).is_none());
}

#[test]
fn a_targetless_call_cannot_borrow_the_only_native_grant() {
    let authorities = [grant("grant/read", "operation/read", "resource/read")];
    assert!(selected_authority(&authorities, &call(0, "operation/read", None)).is_none());
}
