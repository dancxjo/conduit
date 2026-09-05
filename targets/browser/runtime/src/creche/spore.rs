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
    browser_configuration_id: Option<String>,
    browser_profile_id: Option<String>,
    browser_configuration_source: Option<String>,
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
    prepare_selected_for_target_with_browser_configuration(
        entropy,
        now_millis,
        target_id,
        selected_image_content_digest,
        None,
    )
}

pub(super) fn prepare_selected_browser(
    entropy: [u8; 32],
    now_millis: u64,
    selected_image_content_digest: Option<&str>,
    selection: super::browser_configuration::BrowserConfigurationSelection,
) -> Result<PreparedSpore, String> {
    prepare_selected_for_target_with_browser_configuration(
        entropy,
        now_millis,
        super::spore_target::BROWSER_PAGE_TARGET_ID,
        selected_image_content_digest,
        Some(selection),
    )
}

fn prepare_selected_for_target_with_browser_configuration(
    entropy: [u8; 32],
    now_millis: u64,
    target_id: &str,
    selected_image_content_digest: Option<&str>,
    browser_selection: Option<super::browser_configuration::BrowserConfigurationSelection>,
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
        let (target, browser_review) = if let Some(selection) = browser_selection {
            let (review, checked, _) = super::browser_configuration::review(selection)?;
            let target = spore_target::prepare_browser(
                &session.receipt.body_id,
                invitation.invitation_id.as_str(),
                checked,
            )?;
            (target, Some(review))
        } else {
            (
                spore_target::prepare(
                    &session.receipt.body_id,
                    invitation.invitation_id.as_str(),
                    target_id,
                )?,
                None,
            )
        };
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
            schema: "conduit.tour/prepared-physical-spore@1",
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
            browser_configuration_id: browser_review
                .as_ref()
                .map(|review| review.configuration_id.clone()),
            browser_profile_id: browser_review
                .as_ref()
                .map(|review| review.profile_id.clone()),
            browser_configuration_source: browser_review.map(|review| review.canonical_source),
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
        let (pending_spore_id, pending_image_id, pending_invitation_id) = {
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
            (
                pending.spore_id.clone(),
                pending.image_id.clone(),
                pending.invitation_id.clone(),
            )
        };
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
        let first_biography_sequence = session
            .biography
            .records
            .last()
            .ok_or_else(|| "Body biography omitted its BIRTH record".to_string())?
            .sequence
            .checked_add(1)
            .ok_or_else(|| "Body biography sequence overflow".to_string())?;
        let second_biography_sequence = first_biography_sequence
            .checked_add(1)
            .ok_or_else(|| "Body biography sequence overflow".to_string())?;
        session
            .biography
            .can_append(2)
            .map_err(|error| format!("admit physical-spore biography records: {error:?}"))?;

        let prior_event_count = session.receipt.raw_membership.events.len();
        let mut next_admission = session.admission.clone();
        let mut next_membership = session.receipt.raw_membership.clone();
        let credential = match next_admission.complete_spawn(
            &mut next_membership,
            &observation.advertisement,
            &proof,
            observation.observed_at_millis,
            AdmissionSigns {
                part_admitted: SignId::from(format!("{pending_spore_id}/part-admitted")),
                host_attached: SignId::from(format!("{pending_spore_id}/host-attached")),
                candidate_admitted: SignId::from(format!("{pending_spore_id}/join-consumed")),
            },
        ) {
            Ok(credential) => credential,
            Err(error) => {
                session.admission = next_admission;
                return Err(format!("admit physical spore: {error:?}"));
            }
        };
        let new_events = next_membership
            .events
            .get(prior_event_count..)
            .ok_or_else(|| "physical-spore admission lost membership events".to_string())?;
        if new_events.len() != 2 {
            return Err("physical-spore admission produced an unexpected event count".into());
        }
        let biography_events = [
            (new_events[0].change_id.clone(), first_biography_sequence),
            (new_events[1].change_id.clone(), second_biography_sequence),
        ];
        let mut next_biography = session.biography.clone();
        next_biography
            .append_membership_events(next_membership.clone(), &biography_events)
            .map_err(|error| format!("record physical-spore membership biography: {error:?}"))?;

        session.admission = next_admission;
        session.receipt.raw_membership = next_membership;
        session.receipt.membership_revision = session.receipt.raw_membership.revision.0;
        session.biography = next_biography;
        let offer_count = observation.advertisement.capabilities.len();
        let receipt = AdmissionReceipt {
            schema: "conduit.tour/physical-spore-admission@1",
            disposition: "admitted",
            body_id: session.receipt.body_id.clone(),
            spore_id: pending_spore_id,
            image_id: pending_image_id,
            invitation_id: pending_invitation_id,
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
#[path = "spore_tests.rs"]
mod tests;
