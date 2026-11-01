//! Opaque proof of an exact, authenticated epoch retained in durable host files.
//! The binding owner supplies the independently pinned Body and Root. This proof
//! permits only a typed custody signature, never an arbitrary signing operation.
use crate::{checked, observed_history::Collector, Result};
use conduit_body::{BodyId, MembershipCredential};
use sha2::{Digest, Sha256};
use std::path::Path;
#[path = "observed_custody_prepare.rs"]
pub(crate) mod prepare;
#[allow(clippy::duplicate_mod)] // Same exact portable wire as the native verifier.
#[path = "../../../../targets/esp32/proof/tm1637/src/native_observed_custody_wire.rs"]
pub(crate) mod wire;

pub(crate) struct VerifiedDurableEpoch {
    root: [u8; 32],
    body: [u8; 32],
    offer: [u8; 32],
}
impl VerifiedDurableEpoch {
    pub(crate) fn verify(
        offer: &wire::Offer<'_>,
        body: &BodyId,
        root: &[u8; 32],
        directory: &Path,
    ) -> Result<Self> {
        let key = checked(ed25519_dalek::VerifyingKey::from_bytes(root))?;
        let verified = checked(wire::verify_offer(offer, body, &key))?;
        let collector = Collector::from_authenticated_scope(*verified.scope(), root)?;
        let expected = (verified.count(), *verified.digest());
        if collector.durable_summary(directory, verified.forms())? != expected {
            return Err("custody files do not cover exact closed epoch".into());
        }
        // Reopen through a new collector after the fsync boundary. The proof is
        // not minted from a cached cursor or an in-memory successful append.
        let reopened = Collector::from_authenticated_scope(*verified.scope(), root)?;
        if reopened.durable_summary(directory, verified.forms())? != expected {
            return Err("custody files changed after durable verification".into());
        }
        Ok(Self {
            root: *root,
            body: Sha256::digest(body.as_str().as_bytes()).into(),
            offer: verified.id(),
        })
    }
    pub(crate) fn root(&self) -> &[u8; 32] {
        &self.root
    }
    pub(crate) fn body_digest(&self) -> &[u8; 32] {
        &self.body
    }
    pub(crate) fn offer_hash(&self) -> &[u8; 32] {
        &self.offer
    }
    fn request<'a>(&self, credential: &'a MembershipCredential) -> Result<wire::Request<'a>> {
        if Sha256::digest(credential.body_id.as_str().as_bytes())[..] != self.body {
            return Err("custody credential Body differs".into());
        }
        let request = wire::Request {
            schema: wire::ACK,
            offer: wire::Hex(self.offer),
            part: credential.part_id.as_str(),
            host: credential.host_id.as_str(),
            boot: credential.boot_id.as_str(),
            signature: wire::Hex([0; 64]),
        };
        checked(request.validate())?;
        Ok(request)
    }
    pub(crate) fn signing_transcript(&self, credential: &MembershipCredential) -> Result<[u8; 32]> {
        Ok(self.request(credential)?.transcript())
    }
    pub(crate) fn encode_ack(
        &self,
        credential: &MembershipCredential,
        signature: [u8; 64],
    ) -> Result<Vec<u8>> {
        let mut request = self.request(credential)?;
        request.signature = wire::Hex(signature);
        let bytes = serde_json::to_vec(&request)?;
        if bytes.len() > wire::MAX_REQUEST {
            return Err("custody request frame".into());
        }
        Ok(bytes)
    }
}

