//! Actual Boot signer and exact production receipt verifier, CPU entropy only.
use super::*;
use native_owner_receipt::{Receipt, Refusal, State};
#[test]
fn boot_acknowledgement_authenticates_command_scope_state_and_device_key() {
    let root = admission::responder::NativeAdmission::new([17; 32]).unwrap();
    let foreign = admission::responder::NativeAdmission::new([18; 32]).unwrap();
    let mut request = owner_control::request();
    request.operation = native_owner_control::Operation::Start;
    let command = request.transcript().unwrap();
    let receipt = Receipt {
        command,
        state: State::StartedHeld,
        snapshot: None,
    };
    let (signature, memory) = allocation_probe::observe(|| root.sign_owner_receipt(&receipt));
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    for change in 0..15 {
        let mut altered = owner_control::request();
        altered.operation = native_owner_control::Operation::Start;
        match change {
            0 => altered.host = "foreign/host",
            1 => altered.boot = "stale/boot",
            2 => altered.offer_generation += 1,
            3 => altered.body = "other/body",
            4 => altered.wake = "other/wake",
            5 => altered.body_plan = "other/body-plan",
            6 => altered.body_play = "other/body-play",
            7 => altered.plan = "other/plan",
            8 => altered.fragment = "other/fragment",
            9 => altered.fragment_play = "other/play",
            10 => altered.assignment_seal[0] ^= 1,
            11 => altered.operation = native_owner_control::Operation::Release,
            12 => altered.sequence += 1,
            13 => {
                assert_eq!(
                    receipt
                        .verify(
                            &foreign.verifying_key(),
                            &command,
                            State::StartedHeld,
                            &signature
                        )
                        .err(),
                    Some(Refusal::Signature)
                );
                continue;
            }
            _ => {
                assert_eq!(
                    receipt
                        .verify(&root.verifying_key(), &command, State::Released, &signature)
                        .err(),
                    Some(Refusal::Scope)
                );
                continue;
            }
        }
        let (result, memory) = allocation_probe::observe(|| {
            receipt.verify(
                &root.verifying_key(),
                &altered.transcript().unwrap(),
                State::StartedHeld,
                &signature,
            )
        });
        assert_eq!(result.err(), Some(Refusal::Scope));
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    }
    let mut tampered = signature;
    tampered[3] ^= 1;
    assert_eq!(
        receipt
            .verify(
                &root.verifying_key(),
                &command,
                State::StartedHeld,
                &tampered
            )
            .err(),
        Some(Refusal::Signature)
    );
    assert_eq!(
        receipt
            .verify(&[0; 32], &command, State::StartedHeld, &signature)
            .err(),
        Some(Refusal::Key)
    );
    let forged = Receipt {
        command,
        state: State::Released,
        snapshot: None,
    };
    assert_eq!(
        forged
            .verify(&root.verifying_key(), &command, State::Released, &signature)
            .err(),
        Some(Refusal::Signature)
    );
    let (verified, memory) = allocation_probe::observe(|| {
        receipt.verify(
            &root.verifying_key(),
            &command,
            State::StartedHeld,
            &signature,
        )
    });
    assert_eq!(verified.unwrap().into_state(), State::StartedHeld);
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}

#[test]
fn observed_receipt_authenticates_exact_committed_snapshot_metadata() {
    let root = admission::responder::NativeAdmission::new([19; 32]).unwrap();
    let mut request = owner_control::request();
    request.operation = native_owner_control::Operation::Inspect;
    let command = request.transcript().unwrap();
    let snapshot = native_owner_receipt::Snapshot {
        sequence: 3,
        committed_millis: 100,
        observed_millis: 150,
        channel: 0,
        bytes: 1234,
        sha256: [4; 32],
    };
    let receipt = Receipt {
        command,
        state: State::Observed,
        snapshot: Some(snapshot),
    };
    let signature = root.sign_owner_receipt(&receipt);
    assert_eq!(
        receipt
            .verify(&root.verifying_key(), &command, State::Observed, &signature)
            .unwrap()
            .into_state(),
        State::Observed
    );
    for change in 0..10 {
        let mut altered = snapshot;
        match change {
            0 => altered.sequence += 1,
            1 => altered.channel += 1,
            2 => altered.bytes -= 1,
            3 => altered.sha256[0] ^= 1,
            4 => altered.sequence = 0,
            5 => altered.channel = 4,
            6 => altered.bytes = 4097,
            7 => altered.committed_millis += 1,
            8 => altered.observed_millis += 1,
            _ => altered.committed_millis = 151,
        }
        let forged = Receipt {
            command,
            state: State::Observed,
            snapshot: Some(altered),
        };
        assert!(forged
            .verify(&root.verifying_key(), &command, State::Observed, &signature)
            .is_err());
    }
}

#[test]
fn retired_receipt_cannot_be_substituted_by_quiescence_or_another_command() {
    let root = admission::responder::NativeAdmission::new([17; 32]).unwrap();
    let mut request = owner_control::request();
    request.operation = native_owner_control::Operation::Retire;
    let command = request.transcript().unwrap();
    let receipt = Receipt {
        command,
        state: State::Retired,
        snapshot: None,
    };
    let signature = root.sign_owner_receipt(&receipt);
    assert!(receipt
        .verify(&root.verifying_key(), &command, State::Retired, &signature)
        .is_ok());
    assert_eq!(
        receipt
            .verify(&root.verifying_key(), &command, State::Quiesced, &signature)
            .err(),
        Some(Refusal::Scope)
    );
    request.operation = native_owner_control::Operation::Cancel;
    assert_eq!(
        receipt
            .verify(
                &root.verifying_key(),
                &request.transcript().unwrap(),
                State::Retired,
                &signature
            )
            .err(),
        Some(Refusal::Scope)
    );
}

#[test]
fn signed_publication_age_is_conservative_and_never_wraps() {
    let mut snapshot = native_owner_receipt::Snapshot {
        sequence: 1,
        committed_millis: 100,
        observed_millis: 150,
        channel: 0,
        bytes: 1,
        sha256: [1; 32],
    };
    assert!(snapshot.fresh_within(60, 10));
    assert!(!snapshot.fresh_within(59, 10));
    assert!(!snapshot.fresh_within(u64::MAX, u64::MAX));
    snapshot.committed_millis = 151;
    assert!(!snapshot.fresh_within(u64::MAX, 0));
}
