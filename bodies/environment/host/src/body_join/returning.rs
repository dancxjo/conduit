//! Explicit new-Boot return of an already admitted Linux/Pi observer.
//! Each output directory owns one return attempt; the original binding is untouched.
use super::*;
use crate::local_recorder::recovery;
use conduit_body::{MembershipCredential, PartReturnChallenge};
use conduit_core::HostAdvertisement;
use sha2::{Digest, Sha256};

const CHALLENGE: &str = "conduit.environment/observer-return-challenge@1";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Challenge {
    schema: String,
    challenge: PartReturnChallenge,
    producer_host: conduit_core::HostId,
    producer_boot: conduit_core::BootId,
    producer_key: [u8; 32],
    signature: Vec<u8>,
}
impl Challenge {
    fn transcript(&self) -> [u8; 32] {
        let c = &self.challenge;
        let mut h = Sha256::new();
        h.update(b"conduit.environment/observer-return-challenge@1\0");
        for value in [
            c.admission_id.as_str(),
            c.body_id.as_str(),
            c.part_id.as_str(),
            c.host_id.as_str(),
            c.boot_id.as_str(),
        ] {
            text(&mut h, value);
        }
        h.update(c.offer_generation.0.to_le_bytes());
        h.update(c.nonce);
        h.update(c.issued_at_millis.to_le_bytes());
        h.update(c.expires_at_millis.to_le_bytes());
        text(&mut h, self.producer_host.as_str());
        text(&mut h, self.producer_boot.as_str());
        h.update(self.producer_key);
        h.finalize().into()
    }
    fn verify(
        &self,
        bootstrap: &Bootstrap,
        old: &MembershipCredential,
        ad: &HostAdvertisement,
    ) -> Result<()> {
        let c = &self.challenge;
        if self.schema != CHALLENGE
            || self.producer_key != bootstrap.root_key
            || self.producer_host != bootstrap.producer_host
            || self.producer_boot.as_str().is_empty()
            || c.body_id != old.body_id
            || c.part_id != old.part_id
            || c.host_id != old.host_id
            || c.boot_id != ad.boot_id
            || c.offer_generation != ad.offer_generation
            || c.admission_id.as_str().is_empty()
            || c.nonce == [0; 32]
            || c.expires_at_millis <= c.issued_at_millis
        {
            return Err("observer return challenge scope differs".into());
        }
        let signature: [u8; 64] = self
            .signature
            .as_slice()
            .try_into()
            .map_err(|_| "return challenge signature extent")?;
        checked(
            ed25519_dalek::VerifyingKey::from_bytes(&bootstrap.root_key)
                .map_err(|_| "return Root key")?
                .verify_strict(
                    &self.transcript(),
                    &ed25519_dalek::Signature::from_bytes(&signature),
                ),
        )
    }
}
fn text(h: &mut Sha256, value: &str) {
    h.update((value.len() as u64).to_le_bytes());
    h.update(value.as_bytes());
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn advertisement(old: &MembershipCredential, boot: conduit_core::BootId) -> HostAdvertisement {
    HostAdvertisement {
        declared_name: None,
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: old.host_id.clone(),
        boot_id: boot,
        offer_generation: conduit_core::OfferGeneration(1),
        profile: "conduit/environment/history-observer@1".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}
#[path = "return_exchange.rs"]
mod return_exchange;
#[cfg(test)]
use return_exchange::exchange;

fn query(bootstrap: &Bootstrap, ad: &HostAdvertisement, nonce: [u8; 32]) -> recovery::Request {
    let mut request = recovery::Request {
        schema: recovery::REQUEST.into(),
        body_id: bootstrap.invitation.claim.body_id.clone(),
        host_id: ad.host_id.clone(),
        boot_id: ad.boot_id.clone(),
        nonce,
        signature: vec![],
    };
    request.signature = ed25519_dalek::SigningKey::from_bytes(&bootstrap.invitation.secret)
        .sign(&request.transcript())
        .to_bytes()
        .to_vec();
    request
}
fn verify_receipt(
    receipt: &recovery::Receipt,
    bootstrap: &Bootstrap,
    old: &MembershipCredential,
    challenge: &Challenge,
    request: &recovery::Request,
) -> Result<()> {
    checked(receipt.verify(&bootstrap.root_key, request))?;
    if receipt.credential.part_id != old.part_id
        || receipt.producer_host != bootstrap.producer_host
        || receipt.producer_boot.as_str().is_empty()
        || receipt.credential.issued_at_millis < challenge.challenge.issued_at_millis
        || receipt.credential.issued_at_millis > challenge.challenge.expires_at_millis
    {
        return Err("observer returned receipt scope differs".into());
    }
    Ok(())
}

#[path = "return_attempt.rs"]
mod attempt;

pub(super) fn run(binding_path: &Path, continuity: &Path, state: &Path, peer: &str) -> Result<()> {
    run_until(
        binding_path,
        continuity,
        state,
        peer,
        Instant::now() + Duration::from_secs(60),
    )
}
pub(super) fn run_until(
    binding_path: &Path,
    continuity: &Path,
    state: &Path,
    peer: &str,
    deadline: Instant,
) -> Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    if Instant::now() >= deadline {
        return Err("observer return deadline expired before preparation".into());
    }
    let old_binding = crate::local_recorder::validated_binding(binding_path)?;
    let bootstrap: Bootstrap = load(continuity)?;
    if old_binding.root() != &bootstrap.root_key
        || old_binding.invitation() != &bootstrap.invitation.claim
    {
        return Err("observer return continuity/binding differs".into());
    }
    let old = old_binding.credential();
    let actual_host = format!(
        "linux/{}",
        std::fs::read_to_string("/etc/machine-id")?.trim()
    );
    if old.host_id.as_str() != actual_host {
        return Err("observer return must run on the original Linux Host".into());
    }
    if !state.exists() {
        std::fs::DirBuilder::new().mode(0o700).create(state)?;
        std::fs::File::open(state.parent().ok_or("return output parent")?)?.sync_all()?;
    }
    private(state, true)?;
    let ad_path = state.join("return-advertisement.json");
    let ad: HostAdvertisement = if ad_path.exists() {
        load(&ad_path)?
    } else {
        let boot = format!(
            "boot/{}",
            std::fs::read_to_string("/proc/sys/kernel/random/uuid")?.trim()
        )
        .into();
        let ad = advertisement(old, boot);
        save(&ad_path, &ad)?;
        ad
    };
    if ad.boot_id == old.boot_id || ad != advertisement(old, ad.boot_id.clone()) {
        return Err("retained return advertisement differs".into());
    }
    attempt::run(&bootstrap, old, &ad, state, peer, deadline)
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(super) fn fixture() -> (
        Bootstrap,
        MembershipCredential,
        HostAdvertisement,
        Challenge,
    ) {
        let key = ed25519_dalek::SigningKey::from_bytes(&[31; 32]);
        let bootstrap: Bootstrap = serde_json::from_value(serde_json::json!({
            "invitation":{"schema":"conduit.body/spawn-invitation@1", "claim":{"invitation_id":"02".repeat(32),"body_id":"01".repeat(32),"nonce":vec![7;32],"expires_at_millis":50000},"secret":vec![8;32],"rendezvous":null},
            "root_key":key.verifying_key().to_bytes(),"producer_host":"esp32/test","producer_boot":"boot/old-producer"
        })).unwrap();
        let old: MembershipCredential = serde_json::from_value(serde_json::json!({"credential_id":"03".repeat(32),"body_id":"01".repeat(32),"part_id":"04".repeat(32),"host_id":"linux/test","boot_id":"boot/old","issued_at_millis":10})).unwrap();
        let ad = advertisement(&old, "boot/new".into());
        let mut challenge = Challenge {
            schema: CHALLENGE.into(),
            challenge: PartReturnChallenge {
                admission_id: serde_json::from_value(serde_json::json!("05".repeat(32))).unwrap(),
                body_id: old.body_id.clone(),
                part_id: old.part_id.clone(),
                host_id: old.host_id.clone(),
                boot_id: ad.boot_id.clone(),
                offer_generation: ad.offer_generation,
                nonce: [9; 32],
                issued_at_millis: 100,
                expires_at_millis: 30100,
            },
            producer_host: bootstrap.producer_host.clone(),
            producer_boot: "boot/current-producer".into(),
            producer_key: bootstrap.root_key,
            signature: vec![],
        };
        challenge.signature = key.sign(&challenge.transcript()).to_bytes().to_vec();
        (bootstrap, old, ad, challenge)
    }
    #[test]
    fn challenge_requires_root_signature_and_exact_old_part_new_boot() {
        let (bootstrap, old, ad, mut challenge) = fixture();
        challenge.verify(&bootstrap, &old, &ad).unwrap();
        challenge.signature[0] ^= 1;
        assert!(challenge.verify(&bootstrap, &old, &ad).is_err());
        let (_, _, _, mut challenge) = fixture();
        challenge.challenge.part_id =
            serde_json::from_value(serde_json::json!("06".repeat(32))).unwrap();
        challenge.signature = ed25519_dalek::SigningKey::from_bytes(&[31; 32])
            .sign(&challenge.transcript())
            .to_bytes()
            .to_vec();
        assert!(challenge.verify(&bootstrap, &old, &ad).is_err());
        let (_, _, _, mut challenge) = fixture();
        challenge.challenge.boot_id = old.boot_id.clone();
        assert!(challenge.verify(&bootstrap, &old, &ad).is_err());
    }
    #[test]
    fn returned_receipt_preserves_part_and_rejects_wrong_body_or_signature() {
        let (bootstrap, old, ad, challenge) = fixture();
        let query = query(&bootstrap, &ad, challenge.challenge.nonce);
        let mut credential = old.clone();
        credential.boot_id = ad.boot_id.clone();
        credential.issued_at_millis = 200;
        let mut receipt = recovery::Receipt {
            schema: recovery::RECEIPT.into(),
            credential,
            membership_revision: 3,
            producer_host: bootstrap.producer_host.clone(),
            producer_boot: challenge.producer_boot.clone(),
            producer_key: bootstrap.root_key,
            request_nonce: query.nonce,
            signature: vec![],
        };
        let key = ed25519_dalek::SigningKey::from_bytes(&[31; 32]);
        receipt.signature = key.sign(&receipt.transcript()).to_bytes().to_vec();
        verify_receipt(&receipt, &bootstrap, &old, &challenge, &query).unwrap();
        receipt.credential.part_id =
            serde_json::from_value(serde_json::json!("06".repeat(32))).unwrap();
        receipt.signature = key.sign(&receipt.transcript()).to_bytes().to_vec();
        assert!(verify_receipt(&receipt, &bootstrap, &old, &challenge, &query).is_err());
        receipt.credential.part_id = old.part_id.clone();
        receipt.signature[0] ^= 1;
        assert!(verify_receipt(&receipt, &bootstrap, &old, &challenge, &query).is_err());
    }
}

#[cfg(test)]
mod transport_tests {
    use super::*;
    use std::net::{TcpListener, TcpStream};
    fn request(socket: &mut TcpStream) -> (String, Vec<u8>) {
        let mut header = Vec::new();
        let mut byte = [0];
        while !header.ends_with(b"\r\n\r\n") {
            socket.read_exact(&mut byte).unwrap();
            header.push(byte[0]);
        }
        let header = String::from_utf8(header).unwrap();
        let n = header
            .lines()
            .find_map(|line| line.strip_prefix("Content-Length: "))
            .unwrap()
            .parse::<usize>()
            .unwrap();
        let mut body = vec![0; n];
        socket.read_exact(&mut body).unwrap();
        (header, body)
    }
    fn respond(socket: &mut TcpStream, status: u16, body: &impl Serialize) {
        let body = serde_json::to_vec(body).unwrap();
        write!(
            socket,
            "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .unwrap();
        socket.write_all(&body).unwrap();
    }
    #[test]
    fn real_http_return_recovers_lost_finish_reply_and_preserves_original_binding() {
        let dir = super::super::tests::directory("native-return-http");
        let key = ed25519_dalek::SigningKey::from_bytes(&[31; 32]);
        let bootstrap: Bootstrap=serde_json::from_value(serde_json::json!({"invitation":{"schema":"conduit.body/spawn-invitation@1","claim":{"invitation_id":"02".repeat(32),"body_id":"01".repeat(32),"nonce":vec![7;32],"expires_at_millis":50000},"secret":vec![8;32]},"root_key":key.verifying_key().to_bytes(),"producer_host":"esp32/test","producer_boot":"boot/old-producer"})).unwrap();
        let old:MembershipCredential=serde_json::from_value(serde_json::json!({"credential_id":"03".repeat(32),"body_id":"01".repeat(32),"part_id":"04".repeat(32),"host_id":format!("linux/{}",std::fs::read_to_string("/etc/machine-id").unwrap().trim()),"boot_id":"boot/old","issued_at_millis":10})).unwrap();
        let old_query = query(
            &bootstrap,
            &advertisement(&old, old.boot_id.clone()),
            [10; 32],
        );
        let mut old_receipt = recovery::Receipt {
            schema: recovery::RECEIPT.into(),
            credential: old.clone(),
            membership_revision: 2,
            producer_host: bootstrap.producer_host.clone(),
            producer_boot: bootstrap.producer_boot.clone(),
            producer_key: bootstrap.root_key,
            request_nonce: old_query.nonce,
            signature: vec![],
        };
        old_receipt.signature = key.sign(&old_receipt.transcript()).to_bytes().to_vec();
        let binding = dir.join("original-binding.json");
        let continuity = dir.join("continuity.json");
        let output = dir.join("new-boot");
        save(&continuity, &bootstrap).unwrap();
        save(&binding,&serde_json::json!({"schema":"conduit.environment/joined-recorder@2","invitation":bootstrap.invitation.claim,"root_key":bootstrap.root_key,"local_host":old.host_id,"local_boot":old.boot_id,"recovery_request":old_query,"receipt":old_receipt})).unwrap();
        let original = std::fs::read(&binding).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let peer = listener.local_addr().unwrap().to_string();
        let expected_old = old.clone();
        let server = std::thread::spawn(move || {
            let mut challenge = None;
            let mut receipt = None;
            for step in 0..5 {
                let (mut socket, _) = listener.accept().unwrap();
                let (header, body) = request(&mut socket);
                match step {
                    0 => {
                        assert!(header.starts_with("POST /body/join "));
                        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
                        let ad: HostAdvertisement =
                            serde_json::from_value(value["host_advertisement"].clone()).unwrap();
                        assert_ne!(ad.boot_id, expected_old.boot_id);
                        assert_eq!(ad.host_id, expected_old.host_id);
                        let mut c = Challenge {
                            schema: CHALLENGE.into(),
                            challenge: PartReturnChallenge {
                                admission_id: serde_json::from_value(serde_json::json!(
                                    "05".repeat(32)
                                ))
                                .unwrap(),
                                body_id: expected_old.body_id.clone(),
                                part_id: expected_old.part_id.clone(),
                                host_id: ad.host_id.clone(),
                                boot_id: ad.boot_id.clone(),
                                offer_generation: ad.offer_generation,
                                nonce: [9; 32],
                                issued_at_millis: 100,
                                expires_at_millis: 30100,
                            },
                            producer_host: "esp32/test".into(),
                            producer_boot: "boot/current-producer".into(),
                            producer_key: key.verifying_key().to_bytes(),
                            signature: vec![],
                        };
                        c.signature = key.sign(&c.transcript()).to_bytes().to_vec();
                        challenge = Some(c);
                        respond(
                            &mut socket,
                            202,
                            &serde_json::json!({"schema":"conduit.environment/body-join-queued@1","ticket":1}),
                        );
                    }
                    1 => {
                        assert!(header.starts_with("GET /body/join/result?id=1 "));
                        respond(&mut socket, 200, challenge.as_ref().unwrap());
                    }
                    2 => {
                        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
                        let c = challenge.as_ref().unwrap();
                        assert_eq!(
                            value["schema"],
                            "conduit.environment/observer-return-request@1"
                        );
                        assert_eq!(
                            value["signature"],
                            hex(&ed25519_dalek::SigningKey::from_bytes(&[8; 32])
                                .sign(&c.challenge.signing_transcript())
                                .to_bytes())
                        );
                        let mut credential = expected_old.clone();
                        credential.boot_id = c.challenge.boot_id.clone();
                        credential.issued_at_millis = 200;
                        let mut r = recovery::Receipt {
                            schema: recovery::RECEIPT.into(),
                            credential,
                            membership_revision: 3,
                            producer_host: c.producer_host.clone(),
                            producer_boot: c.producer_boot.clone(),
                            producer_key: c.producer_key,
                            request_nonce: c.challenge.nonce,
                            signature: vec![],
                        };
                        r.signature = key.sign(&r.transcript()).to_bytes().to_vec();
                        receipt = Some(r);
                        // Root committed the return; the transport loses its reply.
                    }
                    3 => {
                        let q: recovery::Request = serde_json::from_slice(&body).unwrap();
                        assert_eq!(q.boot_id, challenge.as_ref().unwrap().challenge.boot_id);
                        respond(
                            &mut socket,
                            202,
                            &serde_json::json!({"schema":"conduit.environment/body-join-queued@1","ticket":2}),
                        );
                    }
                    4 => {
                        assert!(header.starts_with("GET /body/join/result?id=2 "));
                        respond(&mut socket, 200, receipt.as_ref().unwrap());
                    }
                    _ => unreachable!(),
                }
            }
        });
        run(&binding, &continuity, &output, &peer).unwrap();
        server.join().unwrap();
        let returned =
            crate::local_recorder::validated_binding(&output.join("recorder-binding.json"))
                .unwrap();
        assert_eq!(returned.credential().part_id, old.part_id);
        assert_eq!(returned.credential().host_id, old.host_id);
        assert_ne!(returned.credential().boot_id, old.boot_id);
        assert_eq!(std::fs::read(&binding).unwrap(), original);
        run(&binding, &continuity, &output, "127.0.0.1:1").unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
}
