//! Original allocating implementation is the independent replay/refusal oracle.
extern crate std;
use super::*;
use alloc::{format, string::String, vec, vec::Vec};
#[allow(
    dead_code,
    reason = "shared real allocator probe supports other admission proofs"
)]
#[path = "validation_tests/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn histories() -> Vec<BodyMembership> {
    let body = crate::Body::born(
        "source/validation".into(),
        "checked/validation".into(),
        1,
        "born".into(),
    )
    .unwrap();
    let mut value = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut states = vec![value.clone()];
    for index in 0..MAX_BODY_PARTS {
        let part = PartId::bind(&body.body_id, &format!("subject/{index}"), index as u64).unwrap();
        let proof = MembershipProofId::bind(&format!("proof/{index}")).unwrap();
        value
            .admit(
                &body.body_id,
                value.revision,
                part.clone(),
                proof.clone(),
                format!("admit/{index}").into(),
            )
            .unwrap();
        states.push(value.clone());
        let observe = |boot: &str, sequence| AuthenticatedHostObservation {
            host_id: format!("host/{index}").into(),
            boot_id: boot.into(),
            offer_generation: OfferGeneration(1),
            proof_id: proof.clone(),
            sequence,
        };
        value
            .observe_present(
                &body.body_id,
                value.revision,
                &part,
                observe("boot/one", 1),
                format!("attach/{index}").into(),
            )
            .unwrap();
        states.push(value.clone());
        value
            .observe_offline(
                &body.body_id,
                value.revision,
                &part,
                &"boot/one".into(),
                format!("detach/{index}").into(),
            )
            .unwrap();
        states.push(value.clone());
        value
            .observe_present(
                &body.body_id,
                value.revision,
                &part,
                observe("boot/two", 2),
                format!("return/{index}").into(),
            )
            .unwrap();
        states.push(value.clone());
    }
    for at in [2, 16, 32] {
        let mut revoked = states[at].clone();
        let part = revoked.parts[0].part_id.clone();
        revoked
            .revoke(
                &body.body_id,
                revoked.revision,
                &part,
                format!("revoked/{at}").into(),
            )
            .unwrap();
        states.push(revoked);
        let mut checkpoint = states[at].clone();
        checkpoint.checkpoint_events().unwrap();
        states.push(checkpoint.clone());
        let part = checkpoint.parts[0].part_id.clone();
        let boot = checkpoint.parts[0]
            .current
            .as_ref()
            .unwrap()
            .boot_id
            .clone();
        checkpoint
            .observe_offline(
                &body.body_id,
                checkpoint.revision,
                &part,
                &boot,
                format!("checkpoint-detach/{at}").into(),
            )
            .unwrap();
        states.push(checkpoint);
    }
    states
}
fn check(value: &BodyMembership) {
    let expected = validation_legacy::validate(value);
    let json = serde_json::to_vec(value).unwrap();
    let postcard = postcard::to_allocvec(value).unwrap();
    let (actual, allocation) = allocation_probe::observe(|| {
        let _refusal = allocation_probe::refuse_all();
        value.validate()
    });
    assert_eq!(actual, expected);
    assert_eq!(
        (
            allocation.allocations,
            allocation.reallocations,
            allocation.deallocations
        ),
        (0, 0, 0)
    );
    assert_eq!(serde_json::to_vec(value).unwrap(), json);
    assert_eq!(postcard::to_allocvec(value).unwrap(), postcard);
}
#[test]
fn bounded_borrowed_replay_matches_legacy_with_all_allocations_refused() {
    let states = histories();
    for value in &states {
        check(value);
    }
    let full = &states[MAX_MEMBERSHIP_EVENTS];
    assert_eq!(
        (full.parts.len(), full.events.len()),
        (MAX_BODY_PARTS, MAX_MEMBERSHIP_EVENTS)
    );
    let (_, legacy) = allocation_probe::observe(|| validation_legacy::validate(full).unwrap());
    std::eprintln!(
        "membership CPU{} events={} parts={} legacy_allocations={} legacy_peak={} borrowed_heap=0 borrowed_rows={}",
        usize::BITS,
        full.events.len(),
        full.parts.len(),
        legacy.allocations,
        legacy.peak_bytes,
        validation::replay_scratch_bytes()
    );
    assert!(legacy.allocations > 0);
    for value in states.iter().step_by(7) {
        let decoded: BodyMembership =
            serde_json::from_slice(&serde_json::to_vec(value).unwrap()).unwrap();
        check(&decoded);
    }
}
#[test]
fn malformed_histories_keep_exact_legacy_refusal_and_first_error_order() {
    for original in histories().into_iter().filter(|v| !v.events.is_empty()) {
        for index in 0..original.events.len() {
            let mut changed = original.clone();
            changed.events[index].revision.0 = 0;
            check(&changed);
            let mut changed = original.clone();
            changed.events[index].body_id = BodyId::bound(String::new());
            check(&changed);
            let mut changed = original.clone();
            changed.events[index].sign_id = "".into();
            check(&changed);
            for kind in [
                MembershipEventKind::Admitted {
                    proof_id: MembershipProofId::bound(String::new()),
                },
                MembershipEventKind::HostDetached {
                    prior_boot_id: "boot/wrong".into(),
                },
                MembershipEventKind::Revoked,
                MembershipEventKind::HostAttached {
                    observation: AuthenticatedHostObservation {
                        host_id: "".into(),
                        boot_id: "boot/other".into(),
                        offer_generation: OfferGeneration(0),
                        proof_id: MembershipProofId::bind("proof/change").unwrap(),
                        sequence: 0,
                    },
                },
            ] {
                let mut changed = original.clone();
                changed.events[index].kind = kind;
                check(&changed);
            }
            if index > 0 {
                let mut changed = original.clone();
                changed.events[index].sign_id = changed.events[0].sign_id.clone();
                changed.events[index].change_id = changed.change_id(
                    &changed.events[index].part_id,
                    &changed.events[index].sign_id,
                    changed.events[index].revision,
                );
                check(&changed);
            }
        }
        let mut changed = original.clone();
        changed.parts.reverse();
        check(&changed);
        let mut changed = original.clone();
        changed.parts[0].state = MembershipState::Revoked;
        check(&changed);
        let mut changed = original.clone();
        changed.revision.0 = u64::MAX;
        check(&changed);
        let mut changed = original.clone();
        changed.fulfilled_sign_id = Some("".into());
        check(&changed);
        let mut changed = original.clone();
        changed.events.push(changed.events[0].clone());
        check(&changed);
    }
    let mut full = histories().remove(MAX_MEMBERSHIP_EVENTS);
    full.parts.push(full.parts[0].clone());
    check(&full);
    let mut checkpoint = histories()
        .into_iter()
        .find(|s| s.checkpoint.is_some())
        .unwrap();
    checkpoint
        .checkpoint
        .as_mut()
        .unwrap()
        .parts
        .push(checkpoint.parts[0].clone());
    check(&checkpoint);
}
#[test]
fn stack_identity_digest_matches_original_length_delimited_unicode_and_u64_bytes() {
    use core::fmt::Write;
    use sha2::{Digest, Sha256};
    for domain in ["", "membership-change", "part", "temperature/°"] {
        for values in [&[][..], &["", "ab", "c"][..], &["a", "bc", "日本語"][..]] {
            for sequence in [0, 1, u64::MAX] {
                let mut hash = Sha256::new();
                for text in core::iter::once(domain).chain(values.iter().copied()) {
                    hash.update((text.len() as u32).to_le_bytes());
                    hash.update(text.as_bytes());
                }
                hash.update(sequence.to_le_bytes());
                let mut expected = String::new();
                for byte in hash.finalize() {
                    write!(&mut expected, "{byte:02x}").unwrap();
                }
                let (actual, allocation) = allocation_probe::observe(|| {
                    let _refusal = allocation_probe::refuse_all();
                    crate::identity::bound_identity_bytes(domain, values, sequence)
                });
                assert_eq!(actual.as_slice(), expected.as_bytes());
                assert_eq!(allocation.allocations, 0);
                assert_eq!(
                    crate::identity::bind_identity(domain, values, sequence),
                    expected
                );
            }
        }
    }
}
