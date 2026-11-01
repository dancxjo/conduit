//! Actual canonical owner and incremental writer on CPU media. No scheduler
//! timing, physical continuity, or installed membership acceptance is claimed.
use super::*;
use crate::native_body_checkpoint::Storage;
struct Access {
    fail: bool,
}
impl SectorStorage<BodyRegion> for Access {
    fn erase_sector(&mut self, s: &mut BodyRegion, bank: u8, sector: u8) -> Result<(), Refusal> {
        if self.fail {
            return Err(Refusal::Storage);
        }
        // CPU BodyRegion exposes whole-bank erase only; physical sector/cut
        // behavior is covered by the existing incremental NOR fixture.
        if sector == 0 {
            s.erase_bank(bank)?;
        }
        Ok(())
    }
    fn program(&mut self, s: &mut BodyRegion, at: u32, b: &[u8]) -> Result<(), Refusal> {
        if self.fail {
            Err(Refusal::Storage)
        } else {
            s.program(at, b)
        }
    }
    fn read(&mut self, s: &mut BodyRegion, at: u32, b: &mut [u8]) -> Result<(), Refusal> {
        if self.fail {
            Err(Refusal::Storage)
        } else {
            s.read(at, b)
        }
    }
}
fn setup() -> (Box<SealedBodyLifecycleSession>, [u8; 32]) {
    let (mut store, mut owner, _, root) = super::super::park_tests::fixture();
    checkpoint_store(&mut store, owner.as_mut().unwrap(), &root).unwrap();
    STORE.lock(|slot| {
        assert!(matches!(*slot.borrow(), Slot::Uninitialized));
        *slot.borrow_mut() = Slot::Available(Box::leak(Box::new(store)));
    });
    (owner.unwrap(), root)
}
fn remove() -> &'static mut Store {
    STORE.lock(
        |slot| match core::mem::replace(&mut *slot.borrow_mut(), Slot::Uninitialized) {
            Slot::Available(s) => s,
            _ => panic!("Store still leased"),
        },
    )
}
#[test]
#[ignore = "owns Boot-global STORE; run this exact test alone in a fresh process"]
fn owned_checkpoint_withholds_output_preserves_identity_and_fences_failure() {
    use crate::native_flash_window::Gate;
    let (owner, root) = setup();
    let identity = &*owner as *const _;
    let job = Job::<u64>::reserve(owner, root).ok().unwrap();
    assert!(take_store().is_err());
    let owner = job.release_unstaged().ok().unwrap();
    assert_eq!(&*owner as *const _, identity);
    let original_generation = generation().unwrap();
    let mut job = Job::<u64>::reserve(owner, root).ok().unwrap();
    let authority = job
        .owner()
        .evidence()
        .membership
        .parts
        .iter()
        .find_map(|p| p.current.as_ref())
        .unwrap();
    let authority_host = authority.host_id.clone();
    let authority_boot = authority.boot_id.clone();
    job.stage(|owner, manager| {
        owner
            .issue_invitation(
                manager,
                conduit_body::SpawnInvitationSecret::from_csprng_bytes([61; 32]).unwrap(),
                [62; 32],
                1,
                10000,
                &authority_host,
                &authority_boot,
            )
            .map_err(|_| "test-invitation")?;
        Ok(73)
    })
    .unwrap();
    let expected_admissions =
        serde_json::to_vec(job.lease.0.as_ref().unwrap().admissions.as_ref().unwrap()).unwrap();
    assert_eq!(job.phase(), Phase::Prepare);
    job = job.adopt().err().unwrap(); // output remains private
    assert!(take_store().is_err());
    let gate = Gate::new(1000, 20, 1000).unwrap();
    let wait = gate.begin_wait(0, 1000).unwrap();
    let permit = wait.try_flash(0, 1000).unwrap();
    let mut access = Access { fail: false };
    drop(permit);
    struct Scheduled(Box<Job<u64>>);
    impl crate::native_flash_jobs::Job for Scheduled {
        fn step(&self) -> u32 {
            self.0.step()
        }
    }
    let mut mailbox = crate::native_flash_jobs::Mailbox::<Scheduled, (), &'static str>::new();
    let (mut submitter, mut executor) = mailbox.split().unwrap();
    let mut last = job.step();
    loop {
        // 1000ms is CPU fixture input, not qualified preparation timing.
        let id = submitter.submit(Scheduled(job), 1000).ok().unwrap();
        executor
            .execute(&wait, 0, |scheduled, permit| {
                let job = &mut scheduled.0;
                match job.phase() {
                    Phase::Prepare => job.prepare(permit),
                    Phase::Capture => job.capture(permit),
                    Phase::Chunk => job.prepare_chunk(permit),
                    Phase::Physical(_) => job.advance(&mut access),
                    Phase::Finish => job.finish_source(permit),
                    _ => Err("test-unexpected-phase"),
                }
            })
            .unwrap();
        let completed = submitter.take(id).unwrap();
        assert!(completed.result.is_ok());
        assert_eq!(
            completed.progress,
            crate::native_flash_jobs::Progress::AdvancedOne
        );
        job = completed.owner.0;
        assert_eq!(job.step(), last + 1);
        last = job.step();
        assert_eq!(job.owner() as *const _, identity);
        if job.phase() == Phase::Complete {
            break;
        }
    }
    let permit = wait.try_flash(0, 1000).unwrap();
    assert!(job.finish_source(&permit).is_err());
    assert_eq!(job.step(), last);
    assert!(take_store().is_err());
    let (owner, value) = job.adopt().ok().unwrap();
    assert_eq!(value, 73);
    assert_eq!(&*owner as *const _, identity);
    assert_eq!(generation(), Some(original_generation + 1));
    assert!(publication_ready());
    let store = remove();
    let mut seed = PrivateSeed([0; 32]);
    let (resident, _) = decode_checkpoint(
        &mut store.journal,
        &store.binding,
        &owner.evidence().body_id,
        &mut seed.0,
        None,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_vec(resident.retained.admissions.as_ref().unwrap()).unwrap(),
        expected_admissions
    );
    let (owner, root) = setup();
    let identity = &*owner as *const _;
    let mut job = Job::<u64>::reserve(owner, root).ok().unwrap();
    job.stage(|_, _| Ok(91)).unwrap();
    job.prepare(&permit).unwrap();
    assert!(job.advance(&mut Access { fail: true }).is_err());
    assert_eq!(job.phase(), Phase::Failed);
    assert_eq!(job.owner() as *const _, identity);
    job = job.adopt().err().unwrap();
    assert!(take_store().is_err());
    drop(job); // fail-closed misuse: never publishes the staged mutation
    assert!(!publication_ready());
    let _ = remove();
    let (owner, root) = setup();
    let mut job = Job::<u64>::reserve(owner, root).ok().unwrap();
    assert_eq!(
        job.stage(|_, _| Err("staged-refusal")),
        Err("staged-refusal")
    );
    assert_eq!(job.phase(), Phase::Failed);
    assert!(job.release_unstaged().is_err());
    assert!(!publication_ready());
    let _ = remove();
}
