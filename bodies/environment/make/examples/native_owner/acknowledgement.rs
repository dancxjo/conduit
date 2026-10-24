//! The verified admission output pins a Boot key. A reply cannot choose it.
use super::{control, document};
use sha2::{Digest, Sha256};
#[allow(dead_code)]
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_owner_receipt.rs"]
pub(super) mod receipt;
use conduit_core::HostAdvertisement;
use std::{error::Error, path::Path};
pub struct VerifiedPublication {
    pub snapshot: receipt::Snapshot,
    pub canonical: Vec<u8>,
}
pub struct AdmittedBoot {
    key: [u8; 32],
}
impl AdmittedBoot {
    pub fn load(directory: &Path, host: &HostAdvertisement) -> Result<Self, Box<dyn Error>> {
        let evidence = document(directory.join("admission.json"))?;
        let binding = &evidence["verified_boot_key"];
        if binding["host_id"] != host.host_id.as_str()
            || binding["boot_id"] != host.boot_id.as_str()
        {
            return Err("verified Boot key scope unavailable".into());
        }
        Ok(Self {
            key: serde_json::from_value(binding["verifying_key"].clone())?,
        })
    }
    pub fn transition(
        &self,
        request: &control::Request<'_>,
        reply: &serde_json::Value,
    ) -> Result<(), Box<dyn Error>> {
        let state = match request.operation {
            control::Operation::Start => receipt::State::StartedHeld,
            control::Operation::Release => receipt::State::Released,
            control::Operation::Cancel => receipt::State::Quiesced,
            control::Operation::Retire => receipt::State::Retired,
            _ => return Err("not a lifecycle acknowledgement".into()),
        };
        self.verify(request, reply, state)?;
        Ok(())
    }
    pub fn publication(
        &self,
        request: &control::Request<'_>,
        reply: &serde_json::Value,
    ) -> Result<Option<VerifiedPublication>, Box<dyn Error>> {
        if request.operation != control::Operation::Inspect {
            return Err("not an inspection".into());
        }
        if reply["publication_bytes"] == 0 {
            return Ok(None);
        }
        let receipt = self.verify(request, reply, receipt::State::Observed)?;
        let snapshot = receipt.snapshot.ok_or("missing signed snapshot")?;
        let bytes: Vec<u8> = serde_json::from_value(reply["publication"].clone())?;
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        if bytes.len() != usize::from(snapshot.bytes) || digest != snapshot.sha256 {
            return Err("signed publication bytes differ".into());
        }
        Ok(Some(VerifiedPublication {
            snapshot,
            canonical: bytes,
        }))
    }
    fn verify(
        &self,
        request: &control::Request<'_>,
        reply: &serde_json::Value,
        state: receipt::State,
    ) -> Result<receipt::Receipt, Box<dyn Error>> {
        let command = request
            .transcript()
            .map_err(|e| format!("command: {e:?}"))?;
        let receipt: receipt::Receipt = serde_json::from_value(reply["acknowledgement"].clone())?;
        let signature: Vec<u8> =
            serde_json::from_value(reply["acknowledgement_signature"].clone())?;
        let signature: [u8; 64] = signature
            .try_into()
            .map_err(|_| "acknowledgement signature bound")?;
        let verified = receipt
            .verify(&self.key, &command, state, &signature)
            .map_err(|e| format!("device acknowledgement attribution: {e:?}"))?;
        if verified.into_state() != state {
            return Err("unexpected acknowledgement".into());
        }
        Ok(receipt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    fn request() -> control::Request<'static> {
        control::Request {
            protocol: control::PROTOCOL,
            operation: control::Operation::Inspect,
            sequence: 3,
            host: "host/cpu",
            boot: "boot/cpu",
            offer_generation: 1,
            body: "body/cpu",
            wake: "wake/cpu",
            body_plan: "body-plan/cpu",
            body_play: "body-play/cpu",
            plan: "plan/cpu",
            fragment: "fragment/cpu",
            fragment_play: "fragment-play/cpu",
            assignment_seal: [7; 32],
        }
    }
    #[test]
    fn host_verifies_actual_publication_bytes_with_its_pinned_boot_key() {
        let signer = SigningKey::from_bytes(&[21; 32]);
        let admitted = AdmittedBoot {
            key: signer.verifying_key().to_bytes(),
        };
        let canonical = vec![1, 2, 3, 4]; // Payload identity proof, not a semantic reading.
        let signed = receipt::Receipt {
            command: request().transcript().unwrap(),
            state: receipt::State::Observed,
            snapshot: Some(receipt::Snapshot {
                sequence: 4,
                committed_millis: 100,
                observed_millis: 150,
                channel: 0,
                bytes: 4,
                sha256: Sha256::digest(&canonical).into(),
            }),
        };
        let signature = signer.sign(&signed.transcript()).to_bytes();
        let reply = serde_json::json!({"acknowledgement":signed,
            "acknowledgement_signature":signature.as_slice(),"publication_bytes":4,
            "publication":canonical});
        assert_eq!(
            admitted
                .publication(&request(), &reply)
                .unwrap()
                .unwrap()
                .canonical,
            canonical
        );
        for change in 0..5 {
            let mut altered = reply.clone();
            match change {
                0 => altered["publication"][0] = 9.into(),
                1 => altered["publication"] = serde_json::json!([1, 2, 3]),
                2 => altered["acknowledgement"]["snapshot"]["sequence"] = 5.into(),
                3 => altered["acknowledgement_signature"] = serde_json::json!([0]),
                _ => altered["acknowledgement"]["state"] = "StartedHeld".into(),
            }
            assert!(admitted.publication(&request(), &altered).is_err());
        }
        let mut stale = request();
        stale.sequence += 1;
        assert!(admitted.publication(&stale, &reply).is_err());
        let foreign = AdmittedBoot {
            key: SigningKey::from_bytes(&[22; 32]).verifying_key().to_bytes(),
        };
        assert!(foreign.publication(&request(), &reply).is_err());
    }
}