/// A physical response reports the selected producer's completed operation; it
/// is never used as a substitute for signed-offer or durable-file verification.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Reclaimed {
    event: String,
    slot: u8,
    offer: wire::Hex<32>,
    reclaimed: bool,
}
trait Carrier {
    fn paced(&mut self, text: &str) -> Result<()>;
    fn event(&mut self, name: &str) -> Result<serde_json::Value>;
}
impl Carrier for crate::transport::Line {
    fn paced(&mut self, text: &str) -> Result<()> {
        self.physical_paced_text(text)
    }
    fn event(&mut self, name: &str) -> Result<serde_json::Value> {
        // Cold custody can verify several complete 960-record epochs before
        // its durable erase/readback reply. Keep the ordinary carrier deadline
        // unchanged while giving this finite maintenance operation its own bound.
        self.event_with_timeout(name, std::time::Duration::from_secs(600))
    }
}
/// A recorder, rather than the physical biography owner, signs custody of one
/// exact closed epoch using its actual admitted continuity private state.
pub(crate) fn run(
    binding: &Path,
    continuity_private: &Path,
    serial: &str,
    history: &Path,
    slot: u8,
) -> Result<()> {
    // Reject TCP before opening anything: this command is a deliberately paced
    // physical cold-service journey, with no ambient endpoint fallback.
    if serial.starts_with("tcp://") || slot > 5 {
        return Err("observed custody requires physical UART and slot 0..5".into());
    }
    let mut line = crate::transport::Line::open(serial)?;
    exchange(&mut line, binding, continuity_private, history, slot)
}
fn exchange(
    line: &mut impl Carrier,
    binding_path: &Path,
    continuity_private: &Path,
    history: &Path,
    slot: u8,
) -> Result<()> {
    if slot > 5 {
        return Err("observed custody slot must be 0..5".into());
    }
    line.paced(&format!("OBS? {slot}"))?;
    let value = line.event("observed-custody-offer")?;
    let bytes = serde_json::to_vec(&value)?;
    let (_, verified) = prepare::verify_offered(binding_path, history, &bytes, Some(slot))?;
    let request =
        crate::body_join::sign_durable_custody(binding_path, continuity_private, &verified)?;
    let command = format!("OBS ACK {}", core::str::from_utf8(&request)?);
    // The request is bounded by the shared wire; physical_paced_text also
    // checks the full command including its prefix before writing any byte.
    line.paced(&command)?;
    let reply: Reclaimed = serde_json::from_value(line.event("observed-custody-reclaimed")?)?;
    if reply.event != "observed-custody-reclaimed"
        || reply.slot != slot
        || reply.offer.0 != *verified.offer_hash()
        || !reply.reclaimed
    {
        return Err("observed custody reclamation reply differs".into());
    }
    println!("observed epoch slot {slot}: durable recorder custody acknowledged and producer reports reclaimed");
    Ok(())
}

