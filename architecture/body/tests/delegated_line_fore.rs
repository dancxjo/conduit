#![cfg(feature = "authenticated-admission")]
//! Authenticated preparation only: these fixtures own no carrier or runtime grant.
use conduit_body::*;
use conduit_core as c;
#[allow(dead_code)]
#[path = "delegated_line_fore/support.rs"]
mod support;
use support::*;
#[test]
fn explicit_profile_authenticates_mixed_schema8_with_canonical_metadata_inventory() {
    let f = Fixture::new();
    let projection = projection(c::ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA);
    let bytes = document(&projection, "body/1", "boot/1", &inventory(), &[]);
    let local = [local()];
    let lines = [line(7)];
    let bindings = [binding()];
    let (fragment, ports) = receive_delegated_body_fragment_with_line_fore::<2>(
        Some(&policy()),
        f.context(),
        command(&f, &bytes),
        profile(&local, &lines, &bindings),
    )
    .unwrap();
    assert_eq!(fragment.projection(), projection);
    assert_eq!(fragment.metadata().body, "body/1");
    assert_eq!(ports.as_slice(), [remote(), local[0]]);
    // The original legacy APIs still refuse schema 8, including the trusted
    // authoring ABI approval entrance. ABI approval cannot add Line pins.
    let mut old = profile(&local, &lines, &bindings).preparation;
    let fore = [remote(), local[0]];
    old.fore = Some(&fore);
    assert!(receive_delegated_body_fragment(
        Some(&policy()),
        f.context(),
        command(&f, &bytes),
        old
    )
    .is_err());
    assert!(receive_delegated_body_fragment_with_approved_fore::<2>(
        Some(&policy()),
        f.context(),
        command(&f, &bytes),
        profile(&local, &lines, &bindings).preparation
    )
    .is_err());
}
#[test]
fn missing_delegation_bad_signature_stale_context_and_foreign_line_refuse() {
    let f = Fixture::new();
    let projection = projection(c::ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA);
    let bytes = document(&projection, "body/1", "boot/1", &inventory(), &[]);
    let local = [local()];
    let lines = [line(7)];
    let foreign = [line(8)];
    let bindings = [binding()];
    assert!(slot(&f).authenticate(&bytes, &[0; 64], 11).is_err());
    assert!(receive_delegated_body_fragment_with_line_fore::<2>(
        None,
        f.context(),
        command(&f, &bytes),
        profile(&local, &lines, &bindings)
    )
    .is_err());
    let mut stale = f.context();
    stale.now_millis = 30;
    assert!(receive_delegated_body_fragment_with_line_fore::<2>(
        Some(&policy()),
        stale,
        command(&f, &bytes),
        profile(&local, &lines, &bindings)
    )
    .is_err());
    assert!(receive_delegated_body_fragment_with_line_fore::<2>(
        Some(&policy()),
        f.context(),
        command(&f, &bytes),
        profile(&local, &foreign, &bindings)
    )
    .is_err());
}
#[test]
fn metadata_scope_inventory_capacity_resources_and_schema7_cannot_bypass_checks() {
    let f = Fixture::new();
    let projection = projection(c::ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA);
    let local = [local()];
    let lines = [line(7)];
    let bindings = [binding()];
    let reversed = [local[0].payload().as_slice(), remote().payload().as_slice()].concat();
    for bytes in [
        document(&projection, "body/other", "boot/1", &inventory(), &[]),
        document(&projection, "body/1", "boot/other", &inventory(), &[]),
        document(&projection, "body/1", "boot/1", &reversed, &[]),
        document(&projection, "body/1", "boot/1", &inventory(), &[255]),
    ] {
        assert!(receive_delegated_body_fragment_with_line_fore::<2>(
            Some(&policy()),
            f.context(),
            command(&f, &bytes),
            profile(&local, &lines, &bindings)
        )
        .is_err());
    }
    let invalid_resources = document(&projection, "body/1", "boot/1", &inventory(), &[255]);
    assert!(matches!(
        receive_delegated_body_fragment_with_line_fore::<2>(
            Some(&policy()),
            f.context(),
            command(&f, &invalid_resources),
            profile(&local, &lines, &bindings)
        ),
        Err(DelegatedFragmentRefusal::Resources(_))
    ));
    let bytes = document(&projection, "body/1", "boot/1", &inventory(), &[]);
    assert!(receive_delegated_body_fragment_with_line_fore::<1>(
        Some(&policy()),
        f.context(),
        command(&f, &bytes),
        profile(&local, &lines, &bindings)
    )
    .is_err());
    let mut conflicting = profile(&local, &lines, &bindings);
    conflicting.preparation.fore = Some(&local);
    assert!(matches!(
        receive_delegated_body_fragment_with_line_fore::<2>(
            Some(&policy()),
            f.context(),
            command(&f, &bytes),
            conflicting
        ),
        Err(DelegatedFragmentRefusal::ForeApproval)
    ));
    let narrow = HostPreparationDelegation {
        maximum_runtime_state_bytes: 64,
        ..policy()
    };
    assert!(matches!(
        receive_delegated_body_fragment_with_line_fore::<2>(
            Some(&narrow),
            f.context(),
            command(&f, &bytes),
            profile(&local, &lines, &bindings)
        ),
        Err(DelegatedFragmentRefusal::Assignment(_))
    ));
    let schema7 = projection_with_schema7();
    let bytes = document(&schema7, "body/1", "boot/1", &inventory(), &[]);
    assert!(matches!(
        receive_delegated_body_fragment_with_line_fore::<2>(
            Some(&policy()),
            f.context(),
            command(&f, &bytes),
            profile(&local, &lines, &bindings)
        ),
        Err(DelegatedFragmentRefusal::Assignment(
            c::AssignedPlanRefusal::WrongSchema
        ))
    ));
}
fn projection_with_schema7() -> Vec<u8> {
    projection(c::ASSIGNED_FORE_NATIVE_PLAN_SCHEMA)
}
