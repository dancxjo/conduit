use conduit_kernel::{self as k, SignSink};
use conduit_network_join_call::recovery_evidence::*;
#[derive(Clone)]
struct Flash {
    bytes: Vec<u8>,
    budget: usize,
}
impl Flash {
    fn erased() -> Self {
        Self {
            bytes: vec![255; CAPACITY as usize],
            budget: usize::MAX,
        }
    }
}
impl Storage for Flash {
    fn capacity(&self) -> u32 {
        CAPACITY
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), Refusal> {
        out.copy_from_slice(&self.bytes[at as usize..at as usize + out.len()]);
        Ok(())
    }
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), Refusal> {
        for (index, byte) in bytes.iter().enumerate() {
            if self.budget == 0 {
                return Err(Refusal::Media);
            }
            self.budget -= 1;
            self.bytes[at as usize + index] &= *byte;
        }
        Ok(())
    }
}
fn scope() -> Scope<'static> {
    Scope {
        host: "esp32/001122334455",
        boot: "boot/current",
        plan: "plan/exact",
        active_play: "play/exact",
        provider_generation: 3,
        play_sequence: 7,
        configuration_revision: 2,
    }
}
fn page() -> [u8; MAXIMUM_BYTES] {
    page_at(7)
}
fn page_at(sequence: u64) -> [u8; MAXIMUM_BYTES] {
    let mut log =
        k::FixedSignLog::<128>::new((128 * core::mem::size_of::<k::KernelEvent>()) as u32).unwrap();
    for index in 0..128 {
        log.record(
            k::NodeId(index % 3),
            Some(k::PortId(0)),
            Some(k::RequestId(u32::from(index))),
            k::KernelEventKind::HostCallCompleted,
        )
        .unwrap();
    }
    let mut wire = [0; MAXIMUM_BYTES];
    let mut scope = scope();
    scope.play_sequence = sequence;
    encode(scope, log.events(), &log, &mut wire).unwrap();
    assert_eq!(
        Page::parse(&wire).unwrap().events().collect::<Vec<_>>(),
        log.events().collect::<Vec<_>>()
    );
    wire
}
fn open(storage: Flash, work: &mut [u8; MAXIMUM_BYTES]) -> Journal<Flash> {
    Journal::open(storage, work).unwrap_or_else(|_| panic!("journal open"))
}
#[test]
fn complete_event_budget_and_exact_scope_round_trip() {
    let wire = page();
    let parsed = Page::parse(&wire).unwrap();
    assert_eq!(parsed.scope(), scope());
    assert_eq!(parsed.events().count(), 128);
    const {
        assert!(MAXIMUM_BYTES <= SLOT_BYTES as usize);
        assert!(3 * MAXIMUM_BYTES <= 16384);
    }
    for at in [0, 6, 8, 24, 32, 232, 256, DATA_END] {
        let mut corrupt = wire;
        corrupt[at] ^= 1;
        assert!(Page::parse(&corrupt).is_err());
    }
}
const DATA_END: usize = MAXIMUM_BYTES - 8;
#[test]
fn every_torn_append_keeps_old_committed_evidence_and_uses_fresh_slot() {
    let wire = page();
    let mut work = [0; MAXIMUM_BYTES];
    let mut base = open(Flash::erased(), &mut work);
    base.append(&wire).unwrap();
    let base = base.into_storage();
    let successor = page_at(8);
    for budget in 0..=MAXIMUM_BYTES {
        let mut storage = base.clone();
        storage.budget = budget;
        let mut journal = open(storage, &mut work);
        let _ = journal.append(&successor);
        let mut storage = journal.into_storage();
        storage.budget = usize::MAX;
        let mut reopened = open(storage, &mut work);
        reopened.read_slot(0, &mut work).unwrap();
        assert_eq!(work, wire);
        assert!(reopened.committed_count() == 1 || reopened.committed_count() == 2);
        let commit = reopened.append(&successor).unwrap();
        assert!(commit.slot() >= 1);
    }
}
#[test]
fn full_or_corrupt_storage_never_overwrites_old_boot_records() {
    let mut work = [0; MAXIMUM_BYTES];
    let mut journal = open(Flash::erased(), &mut work);
    for slot in 0..8 {
        assert_eq!(
            journal
                .append(&page_at(u64::from(slot) + 1))
                .unwrap()
                .slot(),
            slot
        );
    }
    assert_eq!(journal.append(&page_at(9)), Err(Refusal::Capacity));
    let storage = journal.into_storage();
    let original = storage.bytes.clone();
    let mut reopened = open(storage, &mut work);
    assert_eq!(reopened.append(&page_at(9)), Err(Refusal::Capacity));
    let mut storage = reopened.into_storage();
    assert_eq!(storage.bytes, original);
    storage.bytes[32] ^= 1;
    assert!(matches!(
        Journal::open(storage, &mut work),
        Err((_, Refusal::Corrupt))
    ));
}

#[test]
fn exact_retry_reuses_committed_custody_without_programming_again() {
    let wire = page();
    let mut work = [0; MAXIMUM_BYTES];
    let mut journal = open(Flash::erased(), &mut work);
    let first = journal.append(&wire).unwrap();
    let mut storage = journal.into_storage();
    storage.budget = 0;
    let mut reopened = open(storage, &mut work);
    let retry = reopened.append(&wire).unwrap();
    assert_eq!(first, retry);
    assert_eq!(reopened.committed_count(), 1);
}
