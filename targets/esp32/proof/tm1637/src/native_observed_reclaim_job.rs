//! Affine bridge only: no SDK adapter or qualified physical timing is installed.
use crate::{
    native_flash_jobs::Job,
    native_flash_window::FlashPermit,
    native_observed_archive::custody_journal::{
        ErasedData, Error, PendingReclaim, PreparedReclaim, ReclaimStep, WholeEpoch, WORK,
    },
};
use alloc::boxed::Box;
/// Only previously authenticated closed custody or an already durable pending
/// reclaim can enter this job. Initial metadata commit also requires a permit.
pub(crate) struct ReclaimJob<S> {
    state: Option<State<S>>,
    // A resumed job starts at its data step; a newly prepared job first commits
    // Reclaiming. Both expose strictly consecutive, nonwrapping step ordinals.
    offset: u32,
}
/// Lazily allocated by Root after workspace admission, not replicated in every
/// dormant mailbox slot. The private option permits a consuming commit without
/// allocating inside the wait callback, including on failure.
pub(crate) struct PreparedStart<S> {
    prepared: Option<PreparedReclaim<S>>,
}
impl<S> PreparedStart<S> {
    pub(crate) fn new(prepared: PreparedReclaim<S>) -> Self {
        Self {
            prepared: Some(prepared),
        }
    }
}
enum State<S> {
    Prepared(Box<PreparedStart<S>>),
    Pending(PendingReclaim<S>),
    Erased(ErasedData<S>),
}
impl<S: WholeEpoch> ReclaimJob<S> {
    pub(crate) fn new(pending: PendingReclaim<S>) -> Self {
        Self {
            state: Some(State::Pending(pending)),
            offset: 0,
        }
    }
    pub(crate) fn from_prepared(prepared: Box<PreparedStart<S>>) -> Self {
        Self {
            state: Some(State::Prepared(prepared)),
            offset: 1,
        }
    }
    /// The scoped permit is necessary, not proof that this operation's duration
    /// has been independently qualified. One call performs one journal step.
    pub(crate) fn advance(
        &mut self,
        _permit: &FlashPermit<'_, '_>,
        work: &mut [u8; WORK],
    ) -> Result<(), Error> {
        match self.state.as_mut() {
            Some(State::Prepared(_)) => {
                let Some(State::Prepared(prepared)) = self.state.take() else {
                    unreachable!()
                };
                let mut slot = prepared;
                let prepared = slot.prepared.take().expect("actual prepared owner");
                match prepared.commit(work) {
                    Ok(pending) => {
                        self.state = Some(State::Pending(pending));
                        Ok(())
                    }
                    Err((prepared, error)) => {
                        slot.prepared = Some(prepared);
                        self.state = Some(State::Prepared(slot));
                        Err(error)
                    }
                }
            }
            Some(State::Pending(pending)) if pending.next_step() != ReclaimStep::Publish => {
                pending.step()
            }
            Some(State::Pending(_)) => {
                let Some(State::Pending(pending)) = self.state.take() else {
                    unreachable!()
                };
                match pending.publish(work) {
                    Ok(erased) => {
                        self.state = Some(State::Erased(erased));
                        Ok(())
                    }
                    Err((pending, error)) => {
                        self.state = Some(State::Pending(pending));
                        Err(error)
                    }
                }
            }
            _ => Err(Error::Phase),
        }
    }
    #[allow(clippy::result_large_err)]
    pub(crate) fn into_erased(self) -> Result<ErasedData<S>, Self> {
        match self {
            Self {
                state: Some(State::Erased(erased)),
                ..
            } => Ok(erased),
            other => Err(other),
        }
    }
}
impl<S: WholeEpoch> Job for ReclaimJob<S> {
    fn step(&self) -> u32 {
        match self.state.as_ref() {
            Some(State::Prepared(_)) => 0,
            Some(State::Pending(pending)) => match pending.next_step() {
                ReclaimStep::EraseSector(sector) => self.offset + u32::from(sector),
                ReclaimStep::VerifySector(sector) => self.offset + 30 + u32::from(sector),
                ReclaimStep::Publish => self.offset + 60,
            },
            Some(State::Erased(_)) => self.offset + 61,
            None => u32::MAX, // A panicked consuming publish cannot be resubmitted.
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        native_flash_window::Gate,
        native_observed_archive::custody_journal::{Custody, ACCEPTANCE, DATA, SECTOR, TOTAL},
    };
    use std::{cell::Cell, rc::Rc, vec, vec::Vec};
    struct Media {
        bytes: Vec<u8>,
        fail: Rc<Cell<bool>>,
        erases: Rc<Cell<u32>>,
        metadata_ops: Rc<Cell<u32>>,
    }
    impl WholeEpoch for Media {
        fn identity(&self) -> [u8; 32] {
            [7; 32]
        }
        fn read(&mut self, at: usize, out: &mut [u8]) -> Result<(), Error> {
            out.copy_from_slice(&self.bytes[at..at + out.len()]);
            Ok(())
        }
        fn program_marker(&mut self, bank: u8, at: usize, bytes: &[u8]) -> Result<(), Error> {
            self.metadata_ops.set(self.metadata_ops.get() + 1);
            let at = DATA + usize::from(bank) * SECTOR + at;
            self.bytes[at..at + bytes.len()].copy_from_slice(bytes);
            Ok(())
        }
        fn erase_marker(&mut self, bank: u8) -> Result<(), Error> {
            self.metadata_ops.set(self.metadata_ops.get() + 1);
            if self.fail.get() {
                return Err(Error::Media);
            }
            let at = DATA + usize::from(bank) * SECTOR;
            self.bytes[at..at + SECTOR].fill(255);
            Ok(())
        }
        fn erase_data(&mut self, sector: u8) -> Result<(), Error> {
            if self.fail.get() {
                return Err(Error::Media);
            }
            self.erases.set(self.erases.get() + 1);
            let at = usize::from(sector) * SECTOR;
            self.bytes[at..at + SECTOR].fill(255);
            Ok(())
        }
    }
    #[test]
    fn actual_staged_owner_survives_step_and_publish_failures_until_consumed() {
        let fail = Rc::new(Cell::new(false));
        let erases = Rc::new(Cell::new(0));
        let media = Media {
            bytes: vec![255; TOTAL],
            fail: fail.clone(),
            erases: erases.clone(),
            metadata_ops: Rc::new(Cell::new(0)),
        };
        let mut work = [0; WORK];
        let custody = Custody::open(media, &mut work, |_| true).ok().unwrap();
        let mut acceptance = [0; ACCEPTANCE];
        acceptance[..32].fill(7);
        let pending = custody
            .begin_staged(&acceptance, &mut work, |_| true, |_, _| Ok(()))
            .ok()
            .unwrap();
        let job = ReclaimJob::new(pending);
        let mut job = job.into_erased().err().unwrap();
        let gate = Gate::new(100, 100, 100).unwrap();
        let wait = gate.begin_wait(1, 100).unwrap();
        let permit = wait.try_flash(2, 90).unwrap();
        fail.set(true);
        assert_eq!(job.advance(&permit, &mut work), Err(Error::Media));
        assert_eq!(job.step(), 0);
        fail.set(false);
        for ordinal in 0..60 {
            assert_eq!(job.step(), ordinal);
            job.advance(&permit, &mut work).unwrap();
            assert_eq!(job.step(), ordinal + 1);
        }
        assert_eq!(erases.get(), 30);
        fail.set(true);
        assert_eq!(job.advance(&permit, &mut work), Err(Error::Media));
        assert_eq!(job.step(), 60);
        let mut job = job.into_erased().err().unwrap();
        fail.set(false);
        job.advance(&permit, &mut work).unwrap();
        assert_eq!(job.step(), 61);
        assert_eq!(job.advance(&permit, &mut work), Err(Error::Phase));
        let storage = job.into_erased().ok().unwrap().into_storage();
        assert!(storage.bytes[..DATA].iter().all(|b| *b == 255));
        assert_eq!(erases.get(), 30);
    }
    #[test]
    fn initial_commit_is_a_permitted_step_and_preparation_has_no_metadata_effects() {
        let fail = Rc::new(Cell::new(false));
        let erases = Rc::new(Cell::new(0));
        let metadata_ops = Rc::new(Cell::new(0));
        let media = Media {
            bytes: vec![255; TOTAL],
            fail: fail.clone(),
            erases: erases.clone(),
            metadata_ops: metadata_ops.clone(),
        };
        let mut work = [0; WORK];
        let custody = Custody::open(media, &mut work, |_| true).ok().unwrap();
        let mut acceptance = [0; ACCEPTANCE];
        acceptance[..32].fill(7);
        let closed = Cell::new(0);
        let prepared = custody
            .prepare_begin(
                &acceptance,
                |_| true,
                |_, _| {
                    closed.set(closed.get() + 1);
                    Ok(())
                },
            )
            .ok()
            .unwrap();
        assert_eq!(closed.get(), 1);
        assert_eq!(metadata_ops.get(), 0);
        let mut job = ReclaimJob::from_prepared(Box::new(PreparedStart::new(prepared)));
        assert_eq!(job.step(), 0);
        job = job.into_erased().err().unwrap();
        let gate = Gate::new(100, 100, 100).unwrap();
        let wait = gate.begin_wait(1, 100).unwrap();
        let permit = wait.try_flash(2, 90).unwrap();
        fail.set(true);
        assert_eq!(job.advance(&permit, &mut work), Err(Error::Media));
        assert_eq!(job.step(), 0);
        assert_eq!(erases.get(), 0);
        fail.set(false);
        for ordinal in 0..62 {
            assert_eq!(job.step(), ordinal);
            job.advance(&permit, &mut work).unwrap();
            assert_eq!(job.step(), ordinal + 1);
            if ordinal == 0 {
                assert_eq!(erases.get(), 0);
            }
        }
        assert_eq!(
            closed.get(),
            1,
            "closed journal inspection does not run inside flash window"
        );
        assert_eq!(erases.get(), 30);
        let storage = job.into_erased().ok().unwrap().into_storage();
        assert!(storage.bytes[..DATA].iter().all(|b| *b == 255));
    }
    #[test]
    fn rejected_or_cancelled_preparation_never_creates_pending_or_erased_custody() {
        let metadata_ops = Rc::new(Cell::new(0));
        let media = Media {
            bytes: vec![255; TOTAL],
            fail: Rc::new(Cell::new(false)),
            erases: Rc::new(Cell::new(0)),
            metadata_ops: metadata_ops.clone(),
        };
        let mut work = [0; WORK];
        let custody = Custody::open(media, &mut work, |_| true).ok().unwrap();
        let mut acceptance = [0; ACCEPTANCE];
        acceptance[..32].fill(7);
        let (custody, error) = custody
            .prepare_begin(
                &acceptance,
                |_| false,
                |_, _| panic!("authentication must precede closure"),
            )
            .err()
            .unwrap();
        assert_eq!(error, Error::Authentication);
        let (custody, error) = custody
            .prepare_begin(&acceptance, |_| true, |_, _| Err(Error::NotClosed))
            .err()
            .unwrap();
        assert_eq!(error, Error::NotClosed);
        let prepared = custody
            .prepare_begin(&acceptance, |_| true, |_, _| Ok(()))
            .ok()
            .unwrap();
        let custody = prepared.into_custody();
        assert!(matches!(
            custody.classify(),
            Ok(crate::native_observed_archive::custody_journal::Opened::Retained(_))
        ));
        assert_eq!(metadata_ops.get(), 0);
    }
}
