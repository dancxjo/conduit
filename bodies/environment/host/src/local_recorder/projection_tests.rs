//! Actual admitted binding, signed localhost collector and fsynced projection.
use super::tests::canonical_admission;
use super::*;
use crate::observed_history::{retained::Projection, wire};
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::PathBuf,
};
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
struct Fixture {
    root: PathBuf,
    binding: PathBuf,
    history: PathBuf,
    output: PathBuf,
    boot: String,
    scope: [u8; 96],
    forms: [u8; 32],
    key: SigningKey,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "conduit-auto-projection-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let (key, receipt, claim) = canonical_admission(&root);
        let binding = root.join("binding.json");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&binding)
            .unwrap();
        serde_json::to_writer(&mut file,&serde_json::json!({"schema":"conduit.environment/joined-recorder@1","invitation":claim,"root_key":key.verifying_key().to_bytes(),"local_host":receipt.credential.host_id,"local_boot":receipt.credential.boot_id,"receipt":receipt})).unwrap();
        let boot = "boot/projection-source".to_owned();
        let mut scope = [0; 96];
        scope[..32].copy_from_slice(&Sha256::digest(
            receipt.credential.body_id.as_str().as_bytes(),
        ));
        scope[32..64].copy_from_slice(&Sha256::digest(boot.as_bytes()));
        scope[64..].fill(9);
        let forms = Sha256::digest(
            conduit_environment_runtime::observed_record::descriptor(
                &conduit_environment_runtime::observation::ObservationContract::prepare().unwrap(),
            )
            .unwrap(),
        )
        .into();
        Self {
            binding,
            history: root.join("history"),
            output: root.join("series.json"),
            root,
            boot,
            scope,
            forms,
            key,
        }
    }
    fn batch(&self, scope: [u8; 96], after: u32, end: u32) -> Vec<u8> {
        let records:Vec<_>=(after+1..=end).flat_map(|sequence| conduit_environment_runtime::observed_record::ObservedRecord {
            sequence:sequence.into(), observed_at_boot_ms: u64::from(sequence)*731,
            disposition:conduit_environment_runtime::recording::RecordingDisposition::ProviderFailed, quantities:None,
        }.encode().unwrap()).collect();
        serde_json::to_vec(&serde_json::json!({"profile":"observed-history@1","scope":hex(&scope),"forms":hex(&self.forms),"after":after,"committed":end,"records":hex(&records),"signature":hex(&self.key.sign(&wire::transcript(&scope,&self.forms,after,end,&records)).to_bytes())})).unwrap()
    }
    fn listing(&self) -> Vec<u8> {
        let mut raw = [0; wire::DIRECTORY_BYTES];
        raw[..32].copy_from_slice(&self.scope[..32]);
        raw[32] = 2;
        let mut other = self.scope;
        other[32..64].fill(5);
        for (entry, (slot, scope)) in raw[33..299]
            .as_chunks_mut::<133>()
            .0
            .iter_mut()
            .zip([(0, self.scope), (1, other)])
        {
            entry[0] = slot;
            entry[1..97].copy_from_slice(&scope);
            entry[97..129].copy_from_slice(&self.forms);
            entry[129..].copy_from_slice(&2u32.to_le_bytes());
        }
        serde_json::to_vec(&serde_json::json!({"profile":"observed-epochs@1","directory":hex(&raw),"signature":hex(&self.key.sign(&wire::directory_transcript(&raw)).to_bytes())})).unwrap()
    }
    fn projection(&self) -> Projection<'_> {
        Projection {
            producer_boot: &self.boot,
            output: &self.output,
        }
    }
    fn collect(&self, after: u32, end: u32) {
        assert!(!self.collect_attempt(after, end, true));
    }
    fn collect_attempt(&self, after: u32, end: u32, interrupted: bool) -> bool {
        let mut responses = vec![(wire::DIRECTORY_CURSOR, Some(self.listing()))];
        if end > after {
            responses.push((after, Some(self.batch(self.scope, after, end))));
        }
        responses.push((end, Some(self.batch(self.scope, end, end))));
        // A later original epoch fails after the target prefix became durable.
        let mut other = self.scope;
        other[32..64].fill(5);
        responses.push((
            wire::epoch_cursor(1, 0).unwrap(),
            (!interrupted).then(|| self.batch(other, 0, 0)),
        ));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            for (cursor, body) in responses {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut b = [0];
                    socket.read_exact(&mut b).unwrap();
                    request.push(b[0]);
                    assert!(request.len() < 1024);
                }
                assert!(std::str::from_utf8(&request)
                    .unwrap()
                    .starts_with(&format!("GET /history?after={cursor} ")));
                if let Some(body) = body {
                    socket
                        .write_all(b"HTTP/1.0 200 OK\r\nConnection: close\r\n\r\n")
                        .unwrap();
                    socket.write_all(&body).unwrap();
                }
            }
        });
        let result = run_joined(
            &self.binding,
            &address.to_string(),
            &self.history,
            true,
            Some(self.projection()),
        );
        worker.join().unwrap();
        result.is_ok()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
