//! Opt-in copied-media proof. Fixture endorsements never leave this CPU test;
//! this grants no real receiver custody, hardware ACK, or physical timing claim.
use super::*;
// Explicit CPU-only region alias, preserving each owning fixture's fault fields.
type CpuMedia = BodyRegion;

pub(super) struct ProfileAccess {
    pub calls: usize,
    pub fault: Option<(usize, u8)>,
    pub media: Option<CpuMedia>,
}
impl ProfileAccess {
    fn fault(&mut self) -> Option<u8> {
        self.calls += 1;
        self.fault
            .filter(|(step, _)| *step == self.calls)
            .map(|(_, mode)| mode)
    }
}
impl SectorStorage<BodyRegion> for ProfileAccess {
    fn erase_sector(&mut self, s: &mut BodyRegion, bank: u8, sector: u8) -> Result<(), Refusal> {
        self.media = Some(s.clone());
        let fault = self.fault();
        let n = match fault {
            Some(0) => 0,
            Some(1) => 2048,
            _ => 4096,
        };
        let start = usize::from(bank) * 32768 + usize::from(sector) * 4096;
        s.media.lock().unwrap().0[start..start + n].fill(255);
        if fault.is_some() {
            Err(Refusal::Storage)
        } else {
            Ok(())
        }
    }
    fn program(&mut self, s: &mut BodyRegion, at: u32, bytes: &[u8]) -> Result<(), Refusal> {
        self.media = Some(s.clone());
        let fault = self.fault();
        let n = match fault {
            Some(0) => 0,
            Some(1) => (bytes.len() / 2) & !3,
            _ => bytes.len(),
        };
        let mut media = s.media.lock().unwrap();
        for (old, new) in media.0[at as usize..at as usize + n]
            .iter_mut()
            .zip(&bytes[..n])
        {
            assert_eq!(*old & new, *new);
            *old &= new;
        }
        if fault.is_some() {
            Err(Refusal::Storage)
        } else {
            Ok(())
        }
    }
    fn read(&mut self, s: &mut BodyRegion, at: u32, out: &mut [u8]) -> Result<(), Refusal> {
        self.media = Some(s.clone());
        if self.fault().is_some() {
            return Err(Refusal::Storage);
        }
        s.read(at, out)
    }
}

fn root_pin() -> [u8; 32] {
    let hex = std::env::var("CONDUIT_PRIVATE_ROOT_PUBLIC").expect("independent original Root pin");
    assert_eq!(hex.len(), 64);
    let mut bytes = [0; 32];
    for (out, pair) in bytes.iter_mut().zip(hex.as_bytes().as_chunks::<2>().0) {
        *out = u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap();
    }
    bytes
}

