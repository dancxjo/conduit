//! Actual cold/boot/journal/acceptance with instrumented CPU physical media.

use super::*;
use conduit_environment_runtime::{
    display_evidence_journal::{self as evidence, Storage},
    observed_record,
};
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_observed_cold.rs"]
pub(crate) mod cold;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_observed_custody_acceptance.rs"]
pub(crate) mod custody_acceptance;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_observed_custody_boot.rs"]
mod custody_boot;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_observed_custody_journal.rs"]
pub(crate) mod custody_journal;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_local_history.rs"]
mod history;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_observed_state.rs"]
mod state;
type ArchiveRegion = Epoch;
enum State {
    Active(state::Active<Epoch>),
    FrozenRegion(Epoch),
    Pending(custody_journal::Custody<Epoch>),
    Failed,
}
static STATE: Lock<RefCell<Option<State>>> = Lock::new(RefCell::new(None));
static EPOCHS: Lock<RefCell<Option<[State; 5]>>> = Lock::new(RefCell::new(None));
const WORK_BYTES: usize = 536;
static WORK: Lock<RefCell<Option<Box<[u8; WORK_BYTES]>>>> = Lock::new(RefCell::new(None));
struct WorkLease(Option<Box<[u8; WORK_BYTES]>>);
impl Drop for WorkLease {
    fn drop(&mut self) {
        if let Some(work) = self.0.take() {
            WORK.lock(|cell| *cell.borrow_mut() = Some(work));
        }
    }
}
fn with_work<R>(
    action: impl FnOnce(&mut [u8]) -> Result<R, &'static str>,
) -> Result<R, &'static str> {
    let work = WORK
        .lock(|cell| cell.borrow_mut().take())
        .ok_or("observed-history-workspace")?;
    let mut lease = WorkLease(Some(work));
    action(&mut lease.0.as_mut().ok_or("observed-history-workspace")?[..])
}

