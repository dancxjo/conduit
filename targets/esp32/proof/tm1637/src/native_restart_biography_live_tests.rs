//! Actual canonical started owner; CPU media, no electrical worker proof.
use super::*;
use crate::native_body_checkpoint::Storage;
use ed25519_dalek::{Signer, SigningKey};
#[path = "../../../../../architecture/core/tests/common/sealed_state.rs"]
mod core_fixture;

fn fixture() -> (
    Store,
    Box<SealedBodyLifecycleSession>,
    conduit_core::HostAdvertisement,
    [u8; 32],
) {
    let (mut store, owner, mut host, root, _) = super::super::super::park_tests::network_fixture();
    host.boot_id = "boot/live-fixture".into();
    let mut owner = Box::new(
        SealedBodyLifecycleSession::resume_here(
            owner.unwrap().evidence().clone(),
            &host.host_id,
            &host.boot_id,
        )
        .unwrap(),
    );
    checkpoint_store(&mut store, &mut owner, &root).unwrap();
    let plans = owner
        .evidence()
        .body
        .workset
        .plots()
        .iter()
        .enumerate()
        .map(|(index, plot)| {
            let mut fragment = core_fixture::fragment();
            fragment.source_document_id = plot.source_document_id.clone();
            fragment.checked_plot_id = plot.checked_plot_id.clone();
            fragment.expanded_plot_id = alloc::format!("cpu/live/{index}").into();
            fragment.host_id = host.host_id.clone();
            fragment.boot_id = host.boot_id.clone();
            for placement in &mut fragment.placements {
                placement.host_id = host.host_id.clone();
                placement.boot_id = host.boot_id.clone();
            }
            conduit_body::BodyPlotPlan {
                plot: plot.clone(),
                plan: core_fixture::seal(fragment),
            }
        })
        .collect();
    owner.propose(plans, &host.host_id, &host.boot_id).unwrap();
    let current = owner.realization().unwrap();
    let play = conduit_body::BodyPlayIdentity::bind(&current.plan, 1);
    let sign = |n| {
        conduit_core::bind_sign(&host.host_id, &host.boot_id, Some(&play.active_play_id), n).sign_id
    };
    let wake = current
        .wake
        .body_plan_ready(&current.plan, sign(0))
        .unwrap()
        .body_play_started(&current.plan, &play, sign(1))
        .unwrap();
    owner
        .started(&host.host_id, &host.boot_id, play, wake)
        .unwrap();
    checkpoint_store(&mut store, &mut owner, &root).unwrap();
    assert!(owner
        .seal_started_membership_history_for_checkpoint()
        .unwrap());
    assert!(owner.realization().unwrap().play.is_some());
    assert!(!owner.pending_archives().is_empty());
    (store, owner, host, root)
}
fn prepare(
    store: &mut Store,
    owner: &SealedBodyLifecycleSession,
    host: &conduit_core::HostAdvertisement,
    root: &[u8; 32],
) -> Transaction<LiveScope> {
    let scope = LiveScope::prepare(store, owner, root, &host.host_id, &host.boot_id).unwrap();
    Transaction::prepare(store, scope).ok().unwrap()
}
fn receipt(transaction: &Transaction<LiveScope>) -> payload::wire::Receipt {
    let key = SigningKey::from_bytes(&[7; 32]);
    let root = key.verifying_key().to_bytes();
    let offer = transaction.offer().offer();
    let mut receipt = payload::wire::Receipt {
        body: offer.body.clone(),
        summary: offer.end.clone(),
        offer: Hex(offer.transcript(&root)),
        consumer: payload::wire::Consumer::Owner { key: Hex([9; 32]) },
        acknowledged: Hex([10; 32]),
        signature: Hex([0; 64]),
    };
    receipt.signature = Hex(key.sign(&receipt.transcript(&root)).to_bytes());
    receipt
}
// Explicit CPU-only region alias, preserving each owning fixture's fault fields.
type CpuMedia = BodyRegion;
struct Access {
    calls: usize,
    fault: Option<(usize, u8)>,
    media: Option<CpuMedia>,
}
impl Access {
    fn effect(&mut self) -> Option<u8> {
        self.calls += 1;
        self.fault
            .filter(|(n, _)| *n == self.calls)
            .map(|(_, effect)| effect)
    }
}
impl SectorStorage<BodyRegion> for Access {
    fn erase_sector(
        &mut self,
        storage: &mut BodyRegion,
        bank: u8,
        sector: u8,
    ) -> Result<(), Refusal> {
        self.media = Some(storage.clone());
        let fault = self.effect();
        let n = match fault {
            Some(0) => 0,
            Some(1) => 2048,
            _ => 4096,
        };
        let at = usize::from(bank) * 32768 + usize::from(sector) * 4096;
        storage.media.lock().unwrap().0[at..at + n].fill(255);
        if fault.is_some() {
            Err(Refusal::Storage)
        } else {
            Ok(())
        }
    }
    fn program(&mut self, storage: &mut BodyRegion, at: u32, bytes: &[u8]) -> Result<(), Refusal> {
        let fault = self.effect();
        let n = match fault {
            Some(0) => 0,
            Some(1) => bytes.len() / 2,
            _ => bytes.len(),
        };
        {
            let mut media = storage.media.lock().unwrap();
            for (old, new) in media.0[at as usize..at as usize + n].iter_mut().zip(bytes) {
                assert_eq!(*old & *new, *new);
                *old &= *new;
            }
        }
        if fault.is_some() {
            Err(Refusal::Storage)
        } else {
            Ok(())
        }
    }
    fn read(&mut self, storage: &mut BodyRegion, at: u32, out: &mut [u8]) -> Result<(), Refusal> {
        if self.effect().is_some() {
            Err(Refusal::Storage)
        } else {
            storage.read(at, out)
        }
    }
}
fn drive(
    transaction: &mut Transaction<LiveScope>,
    store: &mut Store,
    access: &mut Access,
) -> Result<(), &'static str> {
    loop {
        if transaction.needs_chunk() {
            transaction.prepare_chunk(store)?;
        }
        let calls = access.calls;
        let done = transaction.advance(store, access)?;
        assert_eq!(access.calls, calls + 1);
        if done {
            return Ok(());
        }
    }
}
fn owner_bytes(owner: &SealedBodyLifecycleSession) -> Vec<u8> {
    serde_json::to_vec(&(
        owner.evidence(),
        owner.pending_archives(),
        owner.foreground(),
    ))
    .unwrap()
}
#[test]
fn live_prefix_preserves_actual_started_owner_pending_tail_then_checkpoints_once() {
    let (mut store, mut owner, host, root) = fixture();
    let address = &*owner as *const _;
    let original = owner_bytes(&owner);
    let realization = owner.realization().unwrap().clone();
    let admissions = serde_json::to_vec(&store.admissions).unwrap();
    let custody = store.custody.as_ref().unwrap();
    let old_summary = custody.summary.clone();
    let network = *custody.network.as_ref().unwrap().bytes();
    let pending = owner.pending_archives().len() as u64;
    let generation = store.journal.generation().unwrap();
    let scope = LiveScope::prepare(&store, &owner, &root, &host.host_id, &host.boot_id).unwrap();
    assert_eq!(store.journal.generation(), Some(generation));
    let mut transaction = Transaction::prepare(&mut store, scope).ok().unwrap();
    let signed = receipt(&transaction);
    transaction.begin(&mut store, signed).unwrap();
    drive(
        &mut transaction,
        &mut store,
        &mut Access {
            calls: 0,
            fault: None,
            media: None,
        },
    )
    .unwrap();
    let scope = transaction.finish(&mut store).unwrap();
    scope.owner(&owner).unwrap();
    assert_eq!(&*owner as *const _, address);
    assert!(owner_bytes(&owner) == original && owner.realization() == Some(&realization));
    assert!(store.ready && !store.parked && store.evidence.is_none());
    assert!(store.custody.as_ref().unwrap().summary == old_summary);
    assert_eq!(store.custody.as_ref().unwrap().local_count, 0);
    assert_eq!(serde_json::to_vec(&store.admissions).unwrap(), admissions);
    assert_eq!(
        *store
            .custody
            .as_ref()
            .unwrap()
            .network
            .as_ref()
            .unwrap()
            .bytes(),
        network
    );
    assert_eq!(store.journal.generation(), Some(generation + 1));
    checkpoint_store(&mut store, &mut owner, &root).unwrap();
    assert!(owner.pending_archives().is_empty());
    assert!(owner.realization() == Some(&realization));
    let custody = store.custody.as_ref().unwrap();
    assert_eq!(custody.summary.count, old_summary.count + pending);
    assert_eq!(custody.local_count, pending);
    let mut seed = PrivateSeed([0; 32]);
    let binding = store.binding;
    let (readback, _) = decode_checkpoint(
        &mut store.journal,
        &binding,
        &owner.evidence().body_id,
        &mut seed.0,
        None,
    )
    .unwrap();
    assert!(readback.retained.evidence == *owner.evidence());
    assert_eq!(readback.local_count, pending);
    checkpoint_store(&mut store, &mut owner, &root).unwrap();
    assert_eq!(
        store.custody.as_ref().unwrap().summary.count,
        old_summary.count + pending
    );
    assert_eq!(store.custody.as_ref().unwrap().local_count, pending);
    println!("live_owner_cpu: actual_started_realization=true same_body_boot_owner=true pending_tail_preserved=true next_checkpoint_exactly_once=true native_workers_unproven=true");
}
#[test]
fn live_scope_rejects_stale_root_host_boot_source_and_owner_before_effects() {
    let (mut store, mut owner, host, root) = fixture();
    let before = owner_bytes(&owner);
    let generation = store.journal.generation();
    assert!(
        LiveScope::prepare(&store, &owner, &root, &"wrong-host".into(), &host.boot_id).is_err()
    );
    assert!(
        LiveScope::prepare(&store, &owner, &root, &host.host_id, &"wrong-boot".into()).is_err()
    );
    let wrong = LiveScope::prepare(&store, &owner, &[0; 32], &host.host_id, &host.boot_id).unwrap();
    assert!(Transaction::prepare(&mut store, wrong).is_err());
    let scope = LiveScope::prepare(&store, &owner, &root, &host.host_id, &host.boot_id).unwrap();
    let foreground = owner
        .evidence()
        .body
        .workset
        .plots()
        .iter()
        .find(|p| Some(*p) != owner.foreground())
        .unwrap()
        .clone();
    owner.select_plot(&foreground).unwrap();
    assert!(scope.owner(&owner).is_err());
    assert_eq!(store.journal.generation(), generation);
    assert!(owner_bytes(&owner) != before);
    store.ready = false;
    assert!(scope.source(&store).is_err());
    store.ready = true;
    store.parked = true;
    assert!(scope.source(&store).is_err());
    store.parked = false;
    store.binding[0] ^= 1;
    assert!(scope.source(&store).is_err());
    store.binding[0] ^= 1;
    let stale = owner.clone();
    checkpoint_store(&mut store, &mut owner, &root).unwrap();
    assert!(LiveScope::prepare(&store, &stale, &root, &host.host_id, &host.boot_id).is_err());
    let mut stale_transaction = prepare(&mut store, &owner, &host, &root);
    let signed = receipt(&stale_transaction);
    store
        .journal
        .save(&store.binding, &[7; 32], b"foreign-source")
        .unwrap();
    assert!(scope.source(&store).is_err());
    let mut out = [255; 128];
    assert!(stale_transaction
        .export_chunk(&mut store, 0, &mut out)
        .is_err());
    assert_eq!(out, [0; 128]);
    assert!(stale_transaction.begin(&mut store, signed).is_err());
    assert!(stale_transaction.cancel(&mut store).is_err());
    assert!(stale_transaction.scope().is_ok());
}
#[test]
fn live_cancel_and_nor_cuts_preserve_original_owner_and_reboot_only_complete_sources() {
    let (mut store, owner, host, root) = fixture();
    let mut transaction = prepare(&mut store, &owner, &host, &root);
    let signed = receipt(&transaction);
    transaction.begin(&mut store, signed).unwrap();
    let mut all = Access {
        calls: 0,
        fault: None,
        media: None,
    };
    drive(&mut transaction, &mut store, &mut all).unwrap();
    for cut in 1..=all.calls {
        for effect in 0..3 {
            let (mut store, owner, host, root) = fixture();
            let original = owner_bytes(&owner);
            let realization = owner.realization().unwrap().clone();
            let generation = store.journal.generation().unwrap();

            let mut transaction = prepare(&mut store, &owner, &host, &root);
            let signed = receipt(&transaction);
            transaction.begin(&mut store, signed).unwrap();
            let mut access = Access {
                calls: 0,
                fault: Some((cut, effect)),
                media: None,
            };
            assert!(drive(&mut transaction, &mut store, &mut access).is_err());
            let media = access.media.unwrap();
            let scope = transaction.cancel(&mut store).unwrap();
            scope.owner(&owner).unwrap();
            assert!(owner_bytes(&owner) == original && owner.realization() == Some(&realization));
            assert!(store.ready && !store.parked);
            assert_eq!(store.journal.generation(), Some(generation));
            // Cancellation never acknowledges the still-pending live tail.
            let old_summary = store.custody.as_ref().unwrap().summary.clone();
            let expected_admissions = serde_json::to_vec(&store.admissions).unwrap();
            let expected_network = *store
                .custody
                .as_ref()
                .unwrap()
                .network
                .as_ref()
                .unwrap()
                .bytes();
            let mut reopened = Checkpoint::open(media).unwrap();
            let reopened_generation = reopened.generation().unwrap();
            assert!(reopened_generation == generation || reopened_generation == generation + 1);
            let mut seed = PrivateSeed([0; 32]);
            let (resident, _) = decode_checkpoint(
                &mut reopened,
                &store.binding,
                &owner.evidence().body_id,
                &mut seed.0,
                None,
            )
            .unwrap();
            assert!(resident.prefix.is_some() == (reopened_generation == generation + 1));
            assert!(resident.summary == old_summary);
            assert_eq!(
                serde_json::to_vec(&resident.retained.admissions).unwrap(),
                expected_admissions
            );
            assert_eq!(
                *resident.network.as_ref().unwrap().bytes(),
                expected_network
            );
            assert!(resident.retained.evidence.membership.parts.iter().any(|p| p
                .current
                .as_ref()
                .is_some_and(|c| c.boot_id == host.boot_id && c.host_id == host.host_id)));
        }
    }
    println!("live_owner_nor_cpu: operations={} effects=3 cases={} original_owner_and_pending_tail_preserved=true reboot_old_or_complete_successor=true",all.calls,all.calls*3);
}

