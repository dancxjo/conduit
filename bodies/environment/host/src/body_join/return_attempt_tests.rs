use super::*;
fn directory(name: &str) -> std::path::PathBuf {
    super::super::super::tests::directory(name)
}
fn fixture() -> (
    Bootstrap,
    MembershipCredential,
    HostAdvertisement,
    Challenge,
) {
    super::super::tests::fixture()
}
fn renewed(old: &Challenge, next_boot: bool) -> Challenge {
    let mut next: Challenge = serde_json::from_slice(&serde_json::to_vec(old).unwrap()).unwrap();
    next.challenge.admission_id =
        serde_json::from_value(serde_json::json!("06".repeat(32))).unwrap();
    next.challenge.nonce = [10; 32];
    next.challenge.issued_at_millis = if next_boot {
        1
    } else {
        old.challenge.expires_at_millis + 1
    };
    next.challenge.expires_at_millis = next.challenge.issued_at_millis + 30_000;
    if next_boot {
        next.producer_boot = "boot/restarted-root".into();
    }
    sign(&mut next);
    next
}
fn sign(challenge: &mut Challenge) {
    challenge.signature = ed25519_dalek::SigningKey::from_bytes(&[31; 32])
        .sign(&challenge.transcript())
        .to_bytes()
        .to_vec();
}
fn receipt(
    bootstrap: &Bootstrap,
    old: &MembershipCredential,
    ad: &HostAdvertisement,
    challenge: &Challenge,
) -> Vec<u8> {
    let mut credential = old.clone();
    credential.credential_id = serde_json::from_value(serde_json::json!("07".repeat(32))).unwrap();
    credential.boot_id = ad.boot_id.clone();
    credential.issued_at_millis = challenge.challenge.issued_at_millis + 1;
    let mut receipt = recovery::Receipt {
        schema: recovery::RECEIPT.into(),
        credential,
        membership_revision: 3,
        producer_host: bootstrap.producer_host.clone(),
        producer_boot: challenge.producer_boot.clone(),
        producer_key: bootstrap.root_key,
        request_nonce: challenge.challenge.nonce,
        signature: vec![],
    };
    receipt.signature = ed25519_dalek::SigningKey::from_bytes(&[31; 32])
        .sign(&receipt.transcript())
        .to_bytes()
        .to_vec();
    serde_json::to_vec(&receipt).unwrap()
}
fn interrupted(
    state: &Path,
    bootstrap: &Bootstrap,
    old: &MembershipCredential,
    ad: &HostAdvertisement,
    challenge: &Challenge,
) {
    assert!(execute(bootstrap, old, ad, state, |request| {
        if request["schema"] == "conduit.environment/observer-return-challenge-request@1" {
            Ok(serde_json::to_vec(challenge)?)
        } else {
            Err("interrupted before committed return".into())
        }
    })
    .is_err());
}
#[test]
fn expired_signed_challenge_reconciles_without_overwriting_old_proof_or_boot() {
    let state = directory("expired-return-round");
    let (bootstrap, old, ad, first) = fixture();
    interrupted(&state, &bootstrap, &old, &ad, &first);
    let prior_challenge = std::fs::read(round_path(&state, 0, false)).unwrap();
    let prior_proof = std::fs::read(round_path(&state, 0, true)).unwrap();
    let next = renewed(&first, false);
    let mut calls = 0;
    execute(&bootstrap, &old, &ad, &state, |request| {
        calls += 1;
        match request["schema"].as_str().unwrap() {
            recovery::REQUEST => Err("not committed".into()),
            "conduit.environment/observer-return-challenge-request@1" => {
                assert_eq!(request["host_advertisement"], serde_json::to_value(&ad)?);
                Ok(serde_json::to_vec(&next)?)
            }
            "conduit.environment/observer-return-request@1" => {
                assert_eq!(request["nonce"], hex(&next.challenge.nonce));
                assert_eq!(request["host_advertisement"], serde_json::to_value(&ad)?);
                Ok(receipt(&bootstrap, &old, &ad, &next))
            }
            other => panic!("unexpected request {other}"),
        }
    })
    .unwrap();
    assert_eq!(calls, 3);
    assert_eq!(
        std::fs::read(round_path(&state, 0, false)).unwrap(),
        prior_challenge
    );
    assert_eq!(
        std::fs::read(round_path(&state, 0, true)).unwrap(),
        prior_proof
    );
    assert!(round_path(&state, 1, true).exists());
    let binding =
        crate::local_recorder::validated_binding(&state.join("recorder-binding.json")).unwrap();
    assert_eq!(binding.credential().boot_id, ad.boot_id);
    assert_eq!(binding.credential().part_id, old.part_id);
    // Reopening the completed directory performs no new network/admission work.
    execute(&bootstrap, &old, &ad, &state, |_| {
        panic!("unexpected replay")
    })
    .unwrap();
    std::fs::remove_dir_all(state).unwrap();
}
#[test]
fn committed_lost_reply_is_recovered_before_requesting_a_replacement() {
    let state = directory("committed-return-round");
    let (bootstrap, old, ad, first) = fixture();
    interrupted(&state, &bootstrap, &old, &ad, &first);
    let mut calls = 0;
    execute(&bootstrap, &old, &ad, &state, |request| {
        calls += 1;
        assert_eq!(request["schema"], recovery::REQUEST);
        Ok(receipt(&bootstrap, &old, &ad, &first))
    })
    .unwrap();
    assert_eq!(calls, 1);
    assert!(!round_path(&state, 1, false).exists());
    std::fs::remove_dir_all(state).unwrap();
}
#[test]
fn changed_nonce_requires_signed_expiry_or_changed_authority_boot() {
    let (bootstrap, old, ad, first) = fixture();
    let mut next = renewed(&first, false);
    next.challenge.issued_at_millis = first.challenge.expires_at_millis;
    sign(&mut next);
    next.verify(&bootstrap, &old, &ad).unwrap();
    assert!(replacement(&first, &next).is_err());
    let restarted = renewed(&first, true);
    restarted.verify(&bootstrap, &old, &ad).unwrap();
    replacement(&first, &restarted).unwrap();
    let mut changed = renewed(&first, false);
    changed.signature[0] ^= 1;
    assert!(changed.verify(&bootstrap, &old, &ad).is_err());
}
#[test]
fn challenge_round_capacity_refuses_before_another_root_begin() {
    let state = directory("bounded-return-rounds");
    let (bootstrap, old, ad, mut challenge) = fixture();
    for round in 0..MAX_ROUNDS {
        challenge.challenge.admission_id =
            serde_json::from_value(serde_json::json!(format!("{:064x}", round + 1))).unwrap();
        challenge.challenge.nonce = [round as u8 + 1; 32];
        challenge.challenge.issued_at_millis = 1 + round as u64 * 40_000;
        challenge.challenge.expires_at_millis = challenge.challenge.issued_at_millis + 30_000;
        sign(&mut challenge);
        save(&round_path(&state, round, false), &challenge).unwrap();
    }
    let error = execute(&bootstrap, &old, &ad, &state, |request| {
        assert_eq!(request["schema"], recovery::REQUEST);
        Err("no committed receipt".into())
    })
    .unwrap_err();
    assert!(error.to_string().contains("round capacity"));
    assert_eq!(std::fs::read_dir(&state).unwrap().count(), MAX_ROUNDS);
    std::fs::remove_dir_all(state).unwrap();
}
