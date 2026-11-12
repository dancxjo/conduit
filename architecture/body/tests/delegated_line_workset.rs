#![cfg(feature = "authenticated-admission")]
//! One original complete-set attribution/deadline, with explicit schema-8 pins.
use conduit_body::*;
#[allow(dead_code)]
#[path = "delegated_line_fore/support.rs"]
mod support;
use support::*;
#[test]
fn mixed_workset_uses_original_command_and_independent_child_profiles() {
    let f = Fixture::new();
    let bytes = mixed_workset();
    let policy = policy();
    let set = receive_delegated_body_workset::<2>(
        Some(&policy),
        f.context(),
        workset_command(&f, &bytes),
    )
    .unwrap();
    let local = [local()];
    let lines = [line(7)];
    let bindings = [binding()];
    let (fragment, fore) = set
        .receive_fragment_with_line_fore::<2>(
            0,
            Some(&policy),
            f.context(),
            profile(&local, &lines, &bindings),
        )
        .unwrap();
    assert_eq!(fragment.metadata().fragment, "fragment");
    assert_eq!(fore.as_slice(), [remote(), local[0]]);
    let all = [remote(), local[0]];
    let (fragment, fore) = set
        .receive_fragment_with_pinned_fore::<2>(1, Some(&policy), f.context(), legacy_profile(&all))
        .unwrap();
    assert_eq!(fragment.metadata().fragment, "fragment/legacy");
    assert_eq!(fore.as_slice(), all);
    assert!(matches!(
        set.receive_fragment_with_line_fore::<2>(
            2,
            Some(&policy),
            f.context(),
            profile(&local, &lines, &bindings)
        ),
        Err(DelegatedWorksetRefusal::Index)
    ));
    // A schema-8 child cannot enter through either unchanged legacy door.
    assert!(set
        .receive_fragment_with_pinned_fore::<2>(0, Some(&policy), f.context(), legacy_profile(&all))
        .is_err());
    assert!(set
        .receive_fragment_with_approved_fore::<2>(
            0,
            Some(&policy),
            f.context(),
            profile(&[], &[], &[]).preparation
        )
        .is_err());
    // Selecting the schema-8 profile for the schema-7 child cannot fall back.
    assert!(set
        .receive_fragment_with_line_fore::<2>(
            1,
            Some(&policy),
            f.context(),
            profile(&local, &lines, &bindings)
        )
        .is_err());
}
#[test]
fn every_child_rechecks_original_set_deadline_scope_policy_and_exact_line_count() {
    let f = Fixture::new();
    let bytes = mixed_workset();
    let policy = policy();
    let set = receive_delegated_body_workset::<2>(
        Some(&policy),
        f.context(),
        workset_command(&f, &bytes),
    )
    .unwrap();
    let local = [local()];
    let lines = [line(7)];
    let foreign = [line(8)];
    let bindings = [binding()];
    let mut stale = f.context();
    stale.now_millis = 30;
    assert!(matches!(
        set.receive_fragment_with_line_fore::<2>(
            0,
            Some(&policy),
            stale,
            profile(&local, &lines, &bindings)
        ),
        Err(DelegatedWorksetRefusal::Policy(_))
    ));
    let mut other = Fixture::new();
    other.host.boot_id = "boot/new".into();
    other.member.boot_id = other.host.boot_id.clone();
    assert!(matches!(
        set.receive_fragment_with_line_fore::<2>(
            0,
            Some(&policy),
            other.context(),
            profile(&local, &lines, &bindings)
        ),
        Err(DelegatedWorksetRefusal::Policy(_))
    ));
    assert!(set
        .receive_fragment_with_line_fore::<2>(
            0,
            Some(&policy),
            f.context(),
            profile(&local, &foreign, &bindings)
        )
        .is_err());
    assert!(set
        .receive_fragment_with_line_fore::<2>(
            0,
            Some(&policy),
            f.context(),
            profile(&local, &[], &[])
        )
        .is_err());
    assert!(set
        .receive_fragment_with_line_fore::<1>(
            0,
            Some(&policy),
            f.context(),
            profile(&local, &lines, &bindings)
        )
        .is_err());
    let mut wrong_boot = profile(&local, &lines, &bindings);
    wrong_boot.preparation.requirements.boot = id("boot/other");
    assert!(set
        .receive_fragment_with_line_fore::<2>(0, Some(&policy), f.context(), wrong_boot)
        .is_err());
    let narrow = HostPreparationDelegation {
        maximum_runtime_state_bytes: 128,
        ..policy
    };
    assert!(matches!(
        set.receive_fragment_with_line_fore::<2>(
            0,
            Some(&narrow),
            f.context(),
            profile(&local, &lines, &bindings)
        ),
        Err(DelegatedWorksetRefusal::Policy(
            PreparationDelegationRefusal::Bounds
        ))
    ));
}
