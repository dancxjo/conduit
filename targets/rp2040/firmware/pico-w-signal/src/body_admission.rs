//! Inert, bounded Body admission responder for an already-running Pico image.

use core::fmt::{self, Write};

use conduit_body::{
    ambient_admission_transcript, validate_pico_challenge, validate_pico_spawn_provision,
    PicoAdmissionChallenge, PicoAdmissionProof, PicoSpawnJoinRequest, PicoSpawnProvision,
    SpawnInvitationSecret, MAX_PICO_ADMISSION_FRAME_BYTES,
    PICO_ADMISSION_PROTOCOL, PICO_ADMISSION_REQUEST, PICO_SPAWN_PROTOCOL,
};
use conduit_core::OfferGeneration;
use ed25519_dalek::{Signer, SigningKey};
use embassy_rp::clocks::RoscRng;

use crate::receipts::RuntimeTranscriptIdentity;
use crate::usb_link::{UsbLinkError, UsbLinkSession};

// The build script derives this from the same exact advertisement embedded below.
const HOST_ID: &str = env!("CONDUIT_PICO_BODY_HOST_ID");
const OFFER_GENERATION: u64 = 1;
const BOOT_PLACEHOLDER: &str =
    "conduit-pico-w-signal/runtime-boot:0000000000000000:00000000000000000000000000000000";
const ADVERTISEMENT_JSON: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/pico_body_advertisement.json"));

pub(crate) struct PicoBodyAdmission {
    boot_id: heapless::String<128>,
    signing_key: SigningKey,
    freshness_sequence: u64,
}

impl PicoBodyAdmission {
    pub(crate) fn new(runtime: &RuntimeTranscriptIdentity) -> Self {
        let mut seed = [0u8; 32];
        let mut rng = RoscRng;
        for chunk in seed.as_chunks_mut::<8>().0 {
            chunk.copy_from_slice(&rng.next_u64().to_le_bytes());
        }
        if seed == [0; 32] {
            seed[0] = 1;
        }
        let mut boot_id = heapless::String::new();
        boot_id
            .push_str(runtime.boot_id())
            .expect("runtime Boot fits reviewed identity bound");
        Self {
            boot_id,
            signing_key: SigningKey::from_bytes(&seed),
            freshness_sequence: 0,
        }
    }

    #[cfg(feature = "pico-local")]
    pub(crate) async fn serve_once(
        &mut self,
        line: &mut UsbLinkSession,
    ) -> Result<(), UsbLinkError> {
        let mut input = [0u8; 1024];
        line.wait_connection().await;
        let request = line.receive_raw_stream_frame(&mut input).await?;
        if crate::bootsel::handle_request(line, request).await? {
            return Ok(());
        }
        if self.serve_spawn_request(line, request).await? {
            return Ok(());
        }
        self.serve_request(line, request).await?;
        Ok(())
    }

    pub(crate) async fn serve_request(
        &mut self,
        line: &mut UsbLinkSession,
        request: &[u8],
    ) -> Result<bool, UsbLinkError> {
        if request != PICO_ADMISSION_REQUEST {
            return Ok(false);
        }

        self.freshness_sequence = self.freshness_sequence.saturating_add(1);
        let mut input = [0u8; 1024];
        let mut output = [0u8; MAX_PICO_ADMISSION_FRAME_BYTES];
        let advertisement_length = self.write_advertisement(&mut output)?;
        line.send_raw_stream_frame(&output[..advertisement_length])
            .await?;

        let challenge_bytes = line.receive_raw_stream_frame(&mut input).await?;
        let (challenge, _) =
            serde_json_core::from_slice::<PicoAdmissionChallenge<'_>>(challenge_bytes)
                .map_err(|_| UsbLinkError::InvalidGeneratedEndpoint)?;
        if !validate_pico_challenge(&challenge, HOST_ID, self.boot_id.as_str(), OFFER_GENERATION) {
            return Err(UsbLinkError::InvalidGeneratedEndpoint);
        }
        let transcript = ambient_admission_transcript(
            "ambient-admission-v1",
            challenge.body_id,
            challenge.admission_id,
            challenge.host_id,
            challenge.boot_id,
            OfferGeneration(challenge.offer_generation),
            &challenge.nonce,
            challenge.expires_at_millis,
        );
        let signature = self.signing_key.sign(&transcript).to_bytes();
        let proof = PicoAdmissionProof {
            protocol: PICO_ADMISSION_PROTOCOL,
            admission_id: challenge.admission_id,
            body_id: challenge.body_id,
            host_id: challenge.host_id,
            boot_id: challenge.boot_id,
            nonce: challenge.nonce,
            signature: &signature,
        };
        let proof_length = serde_json_core::to_slice(&proof, &mut output)
            .map_err(|_| UsbLinkError::BufferOverflow)?;
        line.send_raw_stream_frame(&output[..proof_length]).await?;
        Ok(true)
    }

