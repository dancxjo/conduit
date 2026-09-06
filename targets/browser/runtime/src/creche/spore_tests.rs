use super::*;
use crate::creche::initial_forms;
use crate::source_interaction::admit_source;
use conduit_body::{BodyId, SpawnInvitationClaim, SpawnInvitationId};
use conduit_core::{BootId, HostId};

const SEED: &str = r#"form hello_across {
    message: text/literal("hello")
    show: presentation/text
    message > show
}"#;

fn initial_selection() -> String {
    let inventory = initial_forms::reviewed_inventory(SEED).unwrap();
    let form = &inventory.forms[0];
    serde_json::to_string(&[initial_forms::InitialFormSelection {
        name: form.name.clone(),
        source_document_id: form.source_document_id.clone(),
        checked_form_id: form.checked_form_id.clone(),
    }])
    .unwrap()
}

fn born() {
    session::clear_for_test();
    let interaction = admit_source(SEED.as_bytes(), 71).unwrap();
    session::birth(
        "browser/creche",
        "browser-boot/creche",
        "brisk lantern",
        &initial_selection(),
        SEED,
        71,
        interaction,
    )
    .unwrap();
}

fn typed<T: serde::de::DeserializeOwned>(value: &str) -> T {
    serde_json::from_value(serde_json::Value::String(value.into())).unwrap()
}

#[test]
fn fresh_body_changes_spore_while_reviewed_image_identity_stays_fixed() {
    born();
    let first = prepare([11; 32], 1_000).unwrap();
    session::clear_for_test();
    let interaction = admit_source(SEED.as_bytes(), 72).unwrap();
    session::birth(
        "browser/creche",
        "browser-boot/creche",
        "brisk lantern",
        &initial_selection(),
        SEED,
        72,
        interaction,
    )
    .unwrap();
    let second = prepare([12; 32], 2_000).unwrap();
    assert_ne!(first.body_id, second.body_id);
    assert_ne!(first.invitation_id, second.invitation_id);
    assert_ne!(first.spore_id, second.spore_id);
    assert_eq!(first.image_id, second.image_id);
    assert_eq!(first.image_content_digest, second.image_content_digest);
}

#[test]
fn selected_uf2_content_is_bound_before_spore_creation() {
    born();
    let first_digest = format!("sha256:{}", "1".repeat(64));
    let first = prepare_selected([21; 32], 5_000, Some(&first_digest)).unwrap();
    assert_eq!(first.image_content_digest, first_digest);

    session::clear_for_test();
    let interaction = admit_source(SEED.as_bytes(), 73).unwrap();
    session::birth(
        "browser/creche",
        "browser-boot/creche",
        "brisk lantern",
        &initial_selection(),
        SEED,
        73,
        interaction,
    )
    .unwrap();
    let second_digest = format!("sha256:{}", "2".repeat(64));
    let second = prepare_selected([22; 32], 6_000, Some(&second_digest)).unwrap();
    assert_eq!(first.image_id, second.image_id);
    assert_ne!(first.image_content_digest, second.image_content_digest);
    assert_ne!(first.spore_id, second.spore_id);

    session::clear_for_test();
    let interaction = admit_source(SEED.as_bytes(), 74).unwrap();
    session::birth(
        "browser/creche",
        "browser-boot/creche",
        "brisk lantern",
        &initial_selection(),
        SEED,
        74,
        interaction,
    )
    .unwrap();
    assert!(prepare_selected([23; 32], 7_000, Some("sha256:short"))
        .unwrap_err()
        .contains("ImageContentDigestInvalid"));
}

