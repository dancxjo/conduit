//! Actual Root custody/join producer on simulated flash; no hardware or public
//! authenticated Line is established by this CPU proof.
//! Opt in with THERMO_PRIVATE_FLASH pointing to a private complete 4MiB capture,
//! then run this exact ignored test. Only counts and allocation sizes are printed.
// Embedded profile flags do not exist in this CPU fixture crate.
#![allow(unexpected_cfgs, dead_code, clippy::duplicate_mod)]
extern crate alloc;
#[allow(dead_code)]
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_body_checkpoint.rs"]
#[allow(dead_code)] // Compatibility byte-slice API is covered by native_restart tests.
mod native_body_checkpoint;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_body_join.rs"]
mod native_body_join;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_joined_host_wire.rs"]
mod native_joined_host_wire;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_restart.rs"]
#[allow(dead_code)] // Legacy envelope helpers are covered by native_restart tests.
mod native_restart;
mod native_evidence_region {
    use crate::native_body_checkpoint::{Refusal, Storage};
    use std::sync::{Arc, Mutex};
    #[derive(Clone)]
    pub struct BodyRegion(pub Arc<Mutex<(Vec<u8>, bool)>>);
    pub fn test_empty() -> BodyRegion {
        BodyRegion(Arc::new(Mutex::new((vec![255; 65536], true))))
    }
    impl Storage for BodyRegion {
        fn capacity(&self) -> u32 {
            65536
        }
        fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), Refusal> {
            if !at.is_multiple_of(4)
                || !out.len().is_multiple_of(4)
                || at as usize + out.len() > 65536
            {
                eprintln!("CPU BodyRegion read geometry at={at} len={}", out.len());
                return Err(Refusal::Geometry);
            }
            out.copy_from_slice(&self.0.lock().unwrap().0[at as usize..at as usize + out.len()]);
            Ok(())
        }
        fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), Refusal> {
            if !at.is_multiple_of(4)
                || !bytes.len().is_multiple_of(4)
                || at as usize + bytes.len() > 65536
            {
                eprintln!(
                    "CPU BodyRegion program geometry at={at} len={}",
                    bytes.len()
                );
                return Err(Refusal::Geometry);
            }
            let mut state = self.0.lock().unwrap();
            if !state.1 {
                return Err(Refusal::Storage);
            }
            for (old, new) in state.0[at as usize..at as usize + bytes.len()]
                .iter_mut()
                .zip(bytes)
            {
                assert!(*old & new == *new, "NOR transition");
                *old &= new;
            }
            Ok(())
        }
        fn erase_bank(&mut self, bank: u8) -> Result<(), Refusal> {
            let mut state = self.0.lock().unwrap();
            if !state.1 {
                return Err(Refusal::Storage);
            }
            state.0[usize::from(bank) * 32768..(usize::from(bank) + 1) * 32768].fill(255);
            Ok(())
        }
    }
}
// Representative Core fragments prove active-owner custody without electrical
// providers. Their metadata is retained unchanged, never cloned by admission.
#[path = "../../../../architecture/core/tests/common/sealed_state.rs"]
mod core_fixture;
use conduit_core as c;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
#[derive(Clone, Copy)]
struct Memory {
    live: isize,
    peak: isize,
    active: bool,
}
thread_local! { static MEM:Cell<Memory>=const{Cell::new(Memory{live:0,peak:0,active:false})}; }
#[derive(Clone, Copy)]
struct Phases {
    enabled: bool,
    index: usize,
    baseline: isize,
    starts: [isize; 3],
    peaks: [isize; 3],
}
thread_local! { static PHASES:Cell<Phases>=const{Cell::new(Phases{enabled:false,index:0,baseline:0,starts:[0;3],peaks:[0;3]})}; }
fn phase_start() {
    let live = MEM.with(|m| m.get().live);
    PHASES.with(|p| {
        p.set(Phases {
            enabled: true,
            index: 0,
            baseline: live,
            starts: [live; 3],
            peaks: [live; 3],
        })
    });
}
fn phase_next(index: usize) {
    let live = MEM.with(|m| m.get().live);
    PHASES.with(|p| {
        let mut state = p.get();
        state.index = index;
        state.starts[index] = live;
        state.peaks[index] = live;
        p.set(state);
    });
}
fn phase_finish() -> [(isize, isize); 3] {
    PHASES.with(|p| {
        let mut state = p.get();
        state.enabled = false;
        p.set(state);
        core::array::from_fn(|i| {
            (
                state.peaks[i] - state.baseline,
                state.peaks[i] - state.starts[i],
            )
        })
    })
}
struct Counting;
fn account(add: usize, remove: usize) {
    let live = MEM.try_with(|cell| {
        let mut m = cell.get();
        m.live += add as isize - remove as isize;
        if m.active {
            m.peak = m.peak.max(m.live);
        }
        cell.set(m);
        m.live
    });
    if let Ok(live) = live {
        let _ = PHASES.try_with(|cell| {
            let mut phase = cell.get();
            if phase.enabled {
                phase.peaks[phase.index] = phase.peaks[phase.index].max(live);
                cell.set(phase);
            }
        });
    }
}
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            account(l.size(), 0)
        }
        p
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc_zeroed(l) };
        if !p.is_null() {
            account(l.size(), 0)
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        account(0, l.size());
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let q = unsafe { System.realloc(p, l, n) };
        if !q.is_null() {
            account(n, l.size())
        }
        q
    }
}
#[global_allocator]
static ALLOC: Counting = Counting;
fn measure<T>(f: impl FnOnce() -> T) -> (T, isize, isize) {
    let before = MEM.with(|s| {
        let mut m = s.get();
        assert!(!m.active);
        m.active = true;
        m.peak = m.live;
        s.set(m);
        m.live
    });
    let value = f();
    let (delta, peak) = MEM.with(|s| {
        let mut m = s.get();
        m.active = false;
        s.set(m);
        (m.live - before, m.peak - before)
    });
    (value, delta, peak)
}
fn ok<T, E>(v: Result<T, E>, label: &str) -> T {
    match v {
        Ok(v) => v,
        Err(_) => panic!("{label}"),
    }
}
#[allow(dead_code)]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_restart_compress.rs"]
mod codec;
#[test]
#[ignore = "private CPU flash-copy resident-RAM audit; not an actual member admission"]
fn observer_resident_ram_audit() {
    use sha2::{Digest, Sha256};
    let flash = ok(
        std::fs::read(ok(std::env::var("THERMO_PRIVATE_FLASH"), "private path")),
        "private flash",
    );
    let region = flash[0x3f0000..0x400000].to_vec();
    let which = if u64::from_le_bytes(region[8..16].try_into().unwrap())
        > u64::from_le_bytes(region[32776..32784].try_into().unwrap())
    {
        0
    } else {
        32768
    };
    let binding: [u8; 32] = region[which + 24..which + 56].try_into().unwrap();
    let media = native_evidence_region::BodyRegion(std::sync::Arc::new(std::sync::Mutex::new((
        region, true,
    ))));
    let mut journal = ok(
        native_body_checkpoint::Checkpoint::open(media.clone()),
        "private committed journal",
    );
    let length = ok(journal.payload_length(&binding), "private binding").expect("payload");
    let mut encoded = vec![0; length];
    let mut seed = [0; 32];
    ok(
        journal.load(&binding, &mut seed, &mut encoded),
        "private payload",
    );
    drop(journal);
    let mut at = 0;
    let mut decoder = codec::Decoder::new(
        |out: &mut [u8; 128]| {
            let n = out.len().min(encoded.len() - at);
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
        let n = ok(decoder.read(&mut chunk), "private compression");
        if n == 0 {
            break;
        }
        plain.extend_from_slice(&chunk[..n]);
    }
    drop(decoder);
    println!(
        "CPU private plain_bytes={} encoded_bytes={}",
        plain.len(),
        encoded.len()
    );
    let value: serde_json::Value = ok(serde_json::from_slice(&plain), "private envelope");
    let body_id = ok(
        serde_json::from_value(value["evidence"]["body_id"].clone()),
        "private Body ID",
    );
    drop(value);
    plain.fill(0);
    drop(plain);
    let retained = ok(
        native_restart::payload::decode(&encoded, &body_id),
        "private canonical biography",
    );
    drop(encoded);
    println!("CPU64 decoded_absolute_live={}", MEM.with(|m| m.get().live));
    ok(
        native_restart::initialize(media.clone(), retained.evidence),
        "private store initialize",
    );
    drop(retained.archives);
    drop(retained.admissions);
    let selected = ok(
        native_restart::select_signer_seed([99; 32]),
        "retained signer",
    );
    assert!(selected == seed, "Root continuity");
    seed.fill(0);
    let root_key = ed25519_dalek::SigningKey::from_bytes(&selected)
        .verifying_key()
        .to_bytes();
    let retained = ok(native_restart::take_body(), "take biography");
    let prior = retained
        .evidence
        .membership
        .parts
        .iter()
        .find_map(|p| p.current.as_ref())
        .expect("Root current member");
    let host = prior.host_id.clone();
    let boot: c::BootId = format!(
        "cpu/{}",
        "n".repeat(prior.boot_id.as_str().len().saturating_sub(4))
    )
    .into();
    let mut owner = ok(
        conduit_body::SealedBodyLifecycleSession::resume_here(retained.evidence, &host, &boot),
        "canonical CPU restart",
    );
    while ok(
        owner.seal_terminal_wake_for_checkpoint(),
        "cold Wake sealing",
    ) {
        ok(
            native_restart::checkpoint(&mut owner, &root_key),
            "cold Wake checkpoint",
        );
    }
    ok(
        owner.seal_membership_history_for_checkpoint(),
        "cold membership seal",
    );
    ok(
        native_restart::checkpoint(&mut owner, &root_key),
        "cold checkpoint",
    );
    if std::env::var_os("THERMO_STARTUP_ONLY").is_some() {
        let state = media.0.lock().unwrap();
        let lengths = [0usize, 32768]
            .map(|at| u32::from_le_bytes(state.0[at + 16..at + 20].try_into().unwrap()));
        println!(
            "CPU private startup checkpoint success generation={:?} encoded_lengths={:?}",
            native_restart::generation(),
            lengths
        );
        return;
    }
    let active = std::env::var_os("THERMO_AUDIT_ACTIVE").is_some();
    if active {
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
                fragment.expanded_plot_id = format!("cpu/active/{}", index).into();
                fragment.host_id = host.clone();
                fragment.boot_id = boot.clone();
                for placement in &mut fragment.placements {
                    placement.host_id = host.clone();
                    placement.boot_id = boot.clone();
                }
                conduit_body::BodyPlotPlan {
                    plot: plot.clone(),
                    plan: core_fixture::seal(fragment),
                }
            })
            .collect();
        ok(owner.propose(plans, &host, &boot), "CPU active proposal");
        let current = owner.realization().unwrap();
        let play = conduit_body::BodyPlayIdentity::bind(&current.plan, 1);
        let sign = |n| c::bind_sign(&host, &boot, Some(&play.active_play_id), n).sign_id;
        let wake = ok(
            current.wake.body_plan_ready(&current.plan, sign(0)),
            "CPU plan ready",
        );
        let wake = ok(
            wake.body_play_started(&current.plan, &play, sign(1)),
            "CPU started Wake",
        );
        ok(
            owner.started(&host, &boot, play, wake),
            "CPU actual started owner",
        );
        ok(
            native_restart::checkpoint(&mut owner, &root_key),
            "CPU active baseline checkpoint",
        );
    }
    let realization_digest: [u8; 32] =
        Sha256::digest(format!("{:?}", owner.realization()).as_bytes()).into();
    let foreground_digest: [u8; 32] =
        Sha256::digest(serde_json::to_vec(&owner.foreground()).unwrap()).into();
    println!(
        "CPU64 active_owner={} representative_Core_fragments={}",
        active, active
    );
    println!(
        "CPU64 baseline members={}/{} member_events={}/{} records={}/{}",
        owner.evidence().membership.parts.len(),
        owner.evidence().membership.parts.capacity(),
        owner.evidence().membership.events.len(),
        owner.evidence().membership.events.capacity(),
        owner.evidence().records.len(),
        owner.evidence().records.capacity()
    );
    println!(
        "CPU64 baseline_absolute_live={}",
        MEM.with(|m| m.get().live)
    );
    resident_components("baseline", &owner);
    let baseline_members = owner.evidence().membership.parts.len();
    let secret = ok(
        conduit_body::SpawnInvitationSecret::from_csprng_bytes([11; 32]),
        "CPU private entropy",
    );
    let (bootstrap, issue_delta, issue_peak) = measure(|| {
        ok(
            native_body_join::issue(
                &mut owner,
                &root_key,
                &host,
                &boot,
                secret.clone(),
                [12; 32],
                10,
                100,
            ),
            "CPU issue invitation",
        )
    });
    let observer = c::HostAdvertisement {
        declared_name: None,
        protocol_version: c::PROTOCOL_VERSION,
        host_id: format!("linux/{}", "a".repeat(32)).into(),
        boot_id: format!("boot/{}", "b".repeat(36)).into(),
        offer_generation: c::OfferGeneration(1),
        profile: "conduit/environment/history-observer@1".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let claim = &bootstrap.invitation.claim;
    let request = conduit_body::PortableSpawnAdmissionRequest {
        schema: conduit_body::SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
        invitation_id: claim.invitation_id.clone(),
        body_id: claim.body_id.clone(),
        host_advertisement: observer,
        nonce: claim.nonce,
        signature: secret
            .sign(&claim.signing_transcript(
                &format!("linux/{}", "a".repeat(32)).into(),
                &format!("boot/{}", "b".repeat(36)).into(),
                c::OfferGeneration(1),
            ))
            .to_vec(),
        membership_admitted: false,
        plan_created: false,
        play_created: false,
    };
    let (_, bootstrap_drop, _) = measure(|| drop(bootstrap));
    let invitation_server = issue_delta + bootstrap_drop;
    let phases_enabled = active && std::env::var_os("THERMO_AUDIT_PHASES").is_some();
    let mut phases = [(0, 0); 3];
    let (_, admit_server, admit_peak) = measure(|| {
        if phases_enabled {
            // The exact native transaction, manager and active private owner;
            // stage once inside its closure solely to mark allocator phases.
            // The transaction's subsequent ordinary auto-seal is an empty no-op.
            phase_start();
            let proof = ok(request.admission_proof(), "phase admission proof");
            let credential = ok(
                native_restart::admissions_transaction(&mut owner, &root_key, |owner, manager| {
                    let credential = owner
                        .admit_invited_host(
                            manager,
                            &request.host_advertisement,
                            &proof,
                            11,
                            &host,
                            &boot,
                        )
                        .map_err(|_| "phase-admission")?;
                    phase_next(1);
                    owner
                        .seal_started_membership_history_for_checkpoint()
                        .map_err(|_| "phase-seal")?;
                    phase_next(2);
                    Ok(credential)
                }),
                "phase exact canonical transaction",
            );
            phases = phase_finish();
            drop(credential);
        } else {
            let durable = ok(
                native_body_join::admit(&mut owner, &root_key, &host, &boot, &request, 11),
                "CPU admit observer",
            );
            drop(durable);
        }
    });
    if phases_enabled {
        println!("CPU64 phase_probe=true admission_peak_over_operation={} admission_extra_over_phase_entry={} raw_seal_peak_over_operation={} raw_seal_extra_over_phase_entry={} checkpoint_peak_over_operation={} checkpoint_extra_over_phase_entry={}", phases[0].0,phases[0].1,phases[1].0,phases[1].1,phases[2].0,phases[2].1);
    }
    assert!(owner.evidence().membership.parts.len() == baseline_members + 1);
    assert!(owner.realization().is_some() == active);
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(
            format!("{:?}", owner.realization()).as_bytes()
        )),
        realization_digest
    );
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(
            serde_json::to_vec(&owner.foreground()).unwrap()
        )),
        foreground_digest
    );
    assert!(
        owner.pending_archives().is_empty(),
        "only durable ACK releases archive custody"
    );
    println!("CPU64 invite retained_server_delta={} peak={} admission retained_server_delta={} peak={} total_server_delta={} members={}/{} member_events={}/{} records={}/{}",invitation_server,issue_peak,admit_server,admit_peak,invitation_server+admit_server,owner.evidence().membership.parts.len(),owner.evidence().membership.parts.capacity(),owner.evidence().membership.events.len(),owner.evidence().membership.events.capacity(),owner.evidence().records.len(),owner.evidence().records.capacity());
    let (_, seal_delta, seal_peak) = measure(|| {
        ok(
            if active {
                owner.seal_started_membership_history_for_checkpoint()
            } else {
                owner.seal_membership_history_for_checkpoint()
            },
            "post-admission history seal",
        );
        ok(
            native_restart::checkpoint(&mut owner, &root_key),
            "post-admission checkpoint",
        );
    });
    println!("CPU64 after_membership_seal delta={} peak={} total_server_delta={} members={}/{} member_events={}/{} records={}/{}",seal_delta,seal_peak,invitation_server+admit_server+seal_delta,owner.evidence().membership.parts.len(),owner.evidence().membership.parts.capacity(),owner.evidence().membership.events.len(),owner.evidence().membership.events.capacity(),owner.evidence().records.len(),owner.evidence().records.capacity());
    println!(
        "CPU64 sealed_observer_absolute_live={}",
        MEM.with(|m| m.get().live)
    );
    resident_components("sealed_observer", &owner);
    let (_, check_delta, check_peak) = measure(|| {
        ok(
            native_restart::checkpoint(&mut owner, &root_key),
            "repeat checkpoint",
        )
    });
    println!(
        "CPU64 repeat_checkpoint delta={} peak={} receiver_advertisement_empty=true",
        check_delta, check_peak
    );
    let body_binding: [u8; 32] =
        Sha256::digest(owner.evidence().body_id.as_str().as_bytes()).into();
    assert!(body_binding == binding);
    // Reopen actual authenticated checkpoint bytes and verify exact canonical
    // evidence/admission wire plus consumed-invitation replay after restart.
    let expected_evidence = ok(
        serde_json::to_vec(owner.evidence()),
        "expected evidence wire",
    );
    let expected_admissions = ok(
        native_restart::with_admissions(|m| serde_json::to_vec(m).map_err(|_| "admission wire")),
        "expected admissions wire",
    );
    let mut reopened = ok(
        native_body_checkpoint::Checkpoint::open(media.clone()),
        "checkpoint reopen",
    );
    let length =
        ok(reopened.payload_length(&binding), "reopen binding").expect("committed checkpoint");
    let mut encoded = vec![0; length];
    let mut reopened_seed = [0; 32];
    ok(
        reopened.load(&binding, &mut reopened_seed, &mut encoded),
        "reopened payload",
    );
    assert!(reopened_seed == selected, "Root seed continuity");
    reopened_seed.fill(0);
    let retained = ok(
        native_restart::payload::decode(&encoded, &owner.evidence().body_id),
        "reopened canonical payload",
    );
    assert!(
        ok(
            serde_json::to_vec(&retained.evidence),
            "reopened evidence wire"
        ) == expected_evidence,
        "exact evidence wire"
    );
    if active {
        assert!(matches!(
            conduit_body::SealedBodyLifecycleSession::open(retained.evidence.clone()),
            Err(conduit_body::BodyLifecycleSessionError::UnreconciledWake)
        ));
        let next_boot: c::BootId = "cpu/audit-next-boot".into();
        let resumed = ok(
            conduit_body::SealedBodyLifecycleSession::resume_here(
                retained.evidence.clone(),
                &host,
                &next_boot,
            ),
            "active newBoot restart",
        );
        assert!(resumed.realization().is_none());
        assert!(matches!(
            resumed.evidence().body.state,
            conduit_body::BodyState::Lulled
        ));
    }
    let mut admissions = retained.admissions.expect("retained admissions");
    assert!(
        ok(serde_json::to_vec(&admissions), "reopened admissions wire") == expected_admissions,
        "exact admission wire"
    );
    let mut membership = retained.evidence.membership;
    let signs = conduit_body::AdmissionSigns {
        part_admitted: c::bind_sign(&host, &boot, None, 100).sign_id,
        host_attached: c::bind_sign(&host, &boot, None, 101).sign_id,
        candidate_admitted: c::bind_sign(&host, &boot, None, 102).sign_id,
    };
    let replay = admissions.complete_spawn(
        &mut membership,
        &request.host_advertisement,
        &request.admission_proof().unwrap(),
        12,
        signs,
    );
    assert!(
        matches!(replay, Err(conduit_body::AdmissionRefusal::Replay)),
        "replay after checkpoint reopen"
    );
    println!("CPU64 checkpoint_reopened=true exact_evidence_wire=true exact_admission_wire=true consumed_invitation_replay_refused=true Root_seed_continuity=true");
    if active {
        // A second, valid admission stages real archive custody, then storage
        // fails. No receipt/ACK escapes, and the previous complete bank survives.
        let secret = conduit_body::SpawnInvitationSecret::from_csprng_bytes([31; 32]).unwrap();
        let invitation = native_body_join::issue(
            &mut owner,
            &root_key,
            &host,
            &boot,
            secret.clone(),
            [32; 32],
            20,
            100,
        )
        .unwrap();
        let mut request = request.clone();
        request.host_advertisement.host_id = "cpu/second-observer".into();
        request.host_advertisement.boot_id = "cpu/second-observer-boot".into();
        let claim = invitation.invitation.claim;
        request.invitation_id = claim.invitation_id.clone();
        request.body_id = claim.body_id.clone();
        request.nonce = claim.nonce;
        request.signature = secret
            .sign(&claim.signing_transcript(
                &request.host_advertisement.host_id,
                &request.host_advertisement.boot_id,
                request.host_advertisement.offer_generation,
            ))
            .to_vec();
        media.0.lock().unwrap().1 = false;
        assert!(
            native_body_join::admit(&mut owner, &root_key, &host, &boot, &request, 21).is_err()
        );
        assert!(!native_restart::publication_ready());
        assert_eq!(
            owner.pending_archives().len(),
            1,
            "failed write cannot ACK moved history"
        );
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(
                format!("{:?}", owner.realization()).as_bytes()
            )),
            realization_digest
        );
        let mut previous = native_body_checkpoint::Checkpoint::open(media).unwrap();
        let mut old_seed = [0; 32];
        let mut old_payload = vec![0; previous.payload_length(&binding).unwrap().unwrap()];
        previous
            .load(&binding, &mut old_seed, &mut old_payload)
            .unwrap();
        assert!(old_seed == selected);
        old_seed.fill(0);
        let previous =
            native_restart::payload::decode(&old_payload, &owner.evidence().body_id).unwrap();
        assert_eq!(
            previous.evidence.membership.parts.len(),
            baseline_members + 1
        );
        assert_eq!(
            owner.evidence().membership.parts.len(),
            baseline_members + 2
        );
        println!("CPU64 active_failed_checkpoint_fenced=true pending_archive_retained=true previous_committed_bank_preserved=true current_Play_unchanged=true");
    }
}