#[test]
#[ignore = "requires private original pinned flash, Body and producer Host; copied CPU media only"]
fn private_copied_prefix_compacts_without_boot_and_preserves_all_retained_roles() {
    let body: conduit_body::BodyId = serde_json::from_value(serde_json::Value::String(
        std::env::var("CONDUIT_PRIVATE_BODY_ID").unwrap(),
    ))
    .unwrap();
    let host: conduit_core::HostId = std::env::var("CONDUIT_PRIVATE_PRODUCER_HOST")
        .unwrap()
        .into();
    let raw = std::fs::read(std::env::var("CONDUIT_PRIVATE_BODY_BACKUP").unwrap()).unwrap();
    assert_eq!(raw.len(), 4194304);
    let root = root_pin();
    let binding = Sha256::digest(body.as_str().as_bytes()).into();
    let mut region = crate::native_evidence_region::test_empty();
    region.program(0, &raw[0x3f0000..0x400000]).unwrap();
    let mut journal = Checkpoint::open(region).unwrap();
    let input_generation = journal.generation().unwrap();
    let mut seed = PrivateSeed([0; 32]);
    let (resident, source) =
        decode_checkpoint(&mut journal, &binding, &body, &mut seed.0, None).unwrap();
    assert!(SigningKey::from_bytes(&seed.0).verifying_key().to_bytes() == root);
    cold_migration_and_next_boot(&raw, &body, &root, &host);
    let evidence = resident.retained.evidence;
    let original_admissions = serde_json::to_vec(&resident.retained.admissions).unwrap();
    let original_layouts = serde_json::to_vec(&resident.layouts).unwrap();
    let original_network = resident.network.as_ref().map(|c| *c.bytes());
    let mut store = Store {
        journal,
        binding,
        seed: Some(seed.0),
        evidence: Some(evidence.clone()),
        archives: Vec::new(),
        custody: Some(ArchiveCustody {
            proof: resident.proof,
            layouts: resident.layouts,
            network: resident.network,
            source,
            summary: resident.summary,
            prefix: resident.prefix,
            local_count: resident.local_count,
            span: resident.span,
        }),
        admissions: resident.retained.admissions,
        parked: false,
        pristine: false,
        failed: false,
        ready: false,
    };
    // A disk envelope is not a fabricated live parked owner: the historical
    // input can contain an unreconciled Wake. Reconcile a COPIED next Boot and
    // use the actual canonical park entrance to mint the fixture's token.
    // That setup is excluded from the subsequent no-new-Boot transaction.
    let boot = "boot/copied-hot-custody-setup".into();
    let mut owner = SealedBodyLifecycleSession::resume_here(evidence, &host, &boot).unwrap();
    while owner.seal_terminal_wake_for_checkpoint().unwrap() {
        checkpoint_store(&mut store, &mut owner, &root).unwrap();
    }
    owner.seal_membership_history_for_checkpoint().unwrap();
    checkpoint_store(&mut store, &mut owner, &root).unwrap();
    let evidence = owner.evidence().clone();
    let mut owner = Some(Box::new(owner));
    let token = park_store(&mut store, &mut owner, &root, &host, &boot).unwrap();
    assert!(owner.is_none());
    let generation = token.generation;
    let before_bytes = store.journal.payload_length(&binding).unwrap().unwrap();
    let local = store.custody.as_ref().unwrap().local_count;
    let summary = store.custody.as_ref().unwrap().summary.clone();
    println!("private_fixture_setup_only: input_generation={input_generation} copied_next_boot_parked_generation={generation} hardware_boots=0");
    // Setup/oracle DTOs above are deliberately outside the incremental runtime
    // measurement. The production operation only receives the actual token.
    let ((mut transaction, planned), prepare_alloc) = crate::allocation_probe::observe(|| {
        let mut transaction = Transaction::prepare(&mut store, token).ok().unwrap();
        let key = SigningKey::from_bytes(&seed.0);
        let mut receipt = payload::wire::Receipt {
            body: body.clone(),
            summary: transaction.offered.offer.end.clone(),
            offer: Hex(transaction.offered.offer.transcript(&root)),
            consumer: payload::wire::Consumer::Owner { key: Hex([9; 32]) },
            acknowledged: Hex([10; 32]),
            signature: Hex([0; 64]),
        };
        receipt.signature = Hex(key.sign(&receipt.transcript(&root)).to_bytes());
        transaction.begin(&mut store, receipt).unwrap();
        let planned = transaction
            .recipe
            .as_ref()
            .unwrap()
            .planned_bytes()
            .unwrap();
        (transaction, planned)
    });
    assert_eq!(store.journal.generation(), Some(generation));
    assert!(planned < before_bytes && planned <= payload::MAX_BYTES);
    let mut access = ProfileAccess {
        calls: 0,
        fault: None,
        media: None,
    };
    let (mut token, write_alloc) = crate::allocation_probe::observe(|| {
        loop {
            if transaction.writer.as_ref().unwrap().needs_chunk() {
                transaction.prepare_chunk(&mut store).unwrap();
            }
            let calls = access.calls;
            let done = transaction.advance(&mut store, &mut access).unwrap();
            assert_eq!(access.calls, calls + 1);
            if done {
                break;
            }
        }
        transaction.finish(&mut store).unwrap()
    });
    assert_eq!(
        write_alloc.allocations, 0,
        "no hidden runtime allocation in chunk replay, physical steps or adoption"
    );
    assert_eq!(write_alloc.reallocations, 0);
    assert!(
        prepare_alloc.peak_bytes + core::mem::size_of::<Custody>() <= 8192,
        "finite additional preparation/backing bound"
    );
    assert_eq!(token.generation, generation + 1);
    assert!(token.host == host && token.boot == boot && token.key == root);
    let (readback, _) =
        decode_checkpoint(&mut store.journal, &binding, &body, &mut seed.0, None).unwrap();
    assert!(readback.retained.evidence == evidence);
    assert_eq!(
        serde_json::to_vec(&readback.retained.admissions).unwrap(),
        original_admissions
    );
    assert_eq!(
        serde_json::to_vec(&readback.layouts).unwrap(),
        original_layouts
    );
    assert!(readback.network.as_ref().map(|c| *c.bytes()) == original_network);
    assert!(readback.summary == summary && readback.local_count == 0);
    let restored = restore_store(&mut store, &mut token, &host, &boot, &root).unwrap();
    assert!(restored.evidence() == &evidence);
    println!("copied_private_hot_custody: generation={generation}->{} bytes={before_bytes}->{planned} local={local}->0 total_archives={} preparation_peak={} job_backing={} runtime_allocations={} operations={} same_boot_body_root_admissions_clean_layout=true fixture_receipt_only=true",generation+1,summary.count,prepare_alloc.peak_bytes,core::mem::size_of::<Custody>(),write_alloc.allocations,access.calls);
}

