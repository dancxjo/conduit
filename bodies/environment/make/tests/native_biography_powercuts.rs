//! Actual checkpoint/payload transition mutation cuts. This proves the physical
//! algorithm against a CPU flash model, not the entire Store or real hardware.
#![allow(dead_code)]
extern crate alloc;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_body_checkpoint.rs"]
mod checkpoint;
use checkpoint as native_body_checkpoint;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_restart_payload.rs"]
mod payload;
use checkpoint::{Checkpoint, Refusal, SourceSnapshot};
use conduit_body::{
    self as b, AdmissionManager, BodyBiographyArchiveSegment, BodyBiographyEvidence,
};
use conduit_core as c;
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
#[derive(Clone)]
struct RcCutMedia {
    bytes: Rc<RefCell<Vec<u8>>>,
    stages: Rc<Cell<usize>>,
    cut: Rc<Cell<Option<usize>>>,
    lost_commit: Rc<Cell<bool>>,
    committed: Rc<Cell<bool>>,
    reads_after: Rc<Cell<usize>>,
    read_cut: Rc<Cell<Option<usize>>>,
}
impl RcCutMedia {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes: Rc::new(RefCell::new(bytes)),
            stages: Rc::new(Cell::new(0)),
            cut: Rc::new(Cell::new(None)),
            lost_commit: Rc::new(Cell::new(false)),
            committed: Rc::new(Cell::new(false)),
            reads_after: Rc::new(Cell::new(0)),
            read_cut: Rc::new(Cell::new(None)),
        }
    }
    fn mutation(&self) -> Result<(), Refusal> {
        let n = self.stages.get();
        if self.cut.get() == Some(n) {
            return Err(Refusal::Storage);
        }
        self.stages.set(n + 1);
        Ok(())
    }
    fn disarm(&self) {
        self.cut.set(None);
        self.read_cut.set(None);
        self.lost_commit.set(false);
    }
}
impl checkpoint::Storage for RcCutMedia {
    fn capacity(&self) -> u32 {
        65536
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), Refusal> {
        if self.committed.get() {
            let n = self.reads_after.get();
            self.reads_after.set(n + 1);
            if self.read_cut.get() == Some(n) {
                self.read_cut.set(None);
                return Err(Refusal::Storage);
            }
        }
        out.copy_from_slice(
            self.bytes
                .borrow()
                .get(at as usize..at as usize + out.len())
                .ok_or(Refusal::Storage)?,
        );
        Ok(())
    }
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), Refusal> {
        assert_eq!(at % 4, 0);
        assert_eq!(bytes.len() % 4, 0);
        for (index, word) in bytes.as_chunks::<4>().0.iter().enumerate() {
            self.mutation()?;
            let start = at as usize + index * 4;
            for (old, new) in self.bytes.borrow_mut()[start..start + 4]
                .iter_mut()
                .zip(word)
            {
                assert_eq!(*old & new, *new);
                *old &= *new;
            }
        }
        if at % 32768 == 152 {
            self.committed.set(true);
            self.reads_after.set(0);
            if self.lost_commit.replace(false) {
                return Err(Refusal::Storage);
            }
        }
        Ok(())
    }
    fn erase_bank(&mut self, bank: u8) -> Result<(), Refusal> {
        for sector in 0..8 {
            self.mutation()?;
            let at = usize::from(bank) * 32768 + sector * 4096;
            self.bytes.borrow_mut()[at..at + 4096].fill(255);
        }
        Ok(())
    }
}
fn fixture() -> (Vec<BodyBiographyArchiveSegment>, BodyBiographyEvidence) {
    let workset = b::BodyWorkset::from_plots([b::ResidentPlot::new(
        "source/test".into(),
        "checked/test".into(),
    )])
    .unwrap();
    let body = b::Body::born_with_plots(workset, 1, "sign/birth".into()).unwrap();
    let membership = b::BodyMembership::new(body.body_id.clone()).unwrap();
    let evidence =
        b::BodyBiographyEvidence::born(body, membership, "Custody Fixture".into()).unwrap();
    let host = c::HostAdvertisement {
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "host/custody".into(),
        boot_id: "boot/0".into(),
        offer_generation: c::OfferGeneration(1),
        profile: "fixture/custody".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let mut body =
        conduit_environment_runtime::body_image::open_installed_image(evidence, &host).unwrap();
    let mut archives = Vec::new();
    for i in 1..=120 {
        body = b::BodyLifecycleSession::resume_here(
            body.evidence().clone(),
            &host.host_id,
            &format!("boot/{i}").into(),
        )
        .unwrap();
        if !body.pending_archives().is_empty() {
            archives.extend_from_slice(body.pending_archives());
            let head = body.pending_archives().last().unwrap().digest;
            body.acknowledge_archives(head).unwrap();
            if !archives.is_empty() {
                return (archives, body.evidence().clone());
            }
        }
    }
    panic!("fixture failed to cross the actual membership compaction boundary")
}

struct Fixture {
    original: Vec<u8>,
    evidence: BodyBiographyEvidence,
    manager: AdmissionManager,
    root: [u8; 32],
    seed: [u8; 32],
    binding: [u8; 32],
    receipt: payload::wire::Receipt,
}
fn setup() -> Fixture {
    let (archives, evidence) = fixture();
    let seed = [71; 32];
    let key = SigningKey::from_bytes(&seed);
    let root = key.verifying_key().to_bytes();
    let binding = Sha256::digest(evidence.body_id.as_str().as_bytes()).into();
    let manager = AdmissionManager::new(evidence.body_id.clone()).unwrap();
    let media = RcCutMedia::new(vec![255; 65536]);
    let mut journal = Checkpoint::open(media.clone()).unwrap();
    journal
        .save_streamed(&binding, &seed, |sink| {
            payload::stream_compressed_with_admissions(
                &evidence,
                &archives,
                &[],
                Some(&manager),
                sink,
            )
            .map_err(|_| Refusal::Corrupt)
        })
        .unwrap();
    let mut summary = payload::wire::Summary::default();
    for segment in &archives {
        summary.append(&evidence.body_id, segment).unwrap();
    }
    let mut receipt = payload::wire::Receipt {
        body: evidence.body_id.clone(),
        summary,
        offer: payload::wire::Hex([1; 32]),
        consumer: payload::wire::Consumer::Owner {
            key: payload::wire::Hex([2; 32]),
        },
        acknowledged: payload::wire::Hex([3; 32]),
        signature: payload::wire::Hex([0; 64]),
    };
    receipt.signature = payload::wire::Hex(key.sign(&receipt.transcript(&root)).to_bytes());
    let original = media.bytes.borrow().clone();
    Fixture {
        original,
        evidence,
        manager,
        root,
        seed,
        binding,
        receipt,
    }
}
fn loaded(
    journal: &mut Checkpoint<RcCutMedia>,
    f: &Fixture,
) -> (payload::archive::Resident, SourceSnapshot) {
    let mut seed = [0; 32];
    let result = journal
        .load_streamed_with_snapshot(&f.binding, &mut seed, |r| {
            payload::decode_resident_with_prefix(
                |out| r.read(out).map_err(|_| ()),
                &f.evidence.body_id,
                &f.root,
                None,
            )
            .map_err(|_| Refusal::Corrupt)
        })
        .unwrap()
        .unwrap();
    assert!(seed == f.seed);
    assert!(result.0.retained.evidence == f.evidence);
    assert!(result.0.retained.admissions.as_ref() == Some(&f.manager));
    result
}
fn rewrite(
    journal: &mut Checkpoint<RcCutMedia>,
    source: &SourceSnapshot,
    resident: &payload::archive::Resident,
    f: &Fixture,
) -> Result<SourceSnapshot, Refusal> {
    journal.rewrite_streamed(source, &f.binding, &f.seed, |io| {
        payload::stream_rewrite_with_prefix(
            &f.evidence,
            &[],
            &f.manager,
            &resident.summary,
            resident.span,
            Some(&f.receipt),
            0,
            &f.root,
            |out| io.read_source(out).map_err(|_| ()),
            &mut |bytes| io.write(bytes),
        )
        .map(|_| ())
        .map_err(|e| match e {
            payload::StreamRefusal::Sink(e) => e,
            payload::StreamRefusal::Capacity => Refusal::Capacity,
            payload::StreamRefusal::Invalid => Refusal::Corrupt,
        })
    })
}
fn reopened(bytes: Vec<u8>, f: &Fixture) -> u64 {
    let mut j = Checkpoint::open(RcCutMedia::new(bytes)).unwrap();
    let generation = j.generation().unwrap();
    let (r, _) = loaded(&mut j, f);
    match generation {
        1 => {
            assert!(r.prefix.is_none());
            assert_eq!(r.local_count, f.receipt.summary.count);
        }
        2 => {
            let p = r.prefix.as_ref().unwrap();
            assert!(p.signature == f.receipt.signature);
            assert!(p.summary == f.receipt.summary);
            assert_eq!(r.local_count, 0);
        }
        _ => panic!("unexpected generation"),
    };
    assert_eq!(r.summary.count, f.receipt.summary.count);
    generation
}
#[test]
fn every_mutation_cut_preserves_complete_chain_or_verified_prefix_and_original_owner_retry() {
    let f = setup();
    let baseline = RcCutMedia::new(f.original.clone());
    let mut j = Checkpoint::open(baseline.clone()).unwrap();
    let (r, source) = loaded(&mut j, &f);
    baseline.stages.set(0);
    rewrite(&mut j, &source, &r, &f).unwrap();
    let stages = baseline.stages.get();
    assert!(stages > 30);
    assert_eq!(reopened(baseline.bytes.borrow().clone(), &f), 2);
    for cut in 0..stages {
        let media = RcCutMedia::new(f.original.clone());
        let mut j = Checkpoint::open(media.clone()).unwrap();
        let (r, source) = loaded(&mut j, &f);
        media.stages.set(0);
        media.cut.set(Some(cut));
        assert!(
            rewrite(&mut j, &source, &r, &f).is_err(),
            "mutation cut {cut}"
        );
        let bytes = media.bytes.borrow().clone();
        assert!(
            bytes[..32768] == f.original[..32768],
            "original source changed at {cut}"
        );
        assert_eq!(reopened(bytes, &f), 1, "uncommitted cut {cut}");
        media.disarm();
        rewrite(&mut j, &source, &r, &f).unwrap();
        assert_eq!(reopened(media.bytes.borrow().clone(), &f), 2);
    }
    println!(
        "biography_mutation_cuts={stages} exact_source_and_retry=true cpu_algorithm_only=true"
    );
}
#[test]
fn applied_commit_error_and_postcommit_read_faults_keep_recoverable_custody() {
    let f = setup();
    for fault in 0..4 {
        let media = RcCutMedia::new(f.original.clone());
        let mut j = Checkpoint::open(media.clone()).unwrap();
        let (r, source) = loaded(&mut j, &f);
        media.committed.set(false);
        media.stages.set(0);
        if fault == 0 {
            media.lost_commit.set(true);
        } else {
            media.read_cut.set(Some([0, 1, 4][fault - 1]));
        }
        assert!(rewrite(&mut j, &source, &r, &f).is_err());
        media.disarm();
        assert!(media.bytes.borrow()[..32768] == f.original[..32768]);
        assert_eq!(reopened(media.bytes.borrow().clone(), &f), 2);
        assert_eq!(j.generation(), Some(1));
        rewrite(&mut j, &source, &r, &f).unwrap();
        assert_eq!(reopened(media.bytes.borrow().clone(), &f), 2);
    }
}

#[test]
fn observing_finite_checkpoint_phases_preserves_exact_flash_bytes() {
    let f = setup();
    let ordinary = RcCutMedia::new(f.original.clone());
    let observed = RcCutMedia::new(f.original.clone());
    let mut a = Checkpoint::open(ordinary.clone()).unwrap();
    let mut b = Checkpoint::open(observed.clone()).unwrap();
    let (resident_a, source_a) = loaded(&mut a, &f);
    let (resident_b, source_b) = loaded(&mut b, &f);
    rewrite(&mut a, &source_a, &resident_a, &f).unwrap();
    let mut stages = Vec::new();
    b.rewrite_streamed_observed(
        &source_b,
        &f.binding,
        &f.seed,
        |io| {
            payload::stream_rewrite_with_prefix(
                &f.evidence,
                &[],
                &f.manager,
                &resident_b.summary,
                resident_b.span,
                Some(&f.receipt),
                0,
                &f.root,
                |out| io.read_source(out).map_err(|_| ()),
                &mut |bytes| io.write(bytes),
            )
            .map(|_| ())
            .map_err(|_| Refusal::Corrupt)
        },
        &mut |stage| stages.push(stage as usize),
    )
    .unwrap();
    use checkpoint::timing::Stage;
    assert_eq!(
        stages,
        [
            Stage::SourceOne,
            Stage::Measure,
            Stage::FinishOne,
            Stage::Erase,
            Stage::Header,
            Stage::SourceTwo,
            Stage::Write,
            Stage::FinishTwo,
            Stage::Flush,
            Stage::Readback,
            Stage::Commit,
            Stage::Inspect
        ]
        .map(|s| s as usize)
    );
    assert_eq!(*ordinary.bytes.borrow(), *observed.bytes.borrow());
    assert_eq!(reopened(observed.bytes.borrow().clone(), &f), 2);
}

#[path = "native_framed_powercuts.rs"]
mod framed_powercuts;