#[cfg(test)]
#[allow(clippy::duplicate_mod)] // Exact producer/collector signature transcript.
#[path = "../../../../targets/esp32/proof/tm1637/src/native_observed_wire.rs"]
mod history_wire;

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::Signer;
    use std::{collections::VecDeque, os::unix::fs::PermissionsExt, path::PathBuf};
    struct Fixture {
        directory: PathBuf,
        binding: PathBuf,
        private: PathBuf,
        history: PathBuf,
        batch: PathBuf,
        offer: serde_json::Value,
        reply: serde_json::Value,
        continuity: ed25519_dalek::SigningKey,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }
    fn fixture() -> Fixture {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "conduit-observed-ack-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
        let root = ed25519_dalek::SigningKey::from_bytes(&[41; 32]);
        let body = "02".repeat(32);
        let invitation = serde_json::json!({"schema":conduit_body::INVITATION_SCHEMA,"claim":{"invitation_id":"08".repeat(32),"body_id":body,"nonce":vec![3;32],"expires_at_millis":999999},"secret":vec![22;32]});
        let bootstrap = serde_json::json!({"invitation":invitation,"root_key":root.verifying_key().to_bytes(),"producer_host":"06".repeat(32),"producer_boot":"07".repeat(32)});
        let credential: MembershipCredential = serde_json::from_value(serde_json::json!({"credential_id":"01".repeat(32),"body_id":body,"part_id":"03".repeat(32),"host_id":"04".repeat(32),"boot_id":"05".repeat(32),"issued_at_millis":123})).unwrap();
        let mut receipt = crate::local_recorder::joined::JoinedHostReceipt {
            schema: crate::local_recorder::joined::PROFILE.into(),
            credential: credential.clone(),
            membership_revision: 2,
            producer_host: "06".repeat(32).into(),
            producer_boot: "07".repeat(32).into(),
            producer_key: root.verifying_key().to_bytes(),
            invitation_id: serde_json::from_value(invitation["claim"]["invitation_id"].clone())
                .unwrap(),
            offer_generation: conduit_core::OfferGeneration(1),
            signature: vec![],
        };
        receipt.signature = root
            .sign(&crate::local_recorder::joined::transcript(&receipt.scope()))
            .to_bytes()
            .to_vec();
        let binding = directory.join("binding.json");
        let private = directory.join("continuity.json");
        for (path, value) in [
            (
                &binding,
                serde_json::json!({"schema":"conduit.environment/joined-recorder@1","invitation":invitation["claim"],"root_key":root.verifying_key().to_bytes(),"local_host":credential.host_id,"local_boot":credential.boot_id,"receipt":receipt}),
            ),
            (&private, bootstrap),
        ] {
            std::fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        let mut scope = [9; 96];
        scope[..32].copy_from_slice(&Sha256::digest(body.as_bytes()));
        let contract =
            conduit_environment_runtime::observation::ObservationContract::prepare().unwrap();
        let forms: [u8; 32] = Sha256::digest(
            conduit_environment_runtime::observed_record::descriptor(&contract).unwrap(),
        )
        .into();
        let record = conduit_environment_runtime::observed_record::ObservedRecord {
            sequence: 1,
            observed_at_boot_ms: 123,
            disposition:
                conduit_environment_runtime::recording::RecordingDisposition::ProviderFailed,
            quantities: None,
        }
        .encode()
        .unwrap();
        let hex = |bytes: &[u8]| {
            bytes
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        };
        let history = directory.join("history");
        let epoch = history.join(format!("epoch-{}", hex(&scope)));
        std::fs::create_dir_all(&epoch).unwrap();
        let batch = epoch.join("observed-history-0000-0001.json");
        std::fs::write(&batch,serde_json::to_vec(&serde_json::json!({"profile":"observed-history@1","scope":hex(&scope),"forms":hex(&forms),"after":0,"committed":1,"records":hex(&record),"signature":hex(&root.sign(&history_wire::transcript(&scope,&forms,0,1,&record)).to_bytes())})).unwrap()).unwrap();
        let mut offer = wire::Offer {
            schema: wire::OFFER,
            region: wire::Hex([8; 32]),
            scope: wire::Hex(scope),
            forms: wire::Hex(forms),
            committed: 1,
            digest: wire::Hex(Sha256::digest(record).into()),
            signature: wire::Hex([0; 64]),
        };
        let offer_id = wire::offer_transcript(&offer, &root.verifying_key().to_bytes());
        offer.signature = wire::Hex(root.sign(&offer_id).to_bytes());
        Fixture {
            directory,
            binding,
            private,
            history,
            batch,
            offer: serde_json::json!({"event":"observed-custody-offer","slot":5,"offer":offer}),
            reply: serde_json::json!({"event":"observed-custody-reclaimed","slot":5,"offer":hex(&offer_id),"reclaimed":true}),
            continuity: ed25519_dalek::SigningKey::from_bytes(&[22; 32]),
        }
    }
    #[derive(Default)]
    struct Mock {
        sent: Vec<String>,
        replies: VecDeque<serde_json::Value>,
    }
    impl Carrier for Mock {
        fn paced(&mut self, text: &str) -> Result<()> {
            self.sent.push(text.into());
            Ok(())
        }
        fn event(&mut self, _: &str) -> Result<serde_json::Value> {
            self.replies
                .pop_front()
                .ok_or_else(|| "missing mock event".into())
        }
    }
    #[test]
    fn actual_recorder_ack_uses_exact_durable_scope_and_continuity_signature() {
        let f = fixture();
        let mut line = Mock {
            replies: VecDeque::from([f.offer.clone(), f.reply.clone()]),
            ..Mock::default()
        };
        exchange(&mut line, &f.binding, &f.private, &f.history, 5).unwrap();
        assert_eq!(line.sent.len(), 2);
        assert_eq!(line.sent[0], "OBS? 5");
        let bytes = line.sent[1].strip_prefix("OBS ACK ").unwrap();
        assert!(line.sent[1].len() <= 1024);
        let request: wire::Request<'_> = serde_json::from_str(bytes).unwrap();
        assert_eq!(request.part, "03".repeat(32));
        assert_eq!(request.host, "04".repeat(32));
        assert_eq!(request.boot, "05".repeat(32));
        f.continuity
            .verifying_key()
            .verify_strict(
                &request.transcript(),
                &ed25519_dalek::Signature::from_bytes(&request.signature.0),
            )
            .unwrap();
        assert!(
            f.batch.exists(),
            "host retained history must never be removed by ACK"
        );
    }
    #[test]
    fn wrong_offer_or_missing_corrupt_history_never_sends_ack() {
        for mutation in 0..5 {
            let mut f = fixture();
            match mutation {
                0 => f.offer["slot"] = 4.into(),
                1 => f.offer["offer"]["digest"] = "00".repeat(32).into(),
                2 => std::fs::remove_file(&f.batch).unwrap(),
                3 => std::fs::write(&f.batch, b"corrupt").unwrap(),
                4 => {
                    let mut binding: serde_json::Value =
                        serde_json::from_slice(&std::fs::read(&f.binding).unwrap()).unwrap();
                    binding["root_key"] = serde_json::to_value([42; 32]).unwrap();
                    std::fs::write(&f.binding, serde_json::to_vec(&binding).unwrap()).unwrap();
                }
                _ => unreachable!(),
            }
            let mut line = Mock {
                replies: VecDeque::from([f.offer.clone()]),
                ..Mock::default()
            };
            assert!(exchange(&mut line, &f.binding, &f.private, &f.history, 5).is_err());
            assert!(line.sent.len() <= 1);
        }
    }
    #[test]
    fn private_identity_and_reclaimed_reply_must_match() {
        for mutation in 0..5 {
            let mut f = fixture();
            if mutation == 0 {
                let mut private: serde_json::Value =
                    serde_json::from_slice(&std::fs::read(&f.private).unwrap()).unwrap();
                private["invitation"]["claim"]["invitation_id"] = "ff".repeat(32).into();
                std::fs::write(&f.private, serde_json::to_vec(&private).unwrap()).unwrap();
            } else {
                match mutation {
                    1 => f.reply["slot"] = 4.into(),
                    2 => f.reply["offer"] = "00".repeat(32).into(),
                    3 => f.reply["reclaimed"] = false.into(),
                    4 => f.reply["unexpected"] = true.into(),
                    _ => unreachable!(),
                }
            }
            let mut line = Mock {
                replies: VecDeque::from([f.offer.clone(), f.reply.clone()]),
                ..Mock::default()
            };
            assert!(exchange(&mut line, &f.binding, &f.private, &f.history, 5).is_err());
            assert_eq!(line.sent.len(), if mutation == 0 { 1 } else { 2 });
            assert!(f.batch.exists());
        }
    }
    #[test]
    fn invalid_slot_and_tcp_are_refused_before_opening_a_carrier() {
        for (serial, slot) in [("tcp://127.0.0.1:1", 0), ("not-a-device", 6)] {
            assert!(run(
                Path::new("absent"),
                Path::new("absent"),
                serial,
                Path::new("absent"),
                slot
            )
            .is_err());
        }
    }
    include!("observed_custody_prepare_tests.rs");
}