/// The separate first-migration fallback uses the UNCHANGED cold implementation.
/// It is an attended CPU fixture, not a claim that its workers keep running.
fn cold_migration_and_next_boot(
    raw: &[u8],
    body: &conduit_body::BodyId,
    root: &[u8; 32],
    host: &conduit_core::HostId,
) {
    let binding = Sha256::digest(body.as_str().as_bytes()).into();
    let mut region = crate::native_evidence_region::test_empty();
    region.program(0, &raw[0x3f0000..0x400000]).unwrap();
    let mut journal = Checkpoint::open(region).unwrap();
    let generation = journal.generation().unwrap();
    let before = journal.payload_length(&binding).unwrap().unwrap();
    let mut seed = PrivateSeed([0; 32]);
    let (resident, source) =
        decode_checkpoint(&mut journal, &binding, body, &mut seed.0, None).unwrap();
    let evidence = resident.retained.evidence;
    let admissions = serde_json::to_vec(&resident.retained.admissions).unwrap();
    let network = resident.network.as_ref().map(|c| *c.bytes());
    let layouts = serde_json::to_vec(&resident.layouts).unwrap();
    let summary = resident.summary.clone();
    let mut store = Store {
        journal,
        binding,
        seed: Some(seed.0),
        evidence: Some(evidence.clone()),
        archives: Vec::new(),
        custody: Some(ArchiveCustody {
            proof: resident.proof,
            layouts: resident.layouts,
            network: resident.network,
            source,
            summary: resident.summary,
            prefix: resident.prefix,
            local_count: resident.local_count,
            span: resident.span,
        }),
        admissions: resident.retained.admissions,
        parked: false,
        pristine: false,
        failed: false,
        ready: false,
    };
    let export = prepare_store(&mut store).unwrap();
    let key = SigningKey::from_bytes(&seed.0);
    let mut receipt = payload::wire::Receipt {
        body: body.clone(),
        summary: export.offer.end.clone(),
        offer: Hex(export.offer.transcript(root)),
        consumer: payload::wire::Consumer::Owner { key: Hex([9; 32]) },
        acknowledged: Hex([10; 32]),
        signature: Hex([0; 64]),
    };
    receipt.signature = Hex(key.sign(&receipt.transcript(root)).to_bytes());
    super::super::super::retain_store_receipt(&mut store, receipt).unwrap();
    let compacted = store.journal.payload_length(&binding).unwrap().unwrap();
    assert!(compacted < before);
    assert_eq!(store.journal.generation(), Some(generation + 1));
    let (after, _) =
        decode_checkpoint(&mut store.journal, &binding, body, &mut seed.0, None).unwrap();
    assert!(
        after.retained.evidence == evidence && after.summary == summary && after.local_count == 0
    );
    assert_eq!(
        serde_json::to_vec(&after.retained.admissions).unwrap(),
        admissions
    );
    assert!(after.network.as_ref().map(|c| *c.bytes()) == network);
    assert_eq!(serde_json::to_vec(&after.layouts).unwrap(), layouts);
    let boot = "boot/copied-cold-migration-next-boot".into();
    let mut owner = SealedBodyLifecycleSession::resume_here(evidence, host, &boot).unwrap();
    while owner.seal_terminal_wake_for_checkpoint().unwrap() {
        checkpoint_store(&mut store, &mut owner, root).unwrap();
    }
    owner.seal_membership_history_for_checkpoint().unwrap();
    checkpoint_store(&mut store, &mut owner, root).unwrap();
    let next = store.journal.payload_length(&binding).unwrap().unwrap();
    assert!(next <= payload::MAX_BYTES);
    let (after, _) =
        decode_checkpoint(&mut store.journal, &binding, body, &mut seed.0, None).unwrap();
    assert!(after.network.as_ref().map(|c| *c.bytes()) == network);
    assert_eq!(serde_json::to_vec(&after.layouts).unwrap(), layouts);
    println!("copied_cold_first_migration: generation={generation}->{} before={before} compacted={compacted} next_boot={next} capacity={} same_body_root_admissions_clean_layout=true fixture_receipt_only=true no_hardware=true",generation+1,payload::MAX_BYTES);
}

#[test]
fn job_allocation_refusal_returns_owner_before_acquiring_any_store() {
    let (mut store, token, host, root, evidence, _) = fixture();
    let generation = token.generation;
    let gate = conduit_monotonic_wait::flash_window::Gate::new(1000, 20, 1000).unwrap();
    let wait = gate.begin_wait(0, 1000).unwrap();
    let quiet = wait.try_flash(0, 1000).unwrap();
    let guard =
        crate::allocation_probe::refuse_next_allocation_size(core::mem::size_of::<Custody>());
    let (mut token, error) = Custody::prepare(token, &quiet).err().unwrap();
    drop(guard);
    assert_eq!(error, "biography-hot-job-capacity");
    let owner = restore_store(&mut store, &mut token, &host.host_id, &host.boot_id, &root).unwrap();
    assert_eq!(owner.evidence(), &evidence);
    assert_eq!(store.journal.generation(), Some(generation));
}

