#![cfg(feature = "authenticated-admission")]
use conduit_body::*;
use ed25519_dalek::{Signer, SigningKey};
fn ceilings() -> ContinuationCeilings {
    ContinuationCeilings {
        assignment_bytes: 4096,
        runtime_bytes: 16384,
        durable_bytes: 8192,
        operation_slots: 16,
        active_worksets: 1,
    }
}
fn intent(key: &SigningKey) -> ContinuationIntent<'static> {
    ContinuationIntent {
        schema: CONTINUATION_INTENT_SCHEMA,
        body_id: "body/1",
        part_id: "part/1",
        host_id: "host/1",
        credential_id: "credential/57",
        membership_generation: 57,
        intent_generation: 3,
        issuer_host: "owner/1",
        issuer_key: key.verifying_key().to_bytes(),
        source_digest: [1; 32],
        workset_digest: [2; 32],
        image_id: "image/1",
        configuration_id: "config/1",
        ceilings: ceilings(),
    }
}
fn facts(boot: &str) -> TrustedInstalledContinuationFacts<'_> {
    TrustedInstalledContinuationFacts {
        body_id: "body/1",
        part_id: "part/1",
        host_id: "host/1",
        credential_id: "credential/57",
        membership_generation: 57,
        intent_generation: 3,
        source_digest: [1; 32],
        workset_digest: [2; 32],
        image_id: "image/1",
        configuration_id: "config/1",
        actual_boot: boot,
    }
}
fn policy(key: &SigningKey) -> ContinuationPolicy<'static> {
    ContinuationPolicy {
        issuer_host: "owner/1",
        issuer_key: key.verifying_key().to_bytes(),
        maximum: ceilings(),
    }
}
fn refusal(
    result: Result<VerifiedContinuationIntent<'_>, ContinuationIntentRefusal>,
) -> ContinuationIntentRefusal {
    match result {
        Err(e) => e,
        Ok(_) => panic!("unexpected validation"),
    }
}
#[test]
fn exact_installed_intent_validates_recipe_on_distinct_boots_without_replaying_start() {
    let key = SigningKey::from_bytes(&[7; 32]);
    let intent = intent(&key);
    let signature = key.sign(&intent.signing_transcript()).to_bytes();
    let encoded = serde_json::to_string(&intent).unwrap();
    let decoded: ContinuationIntent<'_> = serde_json::from_str(&encoded).unwrap();
    for boot in ["boot/new-1", "boot/new-2"] {
        let checked =
            verify_continuation_intent(decoded, &signature, Some(policy(&key)), facts(boot))
                .unwrap();
        assert_eq!(checked.actual_boot(), boot);
        assert_eq!(checked.intent().workset_digest, [2; 32]);
    }
    assert_eq!(
        refusal(verify_continuation_intent(
            intent,
            &signature,
            None,
            facts("boot/new")
        )),
        ContinuationIntentRefusal::MissingPolicy
    );
}
#[test]
fn scope_generation_and_every_signed_field_refuse_before_any_consumer_receives_recipe() {
    let key = SigningKey::from_bytes(&[7; 32]);
    let original = intent(&key);
    let signature = key.sign(&original.signing_transcript()).to_bytes();
    macro_rules! change {
        ($field:ident, $value:expr) => {{
            let mut changed = original;
            changed.$field = $value;
            assert!(
                verify_continuation_intent(
                    changed,
                    &signature,
                    Some(policy(&key)),
                    facts("boot/new")
                )
                .is_err(),
                stringify!($field)
            );
        }};
    }
    change!(schema, "wrong");
    change!(body_id, "body/other");
    change!(part_id, "part/other");
    change!(host_id, "host/other");
    change!(credential_id, "credential/old");
    change!(issuer_host, "owner/other");
    change!(issuer_key, [8; 32]);
    change!(source_digest, [3; 32]);
    change!(workset_digest, [4; 32]);
    change!(image_id, "image/other");
    change!(configuration_id, "config/other");
    change!(membership_generation, 56);
    change!(intent_generation, 2);
    // Even a smaller, otherwise admissible ceiling remains signature-bound.
    let mut reduced = original.ceilings;
    reduced.runtime_bytes -= 1;
    change!(ceilings, reduced);
    let mut wrong = signature;
    wrong[0] ^= 1;
    assert_eq!(
        refusal(verify_continuation_intent(
            original,
            &wrong,
            Some(policy(&key)),
            facts("boot/new")
        )),
        ContinuationIntentRefusal::Signature
    );
    let foreign = SigningKey::from_bytes(&[9; 32]);
    assert!(verify_continuation_intent(
        original,
        &signature,
        Some(policy(&foreign)),
        facts("boot/new")
    )
    .is_err());
}
#[test]
fn durable_generations_are_exact_trusted_local_inputs_not_candidate_assertions() {
    let key = SigningKey::from_bytes(&[7; 32]);
    let intent = intent(&key);
    let signature = key.sign(&intent.signing_transcript()).to_bytes();
    for generation in [0, 2, 4] {
        let mut installed = facts("boot/new");
        installed.intent_generation = generation;
        assert_eq!(
            refusal(verify_continuation_intent(
                intent,
                &signature,
                Some(policy(&key)),
                installed
            )),
            ContinuationIntentRefusal::Generation
        );
    }
    let mut installed = facts("boot/new");
    installed.membership_generation = 58;
    assert_eq!(
        refusal(verify_continuation_intent(
            intent,
            &signature,
            Some(policy(&key)),
            installed
        )),
        ContinuationIntentRefusal::Generation
    );
}
#[test]
fn finite_limits_and_malformed_identity_are_rejected_even_when_owner_signed() {
    let key = SigningKey::from_bytes(&[7; 32]);
    for field in 0..5 {
        let mut intent = intent(&key);
        match field {
            0 => intent.ceilings.assignment_bytes = 0,
            1 => intent.ceilings.runtime_bytes = 0,
            2 => intent.ceilings.durable_bytes = 0,
            3 => intent.ceilings.operation_slots = 0,
            _ => intent.ceilings.active_worksets = 0,
        }
        let sig = key.sign(&intent.signing_transcript()).to_bytes();
        assert_eq!(
            refusal(verify_continuation_intent(
                intent,
                &sig,
                Some(policy(&key)),
                facts("boot/new")
            )),
            ContinuationIntentRefusal::Bounds
        );
    }
    let mut intent = intent(&key);
    intent.ceilings.active_worksets = 2;
    let sig = key.sign(&intent.signing_transcript()).to_bytes();
    assert_eq!(
        refusal(verify_continuation_intent(
            intent,
            &sig,
            Some(policy(&key)),
            facts("boot/new")
        )),
        ContinuationIntentRefusal::Bounds
    );
    let oversized = "x".repeat(MAX_LIFECYCLE_ID_BYTES + 1);
    intent.host_id = &oversized;
    let sig = key.sign(&intent.signing_transcript()).to_bytes();
    assert_eq!(
        refusal(verify_continuation_intent(
            intent,
            &sig,
            Some(policy(&key)),
            facts("boot/new")
        )),
        ContinuationIntentRefusal::Bounds
    );
}

#[test]
fn wire_cannot_smuggle_start_claims_and_all_ceiling_overruns_refuse() {
    let key = SigningKey::from_bytes(&[7; 32]);
    let original = intent(&key);
    let mut wire = serde_json::to_value(original).unwrap();
    wire["started"] = serde_json::json!(true);
    let wire = serde_json::to_string(&wire).unwrap();
    assert!(serde_json::from_str::<ContinuationIntent<'_>>(&wire).is_err());
    for field in 0..5 {
        let mut changed = original;
        match field {
            0 => changed.ceilings.assignment_bytes += 1,
            1 => changed.ceilings.runtime_bytes += 1,
            2 => changed.ceilings.durable_bytes += 1,
            3 => changed.ceilings.operation_slots += 1,
            _ => changed.ceilings.active_worksets += 1,
        }
        let signature = key.sign(&changed.signing_transcript()).to_bytes();
        assert_eq!(
            refusal(verify_continuation_intent(
                changed,
                &signature,
                Some(policy(&key)),
                facts("boot/new")
            )),
            ContinuationIntentRefusal::Bounds
        );
    }
}