#[test]
fn exact_esp32_targets_bind_in_c3_s3_wroom_order_without_family_widening() {
    let targets = [
        "esp32/riscv32imc/usb-dcf8355d-esp32-c3",
        "esp32/xtensa-lx7/usb-54e2006398-esp32-s3",
        "esp32/xtensa-lx6/hw-463-esp-wroom-32",
    ];
    for (index, target) in targets.into_iter().enumerate() {
        born();
        let digest = format!("sha256:{}", (index + 3).to_string().repeat(64));
        let prepared = prepare_selected_for_target(
            [31 + index as u8; 32],
            8_000 + index as u64,
            target,
            Some(&digest),
        )
        .unwrap();
        assert_eq!(prepared.target_id, target);
        assert_eq!(prepared.output, SporeOutputKind::Esp32Image);
        assert_eq!(prepared.image_content_digest, digest);
        assert_eq!(prepared.fabrication_package_id, "conduit-host-esp32@1");
        assert!(prepared
            .deployment_adapter
            .as_deref()
            .is_some_and(|adapter| adapter.contains("esp32")));
        session::clear_for_test();
    }

    born();
    let refusal = prepare_selected_for_target(
        [40; 32],
        9_000,
        "esp32/generic/family",
        Some(&format!("sha256:{}", "9".repeat(64))),
    )
    .unwrap_err();
    assert!(refusal.contains("unsupported exact Crèche physical Host target"));
}

#[test]
fn exact_pro_micro_spore_retains_external_carrier_truth() {
    const TARGET: &str = "avr/avr5/sparkfun-pro-micro-atmega32u4-5v-16mhz";
    born();
    let digest = format!("sha256:{}", "a".repeat(64));
    let prepared = prepare_selected_for_target([41; 32], 10_000, TARGET, Some(&digest)).unwrap();
    assert_eq!(prepared.target_id, TARGET);
    assert_eq!(prepared.output, SporeOutputKind::IntelHex);
    assert_eq!(prepared.image_content_digest, digest);
    assert_eq!(
        prepared.fabrication_package_id,
        "conduit-host-avr-promicro@1"
    );
    assert_eq!(prepared.deployment_adapter, None);
}

#[test]
fn exact_join_is_admitted_once_after_boot_and_offer_observation() {
    born();
    let prepared = prepare([13; 32], 3_000).unwrap();
    let mut advertisement = conduit_signal_conformance::pico_local_advertisement();
    advertisement.host_id = HostId::from("pico/tour");
    advertisement.boot_id = BootId::from("pico/tour-boot");
    let claim = SpawnInvitationClaim {
        invitation_id: typed::<SpawnInvitationId>(&prepared.invitation_id),
        body_id: typed::<BodyId>(&prepared.body_id),
        nonce: prepared.invitation_nonce,
        expires_at_millis: prepared.invitation_expires_at_millis,
    };
    let secret: [u8; 32] = prepared.invitation_secret.clone().try_into().unwrap();
    let secret = SpawnInvitationSecret::from_csprng_bytes(secret).unwrap();
    let signature = secret.sign(&claim.signing_transcript(
        &advertisement.host_id,
        &advertisement.boot_id,
        advertisement.offer_generation,
    ));
    let receipt = admit(JoinObservation {
        spore_id: prepared.spore_id.clone(),
        image_id: prepared.image_id.clone(),
        advertisement,
        invitation_id: claim.invitation_id,
        body_id: claim.body_id,
        host_id: HostId::from("pico/tour"),
        boot_id: BootId::from("pico/tour-boot"),
        nonce: claim.nonce,
        signature: signature.to_vec(),
        observed_at_millis: 3_001,
    })
    .unwrap();
    assert_eq!(receipt.disposition, "admitted");
    assert!(receipt.offers_observed);
    assert!(receipt.ready);
    assert_eq!(session::current().unwrap().raw_membership.parts.len(), 1);
    let snapshot = session::durable_snapshot().unwrap();
    assert_eq!(
        snapshot.receipt.raw_membership,
        snapshot.biography.membership
    );
    session::clear_for_test();
    let restored = session::restore_durable(snapshot).unwrap();
    assert_eq!(restored.membership_revision, 2);
    assert!(prepare([14; 32], 3_002).is_ok());
}