/// Replays only the real, independently pinned committed source. No fixture
/// receipt, synthetic receiver, offload or production Host identity is minted.
#[test]
#[ignore = "requires private pinned post-custody flash; copied CPU next-Boot only"]
fn private_committed_prefix_survives_next_boot_without_new_custody() {
    let body: conduit_body::BodyId = serde_json::from_value(serde_json::Value::String(
        std::env::var("CONDUIT_PRIVATE_BODY_ID").unwrap(),
    ))
    .unwrap();
    let host: conduit_core::HostId = std::env::var("CONDUIT_PRIVATE_PRODUCER_HOST")
        .unwrap()
        .into();
    let raw = std::fs::read(std::env::var("CONDUIT_PRIVATE_BODY_BACKUP").unwrap()).unwrap();
    assert_eq!(raw.len(), 4194304);
    let root = root_pin();
    let binding = Sha256::digest(body.as_str().as_bytes()).into();
    let mut region = crate::native_evidence_region::test_empty();
    region.program(0, &raw[0x3f0000..0x400000]).unwrap();
    let mut journal = Checkpoint::open(region).unwrap();
    let generation = journal.generation().unwrap();
    let before = journal.payload_length(&binding).unwrap().unwrap();
    let mut seed = PrivateSeed([0; 32]);
    let (resident, source) =
        decode_checkpoint(&mut journal, &binding, &body, &mut seed.0, None).unwrap();
    assert!(SigningKey::from_bytes(&seed.0).verifying_key().to_bytes() == root);
    let prefix = serde_json::to_vec(&resident.prefix).unwrap();
    assert!(
        resident.prefix.is_some(),
        "requires actual committed external custody"
    );
    let network = resident.network.as_ref().map(|c| *c.bytes());
    let layouts = serde_json::to_vec(&resident.layouts).unwrap();
    let summary = resident.summary.clone();
    let evidence = resident.retained.evidence;
    let mut admissions = resident.retained.admissions.unwrap();
    admissions.retire_unfinished_challenges();
    crate::native_restart::compact_admission_storage(&mut admissions);
    let admissions_bytes = serde_json::to_vec(&admissions).unwrap();
    let mut store = Store {
        journal,
        binding,
        seed: Some(seed.0),
        evidence: Some(evidence.clone()),
        archives: Vec::new(),
        custody: Some(ArchiveCustody {
            proof: resident.proof,
            layouts: resident.layouts,
            network: resident.network,
            source,
            summary: resident.summary,
            prefix: resident.prefix,
            local_count: resident.local_count,
            span: resident.span,
        }),
        admissions: Some(admissions),
        parked: false,
        pristine: false,
        failed: false,
        ready: false,
    };
    let boot: conduit_core::BootId = "boot/copied-real-prefix-next-boot-001".into();
    assert_eq!(boot.as_str().len(), 37);
    let mut owner = SealedBodyLifecycleSession::resume_here(evidence, &host, &boot).unwrap();
    while owner.seal_terminal_wake_for_checkpoint().unwrap() {
        checkpoint_store(&mut store, &mut owner, &root).unwrap();
    }
    owner.seal_membership_history_for_checkpoint().unwrap();
    checkpoint_store(&mut store, &mut owner, &root).unwrap();
    let next_bytes = store.journal.payload_length(&binding).unwrap().unwrap();
    assert!(next_bytes <= payload::MAX_BYTES);
    let (after, _) =
        decode_checkpoint(&mut store.journal, &binding, &body, &mut seed.0, None).unwrap();
    assert!(SigningKey::from_bytes(&seed.0).verifying_key().to_bytes() == root);
    assert!(after.retained.evidence == *owner.evidence());
    assert!(after.retained.evidence.body_id == body);
    assert!(serde_json::to_vec(&after.prefix).unwrap() == prefix);
    assert!(serde_json::to_vec(&after.retained.admissions).unwrap() == admissions_bytes);
    assert!(after.network.as_ref().map(|c| *c.bytes()) == network);
    assert!(serde_json::to_vec(&after.layouts).unwrap() == layouts);
    assert!(after.summary.count >= summary.count);
    println!("private_actual_prefix_next_boot: generation={generation}->{} bytes={before}->{next_bytes} capacity={} original_prefix_receipt_preserved=true fixture_receipts=0 hardware_boots=0", store.journal.generation().unwrap(), payload::MAX_BYTES);
}

#[path = "native_restart_biography_hot_private_membership_tests.rs"]
mod owned_membership;