#[test]
#[ignore = "owns Boot-global STORE; run this exact test alone in a fresh process"]
fn owned_waitlane_seam_preserves_lease_across_finish_cancel_and_adopt_refusal() {
    use crate::native_flash_window::Gate;
    let (store, owner, host, root) = fixture();
    let actual = &*owner as *const _;
    STORE.lock(|slot| {
        assert!(matches!(*slot.borrow(), Slot::Uninitialized));
        *slot.borrow_mut() = Slot::Available(Box::leak(Box::new(store)));
    });
    let gate = Gate::new(1000, 20, 1000).unwrap();
    // Reservation is memory-only even while a CPU-model pulse owns the lane.
    let pulse = gate.begin_pulse(0, 20).unwrap();
    let (job, reserve_alloc) = crate::allocation_probe::observe(|| {
        LiveCustody::reserve(&owner, &root, &host.host_id, &host.boot_id).unwrap()
    });
    let mut job = job;
    assert!(reserve_alloc.peak_bytes <= 8192);
    assert!(take_store().is_err());
    assert!(job.offer().is_err());
    let (retained, error) = job.adopt(&owner).err().unwrap();
    assert_eq!(error, "biography-live-phase");
    job = retained;
    assert!(take_store().is_err());
    drop(pulse);
    let wait = gate.begin_wait(0, 1000).unwrap();
    let quiet = wait.try_flash(0, 1000).unwrap();
    job.prepare_source(&quiet).unwrap();
    let signed = receipt(job.transaction.as_ref().unwrap());
    job.begin(signed, &quiet).unwrap();
    let mut access = Access {
        calls: 0,
        fault: None,
        media: None,
    };
    loop {
        if job.needs_chunk() {
            job.prepare_chunk(&quiet).unwrap();
        }
        if job.advance(&mut access).unwrap() {
            break;
        }
    }
    job.finish_source(&quiet).unwrap();
    drop(quiet);
    drop(wait);
    assert!(take_store().is_err());
    let mut foreign = owner.clone();
    let other = foreign
        .evidence()
        .body
        .workset
        .plots()
        .iter()
        .find(|p| Some(*p) != foreign.foreground())
        .unwrap()
        .clone();
    foreign.select_plot(&other).unwrap();
    let (job, error) = job.adopt(&foreign).err().unwrap();
    assert_eq!(error, "biography-live-owner-scope");
    assert!(take_store().is_err());
    let (adopted, adopt_alloc) = crate::allocation_probe::observe(|| job.adopt(&owner));
    adopted.ok().unwrap();
    assert_eq!(adopt_alloc.allocations, 0);
    assert_eq!(adopt_alloc.reallocations, 0);
    assert_eq!(&*owner as *const _, actual);
    let mut lease = take_store().unwrap();
    let store = lease.0.as_mut().unwrap();
    assert!(store.ready && !store.parked);
    assert_eq!(store.custody.as_ref().unwrap().local_count, 0);
    drop(lease);
    // The same owner/lease also survives cancellation before source I/O.
    let mut job = LiveCustody::reserve(&owner, &root, &host.host_id, &host.boot_id).unwrap();
    let wait = gate.begin_wait(1001, 2001).unwrap();
    let quiet = wait.try_flash(1001, 1000).unwrap();
    job.cancel_source(&quiet).unwrap();
    drop(quiet);
    drop(wait);
    job.adopt(&owner).ok().unwrap();
    assert!(take_store().is_ok());
    println!(
        "live_owned_job_cpu: reserve_peak={} backing={} adopt_allocations=0",
        reserve_alloc.peak_bytes,
        core::mem::size_of::<LiveCustody>()
    );
    println!("live_owned_waitlane_cpu: memory_only_reserve=true actual_gate_permit=true body_borrow_not_transferred=true adopt_refusal_retains_lease=true original_owner_recovery=true native_timing=false");
}
