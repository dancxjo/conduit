//! Same incremental checkpoint core and affine Recovery owners as production.
//! CPU NOR adapter supplies no native permit/timing or Draft-admission proof.
use super::super::super::network_recovery as recovery;
use super::*;
struct Network {
    bytes: Vec<u8>,
    operations: usize,
    cut: Option<usize>,
}
impl recovery::Storage for Network {
    fn identity(&self) -> [u8; 32] {
        [13; 32]
    }
    fn capacity(&self) -> u32 {
        32768
    }
    fn erase(&mut self, permit: &recovery::RecoveryPermit, sector: u8) -> Result<(), &'static str> {
        assert!(permit.permits([13; 32], 32768, u32::from(sector) * 4096));
        self.operations += 1;
        let start = usize::from(sector) * 4096;
        if self.cut == Some(self.operations) {
            self.bytes[start..start + 2048].fill(255);
            return Err("live-test-erase-cut");
        }
        self.bytes[start..start + 4096].fill(255);
        Ok(())
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), &'static str> {
        self.operations += 1;
        if self.cut == Some(self.operations) {
            return Err("live-test-read-cut");
        }
        out.copy_from_slice(&self.bytes[at as usize..at as usize + out.len()]);
        Ok(())
    }
}
fn drive(job: &mut CheckpointJob, owner: &SealedBodyLifecycleSession) {
    let mut access = Access { fail: false };
    while !job.is_complete() {
        if job.needs_chunk() {
            job.prepare_chunk(owner).unwrap();
        }
        job.advance(&mut access).unwrap();
    }
}
#[test]
#[ignore = "owns Boot-global STORE; run this exact test alone in a fresh process"]
fn pending_blank_incremental_clean_and_every_live_media_interruption() {
    for cut in 0..=72 {
        let (owner, boot, root, cursor) = setup();
        let clean = recovery::clean_cursor(&cursor, &SigningKey::from_bytes(&[7; 32])).unwrap();
        let mut core =
            CheckpointJob::initialize(take_store().unwrap(), &owner, &boot, &root, cursor).unwrap();
        drive(&mut core, &owner);
        let pending = core.finish(&owner).unwrap();
        assert_eq!(pending.generation(), 2);
        // Production wrapper retains the exclusive lease across the phases,
        // and makes Pending adoption nonterminal until Clean publication.
        core.adopted = false;
        assert!(with_store(|_| Ok(())).is_err());
        let network = Network {
            bytes: vec![0x5a; 32768],
            operations: 0,
            cut: (cut != 0).then_some(cut),
        };
        let mut recovery =
            recovery::begin_live(core.lease.0.as_ref().unwrap(), &owner, &root, network)
                .map_err(|(_, e)| e)
                .unwrap();
        let mut work = [0; recovery::WORK];
        let mut interrupted = false;
        while !recovery.complete() {
            if recovery.step(&mut work).is_err() {
                interrupted = true;
                break;
            }
            assert!(with_store(|_| Ok(())).is_err());
        }
        if interrupted {
            assert!(recovery.finish().is_err());
            drop(core);
            let store = remove();
            assert!(store.failed && !store.ready);
            assert_eq!(
                store
                    .custody
                    .as_ref()
                    .unwrap()
                    .network
                    .as_ref()
                    .unwrap()
                    .bytes()[8],
                1
            );
            continue;
        }
        let blank = recovery.finish().ok().unwrap();
        recovery::check_live_blank(core.lease.0.as_ref().unwrap(), &owner, &root, &blank).unwrap();
        core.begin_clean(&owner, clean).unwrap();
        drive(&mut core, &owner);
        let clean_receipt = core.finish(&owner).unwrap();
        assert_eq!(clean_receipt.generation(), 3);
        assert_eq!(clean_receipt.offer(), pending.offer());
        let proof = recovery::finish_live(core.lease.0.as_ref().unwrap(), &owner, &root, blank)
            .map_err(|(_, e)| e)
            .unwrap();
        let network = proof.into_storage();
        assert_eq!(network.operations, 72);
        assert!(network.bytes.iter().all(|b| *b == 255));
        drop(core);
        let store = remove();
        assert!(store.ready && !store.failed);
        assert_eq!(
            store
                .custody
                .as_ref()
                .unwrap()
                .network
                .as_ref()
                .unwrap()
                .bytes()[8],
            2
        );
    }
}
