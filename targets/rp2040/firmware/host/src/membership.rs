//! Actual current-Boot receipt installation. This does not create a Plan or Play.
#[path = "membership_preparation.rs"]
mod preparation;
#[path = "membership_return.rs"]
mod returning;
use crate::{
    seed::Seed,
    transport::{Link, LIMIT},
};
use alloc::{format, string::String, vec::Vec};
use conduit_body::{
    ambient_admission_transcript, validate_attested_admission_receipt_basis,
    BorrowedAdmissionReceiptAttestation, PicoAdmissionAdvertisement, PicoSpawnJoinRequest,
    PortableAdmissionReceipt, PICO_ADMISSION_PROTOCOL, PICO_SPAWN_PROTOCOL, PICO_SPAWN_REQUEST,
};
use conduit_core::{BootId, HostAdvertisement, HostId, HostProfileId, OfferGeneration};
use core::{cell::Cell, fmt::Write};
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
static RECEIPT_DIGEST: critical_section::Mutex<Cell<Option<[u8; 32]>>> =
    critical_section::Mutex::new(Cell::new(None));
fn retain_receipt_digest(raw: &[u8]) {
    critical_section::with(|cs| {
        RECEIPT_DIGEST
            .borrow(cs)
            .set(Some(Sha256::digest(raw).into()))
    });
}
pub fn is_member() -> bool {
    critical_section::with(|cs| RECEIPT_DIGEST.borrow(cs).get().is_some())
}

pub fn append_status<const N: usize>(out: &mut heapless::String<N>) -> Result<(), ()> {
    if let Some(digest) = critical_section::with(|cs| RECEIPT_DIGEST.borrow(cs).get()) {
        out.push_str(",\"membership_receipt_sha256\":\"")
            .map_err(|_| ())?;
        for byte in digest {
            write!(out, "{byte:02x}").map_err(|_| ())?;
        }
        out.push('"').map_err(|_| ())?;
    }
    Ok(())
}

pub struct Membership {
    seed: Seed<'static>,
    // This existing Core owner retains the actual local pool and reservations.
    // The compact advertisement contains no decoded capability catalog.
    resources: conduit_core::ResourceAdmissionOwner,
    key: SigningKey,
    freshness: u64,
    receipt: Option<conduit_body::MembershipCredential>,
}
impl Membership {
    pub fn new(seed: Seed<'static>, host_id: String, boot_nonce: [u64; 2]) -> Self {
        let key = SigningKey::from_bytes(seed.admission.secret());
        let ad = HostAdvertisement {
            declared_name: None,
            protocol_version: 1,
            host_id: HostId::from(host_id),
            boot_id: BootId::from(format!(
                "rp2040/boot/{:016x}{:016x}",
                boot_nonce[0], boot_nonce[1]
            )),
            offer_generation: OfferGeneration(1),
            profile: HostProfileId::from(crate::config::CONFIGURATION_ID),
            bases: Vec::new(),
            resources: alloc::vec![conduit_core::resource_offer(
                crate::config::RESIDENT_WORK_POOL,
                conduit_core::COOPERATIVE_STEP_WORK_CLASS_ID,
                crate::config::RESIDENT_WORK_UNITS,
            )],
            capabilities: Vec::new(),
            planner_capabilities: Vec::new(),
        };
        Self {
            seed,
            resources: conduit_core::ResourceAdmissionOwner::new(ad),
            key,
            freshness: 0,
            receipt: None,
        }
    }
    pub async fn serve_once(&mut self, link: &mut Link) -> Result<(), ()> {
        let mut input = [0; LIMIT];
        let request = link.receive(&mut input).await?;
        if request == conduit_body::HOST_PREPARATION_REQUEST {
            return self.serve_preparation(link, &mut input).await;
        }
        if request == conduit_body::PICO_MEMBER_RETURN_IMAGES_REQUEST {
            return self.serve_return(link, &mut input).await;
        }
        if request != PICO_SPAWN_REQUEST {
            return Err(());
        }
        // A retained credential must return through canonical member return;
        // never silently admit the same persistent subject as a new Part.
        if self.seed.admission.credential().is_some() || self.receipt.is_some() {
            return Err(());
        }
        self.freshness = self.freshness.checked_add(1).ok_or(())?;
        let mut output = [0; LIMIT];
        let arrival = PicoAdmissionAdvertisement {
            protocol: PICO_ADMISSION_PROTOCOL,
            advertisement: self.resources.host().clone(),
            friendly_label: "RP2040 Host".into(),
            verifying_key: self.key.verifying_key().to_bytes().to_vec(),
            freshness_sequence: self.freshness,
        };
        let n = serde_json_core::to_slice(&arrival, &mut output).map_err(|_| ())?;
        link.send(&output[..n]).await?;
        let provision = self.seed.admission.spawn().ok_or(())?;
        let transcript = ambient_admission_transcript(
            "spawn-invitation-v1",
            provision.body_id,
            provision.invitation_id,
            self.resources.host().host_id.as_str(),
            self.resources.host().boot_id.as_str(),
            self.resources.host().offer_generation,
            &provision.nonce,
            provision.expires_at_millis,
        );
        let signature = self.key.sign(&transcript).to_bytes();
        let proof = PicoSpawnJoinRequest {
            protocol: PICO_SPAWN_PROTOCOL,
            spore_id: provision.spore_id,
            image_id: provision.image_id,
            invitation_id: provision.invitation_id,
            body_id: provision.body_id,
            host_id: self.resources.host().host_id.as_str(),
            boot_id: self.resources.host().boot_id.as_str(),
            offer_generation: self.resources.host().offer_generation.0,
            nonce: provision.nonce,
            signature: &signature,
        };
        let n = serde_json_core::to_slice(&proof, &mut output).map_err(|_| ())?;
        link.send(&output[..n]).await?;
        let metadata_bytes = link.receive(&mut input).await?;
        let metadata_length = metadata_bytes.len();
        let (metadata, used) =
            serde_json_core::from_slice::<BorrowedAdmissionReceiptAttestation<'_>>(metadata_bytes)
                .map_err(|_| ())?;
        // Keep immutable metadata borrowing input while the raw receipt occupies output.
        if used != metadata_length {
            return Err(());
        }
        let raw_receipt = link.receive(&mut output).await?;
        metadata
            .verify(
                raw_receipt,
                &self.seed.owner,
                &self.key.verifying_key().to_bytes(),
            )
            .map_err(|_| ())?;
        let (receipt, used) =
            serde_json_core::from_slice::<PortableAdmissionReceipt>(raw_receipt).map_err(|_| ())?;
        if used != raw_receipt.len() {
            return Err(());
        }
        validate_attested_admission_receipt_basis(
            &receipt,
            &self.seed.body_id,
            self.resources.host(),
        )
        .map_err(|_| ())?;
        self.receipt = Some(receipt.credential);
        retain_receipt_digest(raw_receipt);
        input.fill(0);
        output.fill(0);
        Ok(())
    }
}
