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
mod journal;
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
    println!("scratch={} bytes; retained after open/append=0; eight exact Boot records; final-slot commit retry preserved",wire::MAXIMUM_BYTES);
}
