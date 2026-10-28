//! Actual native archive with mocked finite NOR and heap admission. No device proof.
extern crate alloc;
extern crate self as esp_alloc;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_network_join_call::recovery_evidence as wire;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
static FREE: AtomicUsize = AtomicUsize::new(1_000_000);
static DRAIN_AFTER_PREFLIGHT: AtomicBool = AtomicBool::new(false);
pub struct Heap;
pub static HEAP: Heap = Heap;
impl Heap {
    pub fn free(&self) -> usize {
        let free = FREE.load(Ordering::Relaxed);
        if DRAIN_AFTER_PREFLIGHT.swap(false, Ordering::Relaxed) {
            FREE.store(8191, Ordering::Relaxed);
        }
        free
    }
}
struct Flash {
    bytes: Vec<u8>,
    writes: usize,
    fail_commit_read: bool,
    fail_read: bool,
}
mod native_evidence_region {
    use super::*;
    pub struct NetworkRegion(pub Arc<Mutex<Flash>>);
    impl NetworkRegion {
        pub(crate) fn identity(&self) -> [u8; 32] {
            [0x41; 32]
        }
    }
    impl conduit_environment_runtime::display_evidence_journal::Storage for NetworkRegion {
        fn capacity(&self) -> u32 {
            wire::CAPACITY
        }
        fn read(
            &mut self,
            at: u32,
            out: &mut [u8],
        ) -> Result<(), conduit_environment_runtime::display_evidence_journal::Refusal> {
            let mut f = self.0.lock().unwrap();
            if f.fail_read {
                f.fail_read = false;
                return Err(conduit_environment_runtime::display_evidence_journal::Refusal::Media);
            }
            out.copy_from_slice(&f.bytes[at as usize..at as usize + out.len()]);
            Ok(())
        }
        fn program(
            &mut self,
            at: u32,
            bytes: &[u8],
        ) -> Result<(), conduit_environment_runtime::display_evidence_journal::Refusal> {
            let mut f = self.0.lock().unwrap();
            f.writes += 1;
            for (old, new) in f.bytes[at as usize..at as usize + bytes.len()]
                .iter_mut()
                .zip(bytes)
            {
                assert_eq!(*old & *new, *new);
                *old &= *new;
            }
            if bytes == b"nce1" && f.fail_commit_read {
                f.fail_read = true;
                f.fail_commit_read = false;
            }
            Ok(())
        }
    }
}
struct Kernel(conduit_kernel::FixedSignLog<128>);
impl Kernel {
    fn signs(&self) -> &conduit_kernel::FixedSignLog<128> {
        &self.0
    }
}
struct Evidence {
    kernel: Kernel,
}
struct RetiredNetworkControl {
    evidence: Evidence,
    active: conduit_core::ActivePlayIdentity,
    provider_generation: u64,
    configuration_revision: u64,
}
#[allow(dead_code)]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_network_journal.rs"]
pub(crate) mod journal;
include!("native_network_export_adapter.rs");
fn retired(n: u64) -> RetiredNetworkControl {
    use conduit_kernel::SignSink;
    let mut signs = conduit_kernel::FixedSignLog::<128>::new(
        (128 * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32,
    )
    .unwrap();
    for i in 0..128 {
        signs
            .record(
                conduit_kernel::NodeId(i % 3),
                None,
                Some(conduit_kernel::RequestId(u32::from(i))),
                conduit_kernel::KernelEventKind::HostCallCompleted,
            )
            .unwrap();
    }
    RetiredNetworkControl {
        evidence: Evidence {
            kernel: Kernel(signs),
        },
        active: conduit_core::ActivePlayIdentity {
            host_id: conduit_core::HostId::new("host"),
            boot_id: conduit_core::BootId::new(format!("boot-{n}")),
            plan_id: conduit_core::PlanId::new("plan"),
            active_play_id: conduit_core::ActivePlayId::new(format!("play-{n}")),
            play_sequence: n,
        },
        provider_generation: n,
        configuration_revision: 1,
    }
}
#[test]
fn transient_scratch_refusal_exact_retry_and_all_boots() {
    let flash = Arc::new(Mutex::new(Flash {
        bytes: vec![255; wire::CAPACITY as usize],
        writes: 0,
        fail_commit_read: false,
        fail_read: false,
    }));
    let region = native_evidence_region::NetworkRegion(flash.clone());
    let (opened, allocation) = allocation_probe::observe(|| journal::initialize(region));
    assert_eq!(opened, Ok(()));
    assert_eq!(allocation.requested_bytes, wire::MAXIMUM_BYTES);
    assert_eq!(allocation.retained_bytes(), 0);
    let first = retired(1);
    FREE.store(8192 + wire::MAXIMUM_BYTES - 1, Ordering::Relaxed);
    assert_eq!(journal::append(&first), Err("network-evidence-workspace"));
    FREE.store(1_000_000, Ordering::Relaxed);
    {
        let _refusal = allocation_probe::refuse_next_allocation_size(wire::MAXIMUM_BYTES);
        assert_eq!(journal::append(&first), Err("network-evidence-workspace"));
    }
    DRAIN_AFTER_PREFLIGHT.store(true, Ordering::Relaxed);
    let (refused, allocation) = allocation_probe::observe(|| journal::append(&first));
    assert_eq!(refused, Err("network-evidence-workspace"));
    assert_eq!(allocation.requested_bytes, wire::MAXIMUM_BYTES);
    assert_eq!(allocation.retained_bytes(), 0);
    FREE.store(1_000_000, Ordering::Relaxed);
    assert_eq!(flash.lock().unwrap().writes, 0);
    assert!(flash.lock().unwrap().bytes.iter().all(|byte| *byte == 255));
    for n in 1..=8 {
        let record = retired(n);
        if n == 8 {
            flash.lock().unwrap().fail_commit_read = true;
        }
        let (result, allocation) = allocation_probe::observe(|| journal::append(&record));
        assert_eq!(allocation.requested_bytes, wire::MAXIMUM_BYTES);
        assert_eq!(allocation.retained_bytes(), 0);
        if n == 8 {
            assert_eq!(result, Err("network-evidence-commit"));
            assert!(journal::exhausted());
            let writes = flash.lock().unwrap().writes;
            assert_eq!(journal::append(&record), Ok(()));
            assert_eq!(flash.lock().unwrap().writes, writes);
        } else {
            assert_eq!(result, Ok(()));
        }
    }
    let writes = flash.lock().unwrap().writes;
    assert_eq!(
        journal::append(&retired(9)),
        Err(journal::capacity_refusal())
    );
    let f = flash.lock().unwrap();
    assert_eq!(f.writes, writes);
    for n in 1..=8u64 {
        let at = (n as usize - 1) * wire::SLOT_BYTES as usize;
        let bytes: &[u8; wire::MAXIMUM_BYTES] =
            f.bytes[at..at + wire::MAXIMUM_BYTES].try_into().unwrap();
        let page = wire::Page::parse(bytes).unwrap();
        assert_eq!(page.scope().boot, format!("boot-{n}"));
        assert_eq!(page.scope().provider_generation, n);
        assert_eq!(page.events().count(), 128);
        let record = retired(n);
        let mut expected = [0; wire::MAXIMUM_BYTES];
        let s = record.evidence.kernel.signs();
        wire::encode(page.scope(), s.events(), s, &mut expected).unwrap();
        assert_eq!(bytes, &expected);
    }
    drop(f);
    // Include unused tail bytes, not just reconstructed committed records.
    flash.lock().unwrap().bytes[wire::CAPACITY as usize - 1] = 0x7f;
    let expected = flash.lock().unwrap().bytes.clone();
    let mut inspection = journal::inspect().unwrap();
    assert_eq!(inspection.committed_count(), 8);
    assert!(journal::exhausted());
    assert!(journal::inspect().is_err());
    assert_eq!(
        journal::append(&retired(9)),
        Err("network-evidence-unavailable")
    );
    let mut actual = vec![0; wire::CAPACITY as usize];
    for (n, chunk) in actual.chunks_mut(512).enumerate() {
        inspection.read_chunk(n as u32 * 512, chunk).unwrap();
    }
    assert_eq!(actual, expected);
    assert!(inspection.read_chunk(wire::CAPACITY, &mut [0]).is_err());
    assert!(inspection.read_chunk(0, &mut [0; 513]).is_err());
    flash.lock().unwrap().fail_read = true;
    assert!(inspection.read_chunk(0, &mut [0; 16]).is_err());
    let (_, allocation) = allocation_probe::observe(|| drop(inspection));
    assert_eq!(allocation.requested_bytes, 0);
    FREE.store(8191, Ordering::Relaxed);
    assert_eq!(
        journal::append(&retired(8)),
        Err("network-evidence-workspace")
    );
    // Pressure and unwind both retain the identical frozen region.
    let result = std::panic::catch_unwind(|| {
        let _inspection = journal::inspect().unwrap();
        panic!("interrupted export");
    });
    assert!(result.is_err());
    FREE.store(1_000_000, Ordering::Relaxed);
    assert_eq!(journal::append(&retired(8)), Ok(()));
    assert_eq!(
        journal::append(&retired(9)),
        Err(journal::capacity_refusal())
    );
    assert_eq!(flash.lock().unwrap().writes, writes);
    assert_eq!(flash.lock().unwrap().bytes, expected);
    use sha2::{Digest, Sha256};
    let expected_digest: [u8; 32] = Sha256::digest(&expected).into();
    let mut verifier = journal::snapshot::Verifier::start(journal::inspect().unwrap());
    assert!(verifier.step(&mut []).is_err());
    assert!(verifier.step(&mut [0; 512]).is_err());
    assert!(verifier.finish().is_err());
    let mut verifier = journal::snapshot::Verifier::start(journal::inspect().unwrap());
    flash.lock().unwrap().fail_read = true;
    assert!(verifier.step(&mut [0; 512]).is_err());
    assert!(verifier.step(&mut [0; 512]).is_err());
    drop(verifier);
    let mut verifier = journal::snapshot::Verifier::start(journal::inspect().unwrap());
    for n in 0..64 {
        assert_eq!(verifier.step(&mut [0; 512]).unwrap(), n == 63);
    }
    assert!(verifier.step(&mut [0; 512]).unwrap());
    let mut verified = verifier.finish().unwrap();
    assert_eq!(verified.region_identity(), [0x41; 32]);
    assert_eq!(*verified.digest(), expected_digest);
    assert_eq!(verified.committed_count(), 8);
    let mut tail = [0; 1];
    verified.read_chunk(wire::CAPACITY - 1, &mut tail).unwrap();
    assert_eq!(tail, [0x7f]);
    verified.finish_with(|verified| {
        assert_eq!(verified.region_identity(), [0x41; 32]);
        assert_eq!(*verified.digest(), expected_digest);
        assert!(journal::inspect().is_err());
        assert_eq!(
            journal::append(&retired(9)),
            Err("network-evidence-unavailable")
        );
    });
    // Non-divisor workspace exercises the final partial chunk without hashing
    // unused scratch bytes. Construction/steps retain no heap allocation.
    let (mut verifier, allocation) = allocation_probe::observe(|| {
        journal::snapshot::Verifier::start(journal::inspect().unwrap())
    });
    assert_eq!(allocation.requested_bytes, 0);
    let (_, allocation) =
        allocation_probe::observe(|| while !verifier.step(&mut [0xa5; 257]).unwrap() {});
    assert_eq!(allocation.requested_bytes, 0);
    let verified = verifier.finish().unwrap();
    assert_eq!(verified.region_identity(), [0x41; 32]);
    assert_eq!(*verified.digest(), expected_digest);
    drop(verified);
    let mut verifier = journal::snapshot::Verifier::start(journal::inspect().unwrap());
    assert!(verifier.step(&mut [0; 513]).is_err());
    assert!(verifier.finish().is_err());
    // Early finish also drops and restores the same owner.
    assert!(
        journal::snapshot::Verifier::start(journal::inspect().unwrap())
            .finish()
            .is_err()
    );
    assert!(journal::inspect().is_ok());
    assert_eq!(flash.lock().unwrap().bytes, expected);
    assert_eq!(flash.lock().unwrap().writes, writes);
    // The draft owns the verified physical snapshot through signing/ACK checks.
    let mut verifier = journal::snapshot::Verifier::start(journal::inspect().unwrap());
    while !verifier.step(&mut [0; 512]).unwrap() {}
    let body: conduit_body::BodyId =
        serde_json::from_str("\"body/network-snapshot-test\"").unwrap();
    let boot = conduit_core::BootId::new("boot/current");
    let predecessor = [0x42; 32];
    let scope = || journal::snapshot::Scope {
        body: &body,
        boot: &boot,
        epoch: 7,
        predecessor,
    };
    use ed25519_dalek::{Signature, Signer, SigningKey};
    let key = SigningKey::from_bytes(&[0x43; 32]);
    let root = key.verifying_key();
    let draft = verifier
        .finish()
        .unwrap()
        .offer(scope(), root.to_bytes())
        .unwrap();
    assert!(draft.matches_scope(scope()));
    let foreign_body = serde_json::from_str("\"body/foreign\"").unwrap();
    assert!(!draft.matches_scope(journal::snapshot::Scope {
        body: &foreign_body,
        ..scope()
    }));
    let foreign_boot = conduit_core::BootId::new("boot/foreign");
    assert!(!draft.matches_scope(journal::snapshot::Scope {
        boot: &foreign_boot,
        ..scope()
    }));
    assert!(!draft.matches_scope(journal::snapshot::Scope {
        epoch: 8,
        ..scope()
    }));
    assert!(!draft.matches_scope(journal::snapshot::Scope {
        predecessor: [0x44; 32],
        ..scope()
    }));
    assert_eq!(*draft.root(), root.to_bytes());
    let signed = draft.signed(key.sign(&draft.transcript()).to_bytes());
    assert_eq!(
        &signed[..32],
        Sha256::digest(body.as_str().as_bytes()).as_slice()
    );
    assert_eq!(&signed[32..64], &[0x41; 32]);
    assert_eq!(&signed[64..72], &7u64.to_le_bytes());
    assert_eq!(
        &signed[72..104],
        Sha256::digest(boot.as_str().as_bytes()).as_slice()
    );
    assert_eq!(&signed[104..108], &wire::CAPACITY.to_le_bytes());
    assert_eq!(signed[108], 8);
    assert_eq!(&signed[109..112], &[0; 3]);
    assert_eq!(&signed[112..144], &expected_digest);
    assert_eq!(&signed[144..176], &predecessor);
    let transcript = |bytes: &[u8; 240]| {
        let mut hash = Sha256::new();
        hash.update(b"conduit.environment/network-retirement-offer@1\0");
        hash.update(root.as_bytes());
        hash.update(&bytes[..176]);
        <[u8; 32]>::from(hash.finalize())
    };
    let signature = Signature::from_bytes(signed[176..].try_into().unwrap());
    root.verify_strict(&transcript(&signed), &signature)
        .unwrap();
    for at in 0..176 {
        let mut forged = signed;
        forged[at] ^= 1;
        assert!(root
            .verify_strict(&transcript(&forged), &signature)
            .is_err());
    }
    assert!(journal::inspect().is_err());
    assert_eq!(
        journal::append(&retired(9)),
        Err("network-evidence-unavailable")
    );
    drop(draft);
    assert!(journal::inspect().is_ok());
    assert_eq!(flash.lock().unwrap().bytes, expected);
    assert_eq!(flash.lock().unwrap().writes, writes);
    use conduit_environment_http::network_custody::{Reply, Request};
    let make_draft = || {
        let mut verifier = journal::snapshot::Verifier::start(journal::inspect().unwrap());
        while !verifier.step(&mut [0; 512]).unwrap() {}
        let draft = verifier
            .finish()
            .unwrap()
            .offer(scope(), root.to_bytes())
            .unwrap();
        let signed = draft.signed(key.sign(&draft.transcript()).to_bytes());
        (draft, signed)
    };
    let (draft, mut signed) = make_draft();
    signed[10] ^= 1;
    assert!(journal::snapshot::export::Export::new(draft, signed, 100).is_err());
    let (draft, mut signed) = make_draft();
    signed[176] ^= 1;
    assert!(journal::snapshot::export::Export::new(draft, signed, 100).is_err());
    let (draft, signed) = make_draft();
    let mut export = journal::snapshot::export::Export::new(draft, signed, 100).unwrap();
    let Reply::Offer(bytes) = export.reply(Request::Offer, 1) else {
        panic!("offer")
    };
    let offer: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(offer["profile"], "network-retirement-offer@1");
    let unhex = |text: &str| -> Vec<u8> {
        text.as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(core::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    };
    let snapshot: [u8; 32] = unhex(offer["snapshot"].as_str().unwrap())
        .try_into()
        .unwrap();
    assert_eq!(unhex(offer["offer"].as_str().unwrap()), signed);
    assert!(matches!(
        export.reply(
            Request::Chunk {
                snapshot: [0; 32],
                offset: 0
            },
            1
        ),
        Reply::Refused
    ));
    assert!(matches!(
        export.reply(
            Request::Chunk {
                snapshot,
                offset: 1
            },
            1
        ),
        Reply::Refused
    ));
    assert!(matches!(
        export.reply(
            Request::Chunk {
                snapshot,
                offset: 32768
            },
            1
        ),
        Reply::Refused
    ));
    flash.lock().unwrap().fail_read = true;
    assert!(matches!(
        export.reply(
            Request::Chunk {
                snapshot,
                offset: 0
            },
            1
        ),
        Reply::Unavailable
    ));
    let mut received = Vec::new();
    for offset in (0..32768).step_by(512) {
        let Reply::Offer(bytes) = export.reply(Request::Chunk { snapshot, offset }, 99) else {
            panic!("chunk")
        };
        assert!(bytes.len() <= journal::snapshot::export::REPLY_BYTES);
        let chunk: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(chunk["snapshot"], offer["snapshot"]);
        assert_eq!(chunk["offset"], offset);
        let data = unhex(chunk["bytes"].as_str().unwrap());
        assert_eq!(data.len(), 512);
        received.extend(data);
        let mut response = heapless::String::new();
        assert!(conduit_environment_http::network_custody::render(
            Reply::Offer(bytes),
            &mut response
        ));
        assert!(response.starts_with("HTTP/1.1 200 OK"));
    }
    assert_eq!(received, expected);
    assert!(!export.expired(99));
    assert!(export.expired(100));
    assert!(matches!(export.reply(Request::Offer, 100), Reply::Refused));
    assert!(journal::inspect().is_err());
    drop(export);
    assert!(journal::inspect().is_ok());
    assert_eq!(flash.lock().unwrap().writes, writes);
    assert_eq!(flash.lock().unwrap().bytes, expected);
    exercise_root_export();
    assert_eq!(flash.lock().unwrap().writes, writes);
    assert_eq!(flash.lock().unwrap().bytes, expected);
    println!("scratch={} bytes; retained after open/append=0; eight exact Boot records; final-slot commit retry preserved",wire::MAXIMUM_BYTES);
}
