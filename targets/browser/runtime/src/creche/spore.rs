//! Body-owned preparation and admission for the Tour's physical Pico Host.

use conduit_body::{AdmissionSigns, SpawnAdmissionProof, SpawnInvitationSecret};
use conduit_body_fabrication::{
    seal_prebuilt_body_spore, seal_prebuilt_body_spore_with_content_digest,
    SelectedPrebuiltContent, SporeBinding,
};
use conduit_core::{HostAdvertisement, SignId};
use conduit_host_fabrication::{
    build_host_image, BuildInputs, FabricationCatalog, SporeOutputKind,
};
use serde::{Deserialize, Serialize};

use super::{session, spore_target};
const INVITATION_TTL_MILLIS: u64 = 10 * 60_000;

pub(super) struct PendingSpore {
    spore_id: String,
    image_id: String,
    invitation_id: String,
}

#[derive(Debug, Serialize)]
pub(super) struct PreparedSpore {
    schema: &'static str,
    disposition: &'static str,
    body_id: String,
    spore_id: String,
    image_id: String,
    image_content_digest: String,
    target_id: String,
    output: SporeOutputKind,
    fabrication_package_id: String,
    deployment_adapter: Option<String>,
    invitation_id: String,
    invitation_nonce: [u8; 32],
    invitation_expires_at_millis: u64,
    invitation_secret: Vec<u8>,
    source_identity: String,
    spore_manifest: conduit_body_fabrication::SporeManifest,
    does_not_prove: [&'static str; 6],
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct JoinObservation {
    pub(super) spore_id: String,
    pub(super) image_id: String,
    pub(super) advertisement: HostAdvertisement,
    pub(super) invitation_id: conduit_body::SpawnInvitationId,
    pub(super) body_id: conduit_body::BodyId,
    pub(super) host_id: conduit_core::HostId,
    pub(super) boot_id: conduit_core::BootId,
    pub(super) nonce: [u8; 32],
    pub(super) signature: Vec<u8>,
    pub(super) observed_at_millis: u64,
}

#[derive(Debug, Serialize)]
pub(super) struct AdmissionReceipt {
    schema: &'static str,
    disposition: &'static str,
    body_id: String,
    spore_id: String,
    image_id: String,
    invitation_id: String,
    part_id: String,
    membership_credential_id: String,
    membership_revision: u64,
    host_id: String,
    boot_id: String,
    offer_generation: u64,
    offer_count: usize,
    offers_observed: bool,
    ready: bool,
    plan_id: Option<String>,
    active_play_id: Option<String>,
}

pub(super) fn prepare(entropy: [u8; 32], now_millis: u64) -> Result<PreparedSpore, String> {
    prepare_selected(entropy, now_millis, None)
}

pub(super) fn prepare_selected(
    entropy: [u8; 32],
    now_millis: u64,
    selected_image_content_digest: Option<&str>,
) -> Result<PreparedSpore, String> {
    prepare_selected_for_target(
        entropy,
        now_millis,
        spore_target::PICO_W_TARGET_ID,
        selected_image_content_digest,
    )
}

pub(super) fn prepare_selected_for_target(
    entropy: [u8; 32],
    now_millis: u64,
    target_id: &str,
    selected_image_content_digest: Option<&str>,
) -> Result<PreparedSpore, String> {
    session::with_session(|session| {
        if session.pending_spore.is_some() {
            return Err("this Body already owns one pending physical spore".into());
        }
        let secret = SpawnInvitationSecret::from_csprng_bytes(entropy)
            .map_err(|error| format!("create spore invitation secret: {error:?}"))?;
        let invitation = session
            .admission
            .issue_spawn_invitation(
                secret,
                derive_nonce(&session.receipt.body_id, now_millis),
                now_millis,
                now_millis
                    .checked_add(INVITATION_TTL_MILLIS)
                    .ok_or_else(|| "spore invitation expiry overflow".to_string())?,
            )
            .map_err(|error| format!("issue spore invitation: {error:?}"))?;
        let target = spore_target::prepare(
            &session.receipt.body_id,
            invitation.invitation_id.as_str(),
            target_id,
        )?;
        let catalog = FabricationCatalog::canonical().with_packages(&target.packages);
        let (image, image_bytes) = build_host_image(
            target.configuration.profile().clone(),
            &catalog,
            &target.packages,
            &target.output,
            &BuildInputs {
                source_identity: target.source_identity.into(),
                toolchain_available: true,
            },
        )
        .map_err(|errors| format!("select reviewed prebuilt IMAGE: {errors:?}"))?;
        let spore = match selected_image_content_digest {
            Some(digest) => seal_prebuilt_body_spore_with_content_digest(
                &target.body,
                target.host_name,
                &session.receipt.birth_sign_id,
                &image,
                SelectedPrebuiltContent {
                    image_manifest_bytes: &image_bytes,
                    image_content_digest: digest,
                },
                &catalog,
                &target.packages,
            ),
            None => seal_prebuilt_body_spore(
                &target.body,
                target.host_name,
                &session.receipt.birth_sign_id,
                &image,
                &image_bytes,
                &catalog,
                &target.packages,
            ),
        }
        .map_err(|error| format!("seal Body spore: {error:?}"))?;
        if spore.manifest.binding
            != (SporeBinding::SelfJoining {
                invitation_id: invitation.invitation_id.as_str().into(),
            })
        {
            return Err("sealed spore lost its exact self-joining invitation".into());
        }
        let deployment_adapter = spore.manifest.fabrication.deployment_adapter.clone();
        let prepared = PreparedSpore {
            schema: "conduit.book/prepared-physical-spore@1",
            disposition: "prepared",
            body_id: session.receipt.body_id.clone(),
            spore_id: spore.manifest.spore_id.clone(),
            image_id: spore.manifest.image_id.clone(),
            image_content_digest: spore.manifest.image_content_digest.clone(),
            target_id: spore.manifest.target.clone(),
            output: spore.manifest.output.clone(),
            fabrication_package_id: spore.manifest.fabrication.fabrication_package_id.clone(),
            deployment_adapter,
            invitation_id: invitation.invitation_id.as_str().into(),
            invitation_nonce: invitation.nonce,
            invitation_expires_at_millis: invitation.expires_at_millis,
            invitation_secret: invitation.secret.copy_for_target_provisioning().to_vec(),
            source_identity: spore.manifest.source_identity.clone(),
            spore_manifest: spore.manifest.clone(),
            does_not_prove: [
                "deployment",
                "boot",
                "join",
                "membership",
                "offers",
                "readiness",
            ],
        };
        session.pending_spore = Some(PendingSpore {
            spore_id: prepared.spore_id.clone(),
            image_id: prepared.image_id.clone(),
            invitation_id: prepared.invitation_id.clone(),
        });
        Ok(prepared)
    })
}

pub(super) fn admit(observation: JoinObservation) -> Result<AdmissionReceipt, String> {
    session::with_session(|session| {
        let pending = session
            .pending_spore
            .as_ref()
            .ok_or_else(|| "no prepared physical spore awaits a join request".to_string())?;
        if observation.spore_id != pending.spore_id {
            return Err("join request names the wrong spore".into());
        }
        if observation.image_id != pending.image_id {
            return Err("join request names the wrong IMAGE".into());
        }
        if observation.invitation_id.as_str() != pending.invitation_id {
            return Err("join request names the wrong invitation".into());
        }
        if observation.host_id != observation.advertisement.host_id
            || observation.boot_id != observation.advertisement.boot_id
        {
            return Err("join request does not match the observed fresh Boot".into());
        }
        let signature: [u8; 64] = observation
            .signature
            .try_into()
            .map_err(|_| "join request signature has the wrong bound".to_string())?;
        let proof = SpawnAdmissionProof {
            invitation_id: observation.invitation_id.clone(),
            body_id: observation.body_id,
            host_id: observation.host_id,
            boot_id: observation.boot_id,
            nonce: observation.nonce,
            signature,
        };
        let credential = session
            .admission
            .complete_spawn(
                &mut session.receipt.raw_membership,
                &observation.advertisement,
                &proof,
                observation.observed_at_millis,
                AdmissionSigns {
                    part_admitted: SignId::from(format!("{}/part-admitted", pending.spore_id)),
                    host_attached: SignId::from(format!("{}/host-attached", pending.spore_id)),
                    candidate_admitted: SignId::from(format!("{}/join-consumed", pending.spore_id)),
                },
            )
            .map_err(|error| format!("admit physical spore: {error:?}"))?;
        session.receipt.membership_revision = session.receipt.raw_membership.revision.0;
        let offer_count = observation.advertisement.capabilities.len();
        let receipt = AdmissionReceipt {
            schema: "conduit.book/physical-spore-admission@1",
            disposition: "admitted",
            body_id: session.receipt.body_id.clone(),
            spore_id: pending.spore_id.clone(),
            image_id: pending.image_id.clone(),
            invitation_id: pending.invitation_id.clone(),
            part_id: credential.part_id.as_str().into(),
            membership_credential_id: credential.credential_id.as_str().into(),
            membership_revision: session.receipt.membership_revision,
            host_id: credential.host_id.as_str().into(),
            boot_id: credential.boot_id.as_str().into(),
            offer_generation: observation.advertisement.offer_generation.0,
            offer_count,
            offers_observed: offer_count != 0,
            ready: offer_count != 0,
            plan_id: None,
            active_play_id: None,
        };
        session.pending_spore = None;
        Ok(receipt)
    })
}

fn derive_nonce(body_id: &str, now_millis: u64) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    digest.update(b"conduit.creche/physical-host-spawn-nonce@1");
    digest.update(body_id.as_bytes());
    digest.update(now_millis.to_le_bytes());
    digest.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_interaction::admit_source;
    use conduit_body::{BodyId, SpawnInvitationClaim, SpawnInvitationId};
    use conduit_core::{BootId, HostId};

    const SEED: &str = r#"form hello_across {
    message: text/literal("hello")
    show: presentation/text
    message > show
}"#;

    fn born() {
        session::clear_for_test();
        let interaction = admit_source(SEED.as_bytes(), 71).unwrap();
        session::birth(
            "browser/creche",
            "browser-boot/creche",
            "brisk lantern",
            "morse-network@1",
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
            "morse-network@1",
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
            "morse-network@1",
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
            "morse-network@1",
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
        let prepared =
            prepare_selected_for_target([41; 32], 10_000, TARGET, Some(&digest)).unwrap();
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
        assert!(prepare([14; 32], 3_002).is_ok());
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
}
