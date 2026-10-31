use super::*;
use crate::observed_custody_client::{wire, VerifiedDurableEpoch};
use sha2::{Digest, Sha256};
#[allow(dead_code, clippy::duplicate_mod)] // Exact shared history wire, also included by Collector.
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_observed_wire.rs"]
mod history_wire;

#[test]
fn custody_signs_only_verified_durable_epoch_with_matching_private_membership() {
    let dir = tests::directory("typed-custody");
    let root = ed25519_dalek::SigningKey::from_bytes(&[41; 32]);
    let bootstrap: Bootstrap = serde_json::from_value(serde_json::json!({
        "invitation":{"schema":conduit_body::INVITATION_SCHEMA,"claim":{"invitation_id":"08".repeat(32),"body_id":"02".repeat(32),"nonce":vec![3;32],"expires_at_millis":999999},"secret":vec![22;32]},
        "root_key":root.verifying_key().to_bytes(),"producer_host":"06".repeat(32),"producer_boot":"07".repeat(32)
    })).unwrap();
    let credential: conduit_body::MembershipCredential = serde_json::from_value(serde_json::json!({
        "credential_id":"01".repeat(32),"body_id":"02".repeat(32),"part_id":"03".repeat(32),"host_id":"04".repeat(32),"boot_id":"05".repeat(32),"issued_at_millis":123
    })).unwrap();
    let mut receipt = joined::JoinedHostReceipt {
        schema: joined::PROFILE.into(),
        credential: credential.clone(),
        membership_revision: 2,
        producer_host: "06".repeat(32).into(),
        producer_boot: "07".repeat(32).into(),
        producer_key: root.verifying_key().to_bytes(),
        invitation_id: bootstrap.invitation.claim.invitation_id.clone(),
        offer_generation: conduit_core::OfferGeneration(1),
        signature: vec![],
    };
    receipt.signature = root
        .sign(&joined::transcript(&receipt.scope()))
        .to_bytes()
        .to_vec();
    let binding = dir.join("binding.json");
    let private = dir.join("continuity.json");
    save(&private, &bootstrap).unwrap();
    save(&binding, &serde_json::json!({"schema":"conduit.environment/joined-recorder@1","invitation":bootstrap.invitation.claim,"root_key":bootstrap.root_key,"local_host":credential.host_id,"local_boot":credential.boot_id,"receipt":receipt})).unwrap();
    let mut scope = [9; 96];
    scope[..32].copy_from_slice(&Sha256::digest(credential.body_id.as_str().as_bytes()));
    let contract =
        conduit_environment_runtime::observation::ObservationContract::prepare().unwrap();
    let forms: [u8; 32] = Sha256::digest(
        conduit_environment_runtime::observed_record::descriptor(&contract).unwrap(),
    )
    .into();
    let record = conduit_environment_runtime::observed_record::ObservedRecord {
        sequence: 1,
        observed_at_boot_ms: 123,
        disposition: conduit_environment_runtime::recording::RecordingDisposition::ProviderFailed,
        quantities: None,
    }
    .encode()
    .unwrap();
    let history = dir.join("history");
    std::fs::create_dir(&history).unwrap();
    let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let batch = serde_json::json!({"profile":"observed-history@1","scope":hex(&scope),"forms":hex(&forms),"after":0,"committed":1,"records":hex(&record),"signature":hex(&root.sign(&history_wire::transcript(&scope,&forms,0,1,&record)).to_bytes())});
    std::fs::write(
        history.join("observed-history-0000-0001.json"),
        serde_json::to_vec(&batch).unwrap(),
    )
    .unwrap();
    let mut offer = wire::Offer {
        schema: wire::OFFER,
        region: wire::Hex([8; 32]),
        scope: wire::Hex(scope),
        forms: wire::Hex(forms),
        committed: 1,
        digest: wire::Hex(Sha256::digest(record).into()),
        signature: wire::Hex([0; 64]),
    };
    offer.signature = wire::Hex(
        root.sign(&wire::offer_transcript(&offer, &bootstrap.root_key))
            .to_bytes(),
    );
    let verified =
        VerifiedDurableEpoch::verify(&offer, &credential.body_id, &bootstrap.root_key, &history)
            .unwrap();
    let bytes = sign_durable_custody(&binding, &private, &verified).unwrap();
    let request: wire::Request<'_> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(request.offer.0, verified.offer_hash().to_owned());
    let signer = ed25519_dalek::SigningKey::from_bytes(&bootstrap.invitation.secret);
    signer
        .verifying_key()
        .verify_strict(
            &request.transcript(),
            &ed25519_dalek::Signature::from_bytes(&request.signature.0),
        )
        .unwrap();
    assert_eq!(
        (request.part, request.host, request.boot),
        (
            credential.part_id.as_str(),
            credential.host_id.as_str(),
            credential.boot_id.as_str()
        )
    );
    // A recovered binding uses the same typed credential without readmission.
    use crate::local_recorder::recovery;
    let mut query = recovery::Request {
        schema: recovery::REQUEST.into(),
        body_id: credential.body_id.clone(),
        host_id: credential.host_id.clone(),
        boot_id: credential.boot_id.clone(),
        nonce: [17; 32],
        signature: vec![],
    };
    query.signature = signer.sign(&query.transcript()).to_bytes().to_vec();
    let mut recovered = recovery::Receipt {
        schema: recovery::RECEIPT.into(),
        credential: credential.clone(),
        membership_revision: 2,
        producer_host: "06".repeat(32).into(),
        producer_boot: "19".repeat(32).into(),
        producer_key: bootstrap.root_key,
        request_nonce: query.nonce,
        signature: vec![],
    };
    recovered.signature = root.sign(&recovered.transcript()).to_bytes().to_vec();
    let recovered_path = dir.join("recovered-binding.json");
    save(&recovered_path, &serde_json::json!({
        "schema":"conduit.environment/joined-recorder@2", "invitation":bootstrap.invitation.claim,
        "root_key":bootstrap.root_key, "local_host":credential.host_id,
        "local_boot":credential.boot_id, "receipt":recovered, "recovery_request":query,
    })).unwrap();
    assert_eq!(
        sign_durable_custody(&recovered_path, &private, &verified).unwrap(),
        bytes
    );
    let mut altered = serde_json::to_value(&bootstrap).unwrap();
    altered["root_key"] = serde_json::json!([7; 32].to_vec());
    let wrong = dir.join("wrong.json");
    save(&wrong, &altered).unwrap();
    assert!(sign_durable_custody(&binding, &wrong, &verified).is_err());
    altered = serde_json::to_value(&bootstrap).unwrap();
    altered["invitation"]["claim"]["nonce"] = serde_json::json!([8; 32].to_vec());
    let wrong = dir.join("wrong-claim.json");
    save(&wrong, &altered).unwrap();
    assert!(sign_durable_custody(&binding, &wrong, &verified).is_err());
    std::fs::write(history.join("observed-history-0000-0001.json"), b"torn").unwrap();
    assert!(VerifiedDurableEpoch::verify(
        &offer,
        &credential.body_id,
        &bootstrap.root_key,
        &history
    )
    .is_err());
    std::fs::remove_dir_all(dir).unwrap();
}