struct Media {
    bytes: Vec<u8>,
    erases: usize,
    cut_once: bool,
    fail_ff_read: bool,
}
pub(crate) struct Epoch {
    media: Arc<StdMutex<Media>>,
    identity: [u8; 32],
}
impl Epoch {
    fn from_reclaimed(erased: custody_journal::ErasedData<Self>) -> Self {
        erased.into_storage()
    }
}
impl Storage for Epoch {
    fn capacity(&self) -> u32 {
        custody_journal::DATA as u32
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), evidence::Refusal> {
        outside_lock();
        let m = self.media.lock().unwrap();
        let end = at as usize + out.len();
        if end > custody_journal::DATA {
            return Err(evidence::Refusal::Geometry);
        }
        out.copy_from_slice(&m.bytes[at as usize..end]);
        Ok(())
    }
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), evidence::Refusal> {
        outside_lock();
        let mut m = self.media.lock().unwrap();
        let end = at as usize + bytes.len();
        if end > custody_journal::DATA {
            return Err(evidence::Refusal::Geometry);
        }
        for (a, b) in m.bytes[at as usize..end].iter_mut().zip(bytes) {
            assert_eq!(*a & b, *b);
            *a &= b;
        }
        Ok(())
    }
}
impl custody_journal::WholeEpoch for Epoch {
    fn identity(&self) -> [u8; 32] {
        self.identity
    }
    fn read(&mut self, at: usize, out: &mut [u8]) -> Result<(), custody_journal::Error> {
        outside_lock();
        let mut m = self.media.lock().unwrap();
        if at < custody_journal::DATA && m.erases == 30 && m.fail_ff_read {
            m.fail_ff_read = false;
            return Err(custody_journal::Error::Media);
        }
        let b = m
            .bytes
            .get(at..at + out.len())
            .ok_or(custody_journal::Error::Region)?;
        out.copy_from_slice(b);
        Ok(())
    }
    fn program_marker(
        &mut self,
        bank: u8,
        at: usize,
        bytes: &[u8],
    ) -> Result<(), custody_journal::Error> {
        outside_lock();
        if bank > 1 || at + bytes.len() > custody_journal::SECTOR {
            return Err(custody_journal::Error::Region);
        }
        let at = custody_journal::DATA + usize::from(bank) * custody_journal::SECTOR + at;
        let mut m = self.media.lock().unwrap();
        for (a, b) in m.bytes[at..at + bytes.len()].iter_mut().zip(bytes) {
            assert_eq!(*a & b, *b);
            *a &= b;
        }
        Ok(())
    }
    fn erase_marker(&mut self, bank: u8) -> Result<(), custody_journal::Error> {
        outside_lock();
        if bank > 1 {
            return Err(custody_journal::Error::Region);
        }
        let at = custody_journal::DATA + usize::from(bank) * custody_journal::SECTOR;
        self.media.lock().unwrap().bytes[at..at + custody_journal::SECTOR].fill(255);
        Ok(())
    }
    fn erase_data(&mut self, sector: u8) -> Result<(), custody_journal::Error> {
        outside_lock();
        if usize::from(sector) >= 30 {
            return Err(custody_journal::Error::Region);
        }
        let mut m = self.media.lock().unwrap();
        if m.cut_once {
            m.cut_once = false;
            return Err(custody_journal::Error::Media);
        }
        let at = usize::from(sector) * custody_journal::SECTOR;
        m.bytes[at..at + custody_journal::SECTOR].fill(255);
        m.erases += 1;
        Ok(())
    }
}
fn frozen(
    slot: usize,
    body: &conduit_body::BodyId,
    key: &ed25519_dalek::SigningKey,
) -> Arc<StdMutex<Media>> {
    let contract =
        conduit_environment_runtime::observation::ObservationContract::prepare().unwrap();
    let basis: &'static [u8] = Box::leak(Box::new(observed_record::descriptor(&contract).unwrap()));
    let forms = Sha256::digest(basis).into();
    let mut scope = [0; 96];
    scope[..32].copy_from_slice(&Sha256::digest(body.as_str().as_bytes()));
    scope[32..64].fill(3);
    scope[64..].fill(slot as u8);
    let media = Arc::new(StdMutex::new(Media {
        bytes: vec![255; custody_journal::TOTAL],
        erases: 0,
        cut_once: false,
        fail_ff_read: false,
    }));
    let epoch = Epoch {
        media: media.clone(),
        identity: [slot as u8; 32],
    };
    let erased = evidence::Erased::prepare_retained(epoch).ok().unwrap();
    let mut work = [0; 536];
    let mut journal =
        evidence::Journal::create_prepared(erased, &scope, basis, 960, &mut work).unwrap();
    for sequence in 1..=3 {
        let record = observed_record::ObservedRecord {
            sequence,
            observed_at_boot_ms: sequence * 100,
            disposition:
                conduit_environment_runtime::recording::RecordingDisposition::ProviderFailed,
            quantities: None,
        }
        .encode()
        .unwrap();
        let mut signed = [0; observed_record::BYTES + 64];
        signed[..observed_record::BYTES].copy_from_slice(&record);
        signed[observed_record::BYTES..]
            .copy_from_slice(&history::sign(key, &scope, &forms, &record).unwrap());
        journal.append(&signed, &mut work).unwrap();
    }
    EPOCHS.lock(|e| {
        e.borrow_mut().as_mut().unwrap()[slot - 1] = State::Active(state::Active {
            journal,
            scope,
            forms,
            last_sequence: 3,
            failed: false,
            recovered: true,
            signed: true,
            maximum_records: 960,
        })
    });
    media
}
fn request(
    slot: u8,
    credential: &conduit_body::MembershipCredential,
) -> custody_acceptance::wire::Request<'_> {
    let offer = cold::offer(slot).unwrap();
    let mut request = custody_acceptance::wire::Request {
        schema: custody_acceptance::wire::ACK,
        offer: custody_acceptance::wire::Hex(custody_acceptance::wire::offer_transcript(
            &offer,
            &native_restart::observed::pinned_scope()
                .unwrap()
                .1
                .to_bytes(),
        )),
        part: credential.part_id.as_str(),
        host: credential.host_id.as_str(),
        boot: credential.boot_id.as_str(),
        signature: custody_acceptance::wire::Hex([0; 64]),
    };
    let secret = conduit_body::SpawnInvitationSecret::from_csprng_bytes([22; 32]).unwrap();
    request.signature = custody_acceptance::wire::Hex(secret.sign(&request.transcript()));
    request
}
pub fn prove(
    body: &conduit_body::BodyId,
    key: &ed25519_dalek::SigningKey,
    credential: &conduit_body::MembershipCredential,
    body_media: &Arc<StdMutex<(Vec<u8>, bool)>>,
) {
    STATE.lock(|s| *s.borrow_mut() = Some(State::Failed));
    EPOCHS.lock(|e| *e.borrow_mut() = Some(std::array::from_fn(|_| State::Failed)));
    WORK.lock(|w| *w.borrow_mut() = Some(Box::new([0; 536])));
    let a = frozen(1, body, key);
    let good = request(1, credential);
    let mut wrong = request(1, credential);
    wrong.signature.0[0] ^= 1;
    assert!(cold::acknowledge(&wrong).is_err());
    assert_eq!(a.lock().unwrap().erases, 0);
    EPOCHS.lock(|e| assert!(matches!(&e.borrow().as_ref().unwrap()[0], State::Active(_))));
    let workspace = WORK.lock(|w| w.borrow_mut().take());
    assert!(cold::acknowledge(&good).is_err());
    EPOCHS.lock(|e| assert!(matches!(&e.borrow().as_ref().unwrap()[0], State::Active(_))));
    WORK.lock(|w| *w.borrow_mut() = workspace);
    body_media.lock().unwrap().1 = false;
    assert!(cold::acknowledge(&good).is_err());
    assert_eq!(a.lock().unwrap().erases, 0);
    EPOCHS.lock(|e| {
        assert!(matches!(
            &e.borrow().as_ref().unwrap()[0],
            State::FrozenRegion(_)
        ))
    });
    body_media.lock().unwrap().1 = true;
    let b = frozen(2, body, key);
    let good = request(2, credential);
    assert_eq!(cold::acknowledge(&good).unwrap(), 2);
    assert_eq!(b.lock().unwrap().erases, 30);
    assert!(b.lock().unwrap().bytes[..custody_journal::DATA]
        .iter()
        .all(|b| *b == 255));
    assert!(cold::acknowledge(&good).is_err());
    assert_eq!(b.lock().unwrap().erases, 30);
    let c = frozen(3, body, key);
    let good = request(3, credential);
    c.lock().unwrap().cut_once = true;
    assert!(cold::acknowledge(&good).is_err());
    EPOCHS.lock(|e| {
        assert!(matches!(
            &e.borrow().as_ref().unwrap()[2],
            State::Pending(_)
        ))
    });
    let old =
        EPOCHS.lock(|e| std::mem::replace(&mut e.borrow_mut().as_mut().unwrap()[2], State::Failed));
    drop(old);
    let reopened = Epoch {
        media: c.clone(),
        identity: [3; 32],
    };
    let mut boot_work = [0; 536];
    let custody_journal::Opened::PendingReclaim(reopened) =
        custody_boot::prepare(reopened, &mut boot_work).unwrap()
    else {
        panic!("boot must not expose pending erase as erased storage")
    };
    EPOCHS.lock(|e| e.borrow_mut().as_mut().unwrap()[2] = State::Pending(reopened));
    let workspace = WORK.lock(|w| w.borrow_mut().take());
    assert!(cold::resume_pending().is_err());
    EPOCHS.lock(|e| {
        assert!(matches!(
            &e.borrow().as_ref().unwrap()[2],
            State::Pending(_)
        ))
    });
    WORK.lock(|w| *w.borrow_mut() = workspace);
    cold::resume_pending().unwrap();
    assert_eq!(c.lock().unwrap().erases, 30);
    EPOCHS.lock(|e| {
        assert!(matches!(
            &e.borrow().as_ref().unwrap()[2],
            State::FrozenRegion(_)
        ))
    });
    // The actual boot classifier must validate durable metadata before minting erased custody.
    let State::FrozenRegion(region) =
        EPOCHS.lock(|e| std::mem::replace(&mut e.borrow_mut().as_mut().unwrap()[1], State::Failed))
    else {
        panic!()
    };
    let mut work = [0; 536];
    assert!(matches!(
        custody_boot::prepare(region, &mut work).unwrap(),
        custody_journal::Opened::Erased(_)
    ));
    let ff = frozen(5, body, key);
    let good = request(5, credential);
    ff.lock().unwrap().fail_ff_read = true;
    assert!(cold::acknowledge(&good).is_err());
    assert_eq!(ff.lock().unwrap().erases, 30);
    EPOCHS.lock(|e| {
        assert!(matches!(
            &e.borrow().as_ref().unwrap()[4],
            State::Pending(_)
        ))
    });
    {
        let m = ff.lock().unwrap();
        for bank in 0..2 {
            let start = custody_journal::DATA + bank * custody_journal::SECTOR;
            if &m.bytes[start + 444..start + 448] == b"done" {
                assert_ne!(
                    u32::from_le_bytes(m.bytes[start + 16..start + 20].try_into().unwrap()),
                    2,
                    "Reclaimed cannot publish before all-FF readback"
                )
            }
        }
    }
    cold::resume_pending().unwrap();
    assert_eq!(ff.lock().unwrap().erases, 60);
    let d = frozen(4, body, key);
    let good = request(4, credential);
    ARM_COMMIT_READ_FAULT.store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(cold::acknowledge(&good).is_err());
    assert!(
        COMMIT_READ_FAULT.load(std::sync::atomic::Ordering::SeqCst),
        "actual body commit happened before injected readback failure"
    );
    assert_eq!(d.lock().unwrap().erases, 0);
    assert!(
        d.lock().unwrap().bytes[custody_journal::DATA..]
            .iter()
            .all(|b| *b == 255),
        "no epoch marker before durable layout readback"
    );
    EPOCHS.lock(|e| {
        assert!(matches!(
            &e.borrow().as_ref().unwrap()[3],
            State::FrozenRegion(_)
        ))
    });
    COMMIT_READ_FAULT.store(false, std::sync::atomic::Ordering::SeqCst);
    println!("actual cold module + actual Root restart observed signer/persisted manager: canonical ACK, wrong signature, missing workspace, failed layout write+appliedcommit/readbackrefusal, all30erase+allFFreadbackrefusal+replay refusal, Pending authenticated reboot+resume, authenticated erased boot classify; every physical backend operation outside State/Work critical scopes");
}