    pub(crate) async fn serve_spawn_request(
        &mut self,
        line: &mut UsbLinkSession,
        request: &[u8],
    ) -> Result<bool, UsbLinkError> {
        let (provision, _) = match serde_json_core::from_slice::<PicoSpawnProvision<'_>>(request) {
            Ok(value) => value,
            Err(_) => return Ok(false),
        };
        if !validate_pico_spawn_provision(&provision) {
            return Err(UsbLinkError::InvalidGeneratedEndpoint);
        }
        let mut secret_bytes = [0u8; 32];
        secret_bytes.copy_from_slice(provision.secret);
        let secret = SpawnInvitationSecret::from_csprng_bytes(secret_bytes)
            .map_err(|_| UsbLinkError::InvalidGeneratedEndpoint)?;
        secret_bytes.fill(0);
        let transcript = ambient_admission_transcript(
            "spawn-invitation-v1",
            provision.body_id,
            provision.invitation_id,
            HOST_ID,
            self.boot_id.as_str(),
            OfferGeneration(OFFER_GENERATION),
            &provision.nonce,
            provision.expires_at_millis,
        );
        let signature = secret.sign(&transcript);

        self.freshness_sequence = self.freshness_sequence.saturating_add(1);
        let mut output = [0u8; MAX_PICO_ADMISSION_FRAME_BYTES];
        let advertisement_length = self.write_advertisement(&mut output)?;
        line.send_raw_stream_frame(&output[..advertisement_length]).await?;
        let join = PicoSpawnJoinRequest {
            protocol: PICO_SPAWN_PROTOCOL,
            spore_id: provision.spore_id,
            image_id: provision.image_id,
            invitation_id: provision.invitation_id,
            body_id: provision.body_id,
            host_id: HOST_ID,
            boot_id: self.boot_id.as_str(),
            offer_generation: OFFER_GENERATION,
            nonce: provision.nonce,
            signature: &signature,
        };
        let join_length = serde_json_core::to_slice(&join, &mut output)
            .map_err(|_| UsbLinkError::BufferOverflow)?;
        line.send_raw_stream_frame(&output[..join_length]).await?;
        Ok(true)
    }

    fn write_advertisement(&self, output: &mut [u8]) -> Result<usize, UsbLinkError> {
        let template = core::str::from_utf8(ADVERTISEMENT_JSON)
            .map_err(|_| UsbLinkError::InvalidGeneratedEndpoint)?;
        let placeholder = template
            .find(BOOT_PLACEHOLDER)
            .ok_or(UsbLinkError::InvalidGeneratedEndpoint)?;
        if self.boot_id.len() != BOOT_PLACEHOLDER.len() {
            return Err(UsbLinkError::InvalidGeneratedEndpoint);
        }
        let key = self.signing_key.verifying_key().to_bytes();
        let mut writer = SliceWriter::new(output);
        writer.write_str("{\"protocol\":1,\"advertisement\":")?;
        writer.write_str(&template[..placeholder])?;
        writer.write_str(self.boot_id.as_str())?;
        writer.write_str(&template[placeholder + BOOT_PLACEHOLDER.len()..])?;
        writer.write_str(",\"friendly_label\":\"Pico W · USB\",\"verifying_key\":[")?;
        for (index, byte) in key.iter().enumerate() {
            if index != 0 {
                writer.write_char(',')?;
            }
            write!(writer, "{byte}")?;
        }
        write!(
            writer,
            "],\"freshness_sequence\":{}}}",
            self.freshness_sequence
        )?;
        Ok(writer.len)
    }
}

struct SliceWriter<'a> {
    bytes: &'a mut [u8],
    len: usize,
}

impl<'a> SliceWriter<'a> {
    fn new(bytes: &'a mut [u8]) -> Self {
        Self { bytes, len: 0 }
    }
}

impl Write for SliceWriter<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let end = self.len.checked_add(value.len()).ok_or(fmt::Error)?;
        let target = self.bytes.get_mut(self.len..end).ok_or(fmt::Error)?;
        target.copy_from_slice(value.as_bytes());
        self.len = end;
        Ok(())
    }
}

impl From<fmt::Error> for UsbLinkError {
    fn from(_: fmt::Error) -> Self {
        Self::BufferOverflow
    }
}
