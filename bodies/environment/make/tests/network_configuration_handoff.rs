//! CPU custody across real private profiles, acknowledged mailbox and policy.
//! No SDK/radio operation, native target fit or Body admission is claimed.
extern crate alloc;
#[allow(dead_code)]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_known_credentials.rs"]
mod known;
#[allow(dead_code)]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_network_recovery.rs"]
mod recovery;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_credential_selection.rs"]
mod selection;
use conduit_environment_http::network_setup::{
    NetworkSetupInput, NetworkSetupLimits, NetworkSetupMailbox, NetworkSetupRefusal,
};
use conduit_net::{KnownSecurity, PrivateNetworkInput};
fn input(name: &[u8], password: &[u8]) -> PrivateNetworkInput {
    PrivateNetworkInput::new(name, password, KnownSecurity::Wpa2Personal, 0).unwrap()
}
#[test]
fn superseded_unused_snapshot_preserves_exact_ack_until_selected_transfer() {
    let mut owner = known::Owner::empty(3);
    let old = owner
        .upsert_after_acknowledgement(input(b"old", b"public-old-pass"), |_| Ok(1))
        .unwrap();
    let old_snapshot = owner.freeze(old).unwrap();
    let mut origin = None;
    let mut mailbox = NetworkSetupMailbox::new(2);
    let token = mailbox
        .submit(
            NetworkSetupInput::from_form(
                b"ssid=new&password=public-new-pass",
                NetworkSetupLimits {
                    minimum_credential_bytes: 8,
                    maximum_credential_bytes: 63,
                },
            )
            .unwrap(),
        )
        .unwrap();
    // Unacknowledged input cannot retire a frozen snapshot.
    selection::retire_superseded(
        &mut owner,
        &mut origin,
        mailbox.acknowledged_submission().ok(),
    )
    .unwrap();
    assert_eq!(owner.selection(), Some(old_snapshot));
    mailbox.acknowledge(token).unwrap();
    selection::retire_superseded(&mut owner, &mut origin, Some(token)).unwrap();
    assert!(owner.selection().is_none());
    assert_eq!(owner.accepted_updates(), 1);
    assert_eq!(owner.store_revision(), 1);
    assert!(mailbox.acknowledged_submission() == Ok(token));
    // A durable commit refusal retains the original profile/counters and ACK.
    assert!(mailbox
        .prepare_acknowledged(|request| owner
            .upsert_after_acknowledgement(input(request.ssid, request.credential), |_| Err(())))
        .unwrap()
        .is_err());
    assert_eq!(owner.accepted_updates(), 1);
    assert!(mailbox.acknowledged_submission() == Ok(token));
    let key = mailbox
        .prepare_acknowledged(|request| {
            owner.upsert_after_acknowledgement(input(request.ssid, request.credential), |_| Ok(2))
        })
        .unwrap()
        .unwrap();
    let selected = owner.freeze(key).unwrap();
    origin = Some(token);
    // Retrying the SAME acknowledged selection cannot erase/recount it.
    selection::retire_superseded(
        &mut owner,
        &mut origin,
        mailbox.acknowledged_submission().ok(),
    )
    .unwrap();
    assert_eq!(owner.selection(), Some(selected));
    assert_eq!(owner.accepted_updates(), 2);
    assert_eq!(owner.store_revision(), 2);
    assert!(mailbox.acknowledged_submission() == Ok(token));
    let mut expected = [0; 167];
    let mut actual = [0; 167];
    let selection = owner.take_selected(&mut expected).unwrap();
    let length = mailbox.take_acknowledged(&mut actual).unwrap();
    assert_eq!(length, selection.wire_bytes);
    assert_eq!(actual[..length], expected[..length]);
    assert!(mailbox.acknowledged_submission() == Err(NetworkSetupRefusal::NoSubmission));
    assert!(owner.selection().is_none());
}

#[test]
fn stale_submission_cannot_retire_a_different_acknowledged_input() {
    let limits = NetworkSetupLimits {
        minimum_credential_bytes: 8,
        maximum_credential_bytes: 63,
    };
    let mut mailbox = NetworkSetupMailbox::new(2);
    let old_token = mailbox
        .submit(NetworkSetupInput::from_form(b"ssid=old&password=public-old-pass", limits).unwrap())
        .unwrap();
    mailbox.acknowledge(old_token).unwrap();
    let mut owner = known::Owner::empty(2);
    let key = owner
        .upsert_after_acknowledgement(input(b"old", b"public-old-pass"), |_| Ok(1))
        .unwrap();
    let frozen = owner.freeze(key).unwrap();
    let mut origin = Some(old_token);
    selection::retire_superseded(&mut owner, &mut origin, Some(old_token)).unwrap();
    assert_eq!(owner.selection(), Some(frozen));
    mailbox.cancel(old_token).unwrap();
    let new_token = mailbox
        .submit(NetworkSetupInput::from_form(b"ssid=new&password=public-new-pass", limits).unwrap())
        .unwrap();
    mailbox.acknowledge(new_token).unwrap();
    assert_eq!(
        mailbox.cancel(old_token),
        Err(NetworkSetupRefusal::StaleSubmission)
    );
    assert!(mailbox.acknowledged_submission() == Ok(new_token));
    selection::retire_superseded(
        &mut owner,
        &mut origin,
        mailbox.acknowledged_submission().ok(),
    )
    .unwrap();
    assert!(owner.selection().is_none());
    assert!(origin.is_none());
    assert_eq!(owner.accepted_updates(), 1);
    assert!(mailbox.acknowledged_submission() == Ok(new_token));
}