// Exact owned clone allocation isolates metadata payload from spare capacities.
// These CPU64 counts do not claim Xtensa allocation sizes.
fn clone_bytes<T: Clone>(value: &T) -> isize {
    let (copy, bytes, _) = measure(|| value.clone());
    drop(copy);
    bytes
}
fn resident_components(label: &str, owner: &conduit_body::SealedBodyLifecycleSession) {
    let e = owner.evidence();
    let parts = clone_bytes(&e.membership.parts);
    let checkpoint = clone_bytes(&e.membership.checkpoint);
    let body = clone_bytes(&e.body);
    let records = clone_bytes(&e.records);
    let admissions = ok(
        native_restart::with_admissions(|m| Ok(clone_bytes(m))),
        "private component audit",
    );
    println!("CPU64 components label={label} parts_clone_bytes={parts} checkpoint_clone_bytes={checkpoint} body_clone_bytes={body} records_clone_bytes={records} admissions_clone_bytes={admissions}");
}
#[test]
fn immutable_body_id_unique_and_clone_allocation_cost() {
    let text = format!("body/{}", "a".repeat(64));
    let wire = serde_json::to_vec(&text).unwrap();
    let (plain, plain_heap, _) = measure(|| text.clone());
    let (identity, shared_heap, _) =
        measure(|| serde_json::from_slice::<conduit_body::BodyId>(&wire).unwrap());
    let (cloned, clone_heap, _) = measure(|| identity.clone());
    assert_eq!(identity, cloned);
    assert_eq!(clone_heap, 0);
    let original_total = plain_heap + core::mem::size_of::<String>() as isize;
    let shared_total = shared_heap + core::mem::size_of::<conduit_body::BodyId>() as isize;
    println!("CPU64 unique_id_text_bytes={} String_total={} shared_id_total={} unique_overhead={} clone_heap={} decoded_independently=true",text.len(),original_total,shared_total,shared_total-original_total,clone_heap);
    drop((plain, identity, cloned));
}
