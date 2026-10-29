//! CPU restart custody: actual canonical Body lifecycle with explicit stand-in
//! Core Plans. This supplies no native execution, flash or same-Body join proof.
#![allow(unexpected_cfgs)]
#![allow(clippy::duplicate_mod)] // Exercise independent legacy codec and production restart decoder.
extern crate alloc;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

#[path = "../../../../targets/esp32/proof/tm1637/src/native_body_checkpoint.rs"]
#[allow(dead_code)]
mod checkpoint;
use checkpoint as native_body_checkpoint;
mod native_evidence_region {
    pub struct BodyRegion(pub Vec<u8>);
    pub fn test_empty() -> BodyRegion {
        BodyRegion(vec![255; 65536])
    }
    impl super::checkpoint::Storage for BodyRegion {
        fn capacity(&self) -> u32 {
            self.0.len() as u32
        }
        fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), super::checkpoint::Refusal> {
            out.copy_from_slice(
                self.0
                    .get(at as usize..at as usize + out.len())
                    .ok_or(super::checkpoint::Refusal::Storage)?,
            );
            Ok(())
        }
        fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), super::checkpoint::Refusal> {
            for (old, new) in self.0[at as usize..at as usize + bytes.len()]
                .iter_mut()
                .zip(bytes)
            {
                assert_eq!(*old & new, *new);
                *old &= new;
            }
            Ok(())
        }
        fn erase_bank(&mut self, bank: u8) -> Result<(), super::checkpoint::Refusal> {
            self.0[usize::from(bank) * 32768..(usize::from(bank) + 1) * 32768].fill(255);
            Ok(())
        }
    }
}

#[path = "../../../../targets/esp32/proof/tm1637/src/native_restart_compress.rs"]
#[allow(dead_code)]
mod codec;
#[allow(dead_code)]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_restart.rs"]
mod restart;
#[test]
#[ignore = "requires local private flash backup via CONDUIT_PRIVATE_FLASH"]
fn private_actual_next_boot_archive_checkpoint() {
    let Ok(path) = std::env::var("CONDUIT_PRIVATE_FLASH") else {
        return;
    };
    let flash = std::fs::read(path).unwrap();
    let region = flash[0x3f0000..0x400000].to_vec();
    let len = u32::from_le_bytes(region[16..20].try_into().unwrap()) as usize;
    let encoded = &region[160..160 + len];
    let mut at = 0;
    let mut decoder = codec::Decoder::new(
        |out: &mut [u8; 128]| {
            let n = 128.min(encoded.len() - at);
            out[..n].copy_from_slice(&encoded[at..at + n]);
            at += n;
            Ok(n)
        },
        131072,
        32608,
    );
    let mut plain = Vec::new();
    let mut chunk = [0; 128];
    loop {
        let n = decoder.read(&mut chunk).unwrap();
        if n == 0 {
            break;
        }
        plain.extend_from_slice(&chunk[..n]);
    }
    drop(decoder);
    let value: serde_json::Value = serde_json::from_slice(&plain).unwrap();
    let body_id = serde_json::from_value(value["evidence"]["body_id"].clone()).unwrap();
    drop(value);
    drop(plain);
    let retained = restart::payload::decode(encoded, &body_id).unwrap();
    let compiled = retained.evidence;
    drop(retained.archives);
    restart::initialize(native_evidence_region::BodyRegion(region), compiled).unwrap();
    let seed = restart::select_signer_seed([99; 32]).unwrap();
    let key = ed25519_dalek::SigningKey::from_bytes(&seed)
        .verifying_key()
        .to_bytes();
    let retained = restart::take_body().unwrap();
    assert!(!retained.pristine);
    let host = retained
        .evidence
        .membership
        .parts
        .iter()
        .find_map(|p| p.current.as_ref())
        .unwrap()
        .host_id
        .clone();
    let boot = conduit_core::BootId::new("private-cpu-recovery-next-boot");
    let mut owner =
        conduit_body::SealedBodyLifecycleSession::resume_here(retained.evidence, &host, &boot)
            .unwrap();
    let mut commits = 0;
    while owner.seal_terminal_wake_for_checkpoint().unwrap() {
        restart::checkpoint(&mut owner, &key).unwrap();
        commits += 1;
    }
    owner.seal_membership_history_for_checkpoint().unwrap();
    restart::checkpoint(&mut owner, &key).unwrap();
    assert!(restart::publication_ready());
    assert_eq!(restart::select_signer_seed([98; 32]).unwrap(), seed);
    let before = owner.evidence().clone();
    let mut boxed = Some(Box::new(owner));
    let token = restart::park_box(&mut boxed, &key, &host, &boot).unwrap();
    assert!(boxed.is_none());
    let restored = restart::restore_parked(token, &host, &boot, &key).unwrap();
    assert_eq!(restored.evidence(), &before);
    eprintln!(
        "actual nextBoot complete commits={} generation={:?}",
        commits + 1,
        restart::generation()
    );
}