#[test]
fn signed_partial_collection_projects_updates_and_restart_without_rewriting_unchanged_output() {
    let f = Fixture::new();
    let binding = fs::read(&f.binding).unwrap();
    f.collect(0, 1);
    let first = fs::read(&f.output).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&first).unwrap();
    assert_eq!(value["verified_through_sequence"], 1);
    assert_eq!(value["producer_boot"], f.boot);
    f.collect(1, 2);
    let second = fs::read(&f.output).unwrap();
    assert_ne!(first, second);
    let value: serde_json::Value = serde_json::from_slice(&second).unwrap();
    assert_eq!(value["verified_through_sequence"], 2);
    let records: Vec<_> = value["records_hex"]
        .as_str()
        .unwrap()
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    assert_eq!(u64::from_le_bytes(records[8..16].try_into().unwrap()), 731);
    let inode = fs::metadata(&f.output).unwrap().ino();
    f.collect(2, 2);
    assert_eq!(fs::metadata(&f.output).unwrap().ino(), inode);
    assert_eq!(fs::read(&f.output).unwrap(), second);
    assert_eq!(fs::read(&f.binding).unwrap(), binding); // Projection never rewrites admission/custody.
    let epoch = f.history.join(format!("epoch-{}", hex(&f.scope)));
    let mut foreign = f.scope;
    foreign[64] ^= 1;
    let successor = epoch.join("observed-history-0002-0003.json");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&successor)
        .unwrap();
    file.write_all(&f.batch(foreign, 2, 3)).unwrap();
    assert!(f.projection().refresh(&f.binding, &f.history).is_err());
    assert_eq!(fs::read(&f.output).unwrap(), second);
    fs::write(&successor, b"corrupt interrupted successor").unwrap();
    assert!(f.projection().refresh(&f.binding, &f.history).is_err());
    assert_eq!(fs::read(&f.output).unwrap(), second);
}
#[test]
fn absent_or_ambiguous_pinned_play_and_unsafe_output_preserve_previous_series() {
    let f = Fixture::new();
    f.collect(0, 1);
    let previous = fs::read(&f.output).unwrap();
    let wrong = Projection {
        producer_boot: "boot/other",
        output: &f.output,
    };
    assert!(wrong.refresh(&f.binding, &f.history).is_err());
    let mut other = f.scope;
    other[64] ^= 1;
    let directory = f.history.join(format!("epoch-{}", hex(&other)));
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .unwrap();
    let path = directory.join("observed-history-0000-0001.json");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.write_all(&f.batch(other, 0, 1)).unwrap();
    assert!(f.projection().refresh(&f.binding, &f.history).is_err());
    assert_eq!(fs::read(&f.output).unwrap(), previous);
    fs::remove_dir_all(directory).unwrap();
    let saved = f.root.join("saved.json");
    fs::rename(&f.output, &saved).unwrap();
    std::os::unix::fs::symlink(&saved, &f.output).unwrap();
    assert!(f.projection().refresh(&f.binding, &f.history).is_err());
    assert_eq!(fs::read(&saved).unwrap(), previous);
}

#[test]
fn projection_refusal_cannot_turn_successful_signed_collection_into_failure() {
    let f = Fixture::new();
    let saved = f.root.join("unrelated-private-output.json");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&saved)
        .unwrap();
    file.write_all(b"previous graph").unwrap();
    std::os::unix::fs::symlink(&saved, &f.output).unwrap();
    assert!(f.collect_attempt(0, 1, false));
    assert_eq!(fs::read(&saved).unwrap(), b"previous graph");
    let epoch = f.history.join(format!("epoch-{}", hex(&f.scope)));
    assert!(epoch.join("observed-history-0000-0001.json").is_file());
}