#[test]
fn canonical_browser_join_crosses_the_creche_abi_with_bounded_receipts() {
    use crate::creche::abi;

    born();
    let prepared = prepare_selected_for_target(
        [43; 32],
        11_000,
        spore_target::BROWSER_PAGE_TARGET_ID,
        Some(&format!("sha256:{}", "b".repeat(64))),
    )
    .unwrap();
    let advertisement = crate::installed_browser::membership_advertisement(
        HostId::from("browser/creche-abi-join"),
        BootId::from("browser-boot/creche-abi-join"),
    );
    let claim = SpawnInvitationClaim {
        invitation_id: typed::<SpawnInvitationId>(&prepared.invitation_id),
        body_id: typed::<BodyId>(&prepared.body_id),
        nonce: prepared.invitation_nonce,
        expires_at_millis: prepared.invitation_expires_at_millis,
    };
    let secret = SpawnInvitationSecret::from_csprng_bytes(
        prepared.invitation_secret.clone().try_into().unwrap(),
    )
    .unwrap();
    let signature = secret.sign(&claim.signing_transcript(
        &advertisement.host_id,
        &advertisement.boot_id,
        advertisement.offer_generation,
    ));
    let envelope = serde_json::to_vec(&serde_json::json!({
        "spore_id": prepared.spore_id,
        "image_id": prepared.image_id,
        "invitation_id": claim.invitation_id,
        "body_id": claim.body_id,
        "host_id": advertisement.host_id,
        "boot_id": advertisement.boot_id,
        "nonce": claim.nonce,
        "signature": signature.to_vec(),
        "observed_at_millis": 11_001,
        "advertisement": advertisement,
    }))
    .unwrap();
    assert!(envelope.len() > 32 * 1024);
    assert!(envelope.len() <= abi::conduit_creche_input_capacity());
    let before = session::current().unwrap().raw_membership;
    assert_eq!(
        abi::conduit_creche_admit_physical_spore(abi::conduit_creche_input_capacity() + 1),
        abi::ERROR_INPUT
    );
    assert_eq!(session::current().unwrap().raw_membership, before);
    // The ABI owns this thread-local buffer; copy only the admitted input length.
    unsafe {
        std::ptr::copy_nonoverlapping(
            envelope.as_ptr(),
            abi::conduit_creche_input_ptr() as *mut u8,
            envelope.len(),
        );
    }
    assert_eq!(abi::conduit_creche_admit_physical_spore(envelope.len()), 0);
    // Output remains valid until the next ABI call on this thread.
    let output = unsafe {
        std::slice::from_raw_parts(
            abi::conduit_creche_output_ptr() as *const u8,
            abi::conduit_creche_output_len(),
        )
    };
    let receipt: serde_json::Value = serde_json::from_slice(output).unwrap();
    assert_eq!(receipt["disposition"], "admitted");
    assert_eq!(receipt["ready"], true);
    assert_eq!(session::current().unwrap().raw_membership.parts.len(), 1);
    assert_eq!(abi::conduit_creche_current(), 0);
    assert_eq!(abi::conduit_creche_durable_snapshot(), 0);
}

#[test]
fn wrong_image_refuses_without_membership_mutation() {
    born();
    let prepared = prepare([15; 32], 4_000).unwrap();
    let advertisement = conduit_signal_conformance::pico_local_advertisement();
    let before = session::current().unwrap().raw_membership;
    let refusal = admit(JoinObservation {
        spore_id: prepared.spore_id,
        image_id: "image:stale".into(),
        invitation_id: typed::<SpawnInvitationId>(&prepared.invitation_id),
        body_id: typed::<BodyId>(&prepared.body_id),
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        nonce: prepared.invitation_nonce,
        signature: vec![0; 64],
        observed_at_millis: 4_001,
        advertisement,
    })
    .unwrap_err();
    assert!(refusal.contains("wrong IMAGE"));
    assert_eq!(session::current().unwrap().raw_membership, before);
}
