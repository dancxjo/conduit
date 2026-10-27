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
            out.copy_from_slice(&self.0.lock().unwrap().0[at as usize..at as usize + out.len()]);
            Ok(())
        }
        fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), Refusal> {
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
struct Counting;
fn account(add: usize, remove: usize) {
    let _ = MEM.try_with(|cell| {
        let mut m = cell.get();
        m.live += add as isize - remove as isize;
        if m.active {
            m.peak = m.peak.max(m.live);
        }
        cell.set(m);
    });
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
    ok(
        native_restart::initialize(media, retained.evidence),
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
    assert!(owner.realization().is_none());
    println!(
        "CPU64 baseline members={}/{} member_events={}/{} records={}/{} current_realization=false",
        owner.evidence().membership.parts.len(),
        owner.evidence().membership.parts.capacity(),
        owner.evidence().membership.events.len(),
        owner.evidence().membership.events.capacity(),
        owner.evidence().records.len(),
        owner.evidence().records.capacity()
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
    let (durable, admit_delta, admit_peak) = measure(|| {
        ok(
            native_body_join::admit(&mut owner, &root_key, &host, &boot, &request, 11),
            "CPU admit observer",
        )
    });
    let (_, receipt_drop, _) = measure(|| drop(durable));
    let admit_server = admit_delta + receipt_drop;
    assert!(owner.evidence().membership.parts.len() == baseline_members + 1);
    assert!(owner.realization().is_none());
    println!("CPU64 invite retained_server_delta={} peak={} admission retained_server_delta={} peak={} total_server_delta={} members={}/{} member_events={}/{} records={}/{}",invitation_server,issue_peak,admit_server,admit_peak,invitation_server+admit_server,owner.evidence().membership.parts.len(),owner.evidence().membership.parts.capacity(),owner.evidence().membership.events.len(),owner.evidence().membership.events.capacity(),owner.evidence().records.len(),owner.evidence().records.capacity());
    let (_, seal_delta, seal_peak) = measure(|| {
        ok(
            owner.seal_membership_history_for_checkpoint(),
            "post-admission history seal",
        );
        ok(
            native_restart::checkpoint(&mut owner, &root_key),
            "post-admission checkpoint",
        );
    });
    println!("CPU64 after_membership_seal delta={} peak={} total_server_delta={} members={}/{} member_events={}/{} records={}/{}",seal_delta,seal_peak,invitation_server+admit_server+seal_delta,owner.evidence().membership.parts.len(),owner.evidence().membership.parts.capacity(),owner.evidence().membership.events.len(),owner.evidence().membership.events.capacity(),owner.evidence().records.len(),owner.evidence().records.capacity());
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
