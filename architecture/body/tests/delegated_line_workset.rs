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

#[test]
fn borrowed_workset_inspection_reuses_complete_envelope_checks_without_attribution() {
    let bytes = mixed_workset();
    let view = BodyWorksetDocumentView::<2>::decode(&bytes).unwrap();
    assert_eq!(view.len(), 2);
    assert!(!view.is_empty());
    let first = view.document(0).unwrap();
    assert_eq!(
        decode_body_fragment_document(first).unwrap().0.fragment,
        "fragment"
    );
    assert!(first.as_ptr() >= bytes.as_ptr());
    assert!((first.as_ptr() as usize + first.len()) <= bytes.as_ptr() as usize + bytes.len());
    assert!(view.document(2).is_none());
    assert!(BodyWorksetDocumentView::<1>::decode(&bytes).is_err());
    assert!(BodyWorksetDocumentView::<0>::decode(&bytes).is_err());
    assert!(BodyWorksetDocumentView::<2>::decode(&bytes[..bytes.len() - 1]).is_err());
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(BodyWorksetDocumentView::<2>::decode(&trailing).is_err());
    let mut corrupted = bytes.clone();
    *corrupted.last_mut().unwrap() ^= 1;
    assert!(BodyWorksetDocumentView::<2>::decode(&corrupted).is_err());
}

#[test]
fn whole_workset_state_keeps_high_words_and_rejects_sum_overflow() {
    fn with_states(states: [u32; 2]) -> Vec<u8> {
        let fragments = ["fragment", "fragment/legacy"];
        let mut projections = [
            projection(conduit_core::ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA),
            projection(conduit_core::ASSIGNED_FORE_NATIVE_PLAN_SCHEMA),
        ];
        projections[1][32..48].copy_from_slice(&id(fragments[1]).0);
        let documents: Vec<_> = projections
            .iter_mut()
            .zip(states)
            .zip(fragments)
            .map(|((projection, state), fragment)| {
                projection[12..16].copy_from_slice(&state.to_le_bytes());
                document_for(
                    projection,
                    "body/1",
                    "boot/1",
                    &inventory(),
                    &[],
                    fragment,
                    &fragments,
                )
            })
            .collect();
        encode_body_workset_document(&[&documents[0], &documents[1]], 4096).unwrap()
    }
    let f = Fixture::new();
    let bytes = with_states([70_000, 80_000]);
    let mut policy = policy();
    policy.maximum_runtime_state_bytes = 150_000;
    let set = receive_delegated_body_workset::<2>(
        Some(&policy),
        f.context(),
        workset_command(&f, &bytes),
    )
    .unwrap();
    assert_eq!(set.declared_runtime_state_bytes(), 150_000);
    policy.maximum_runtime_state_bytes -= 1;
    assert!(matches!(
        receive_delegated_body_workset::<2>(
            Some(&policy),
            f.context(),
            workset_command(&f, &bytes)
        ),
        Err(DelegatedWorksetRefusal::Policy(
            PreparationDelegationRefusal::Bounds
        ))
    ));
    let overflow = with_states([u32::MAX, 1]);
    assert!(matches!(
        BodyWorksetDocumentView::<2>::decode(&overflow),
        Err(BodyWorksetDocumentRefusal::Bounds)
    ));
}
