//! One preadmitted transaction backing spans Pending, qualified blanking and
//! incremental Clean. The actual Store lease and closed physical owner never
//! escape between phases; no reboot or duplicated Body evidence is required.
use super::*;
use crate::native_play::journal::{self, snapshot::Draft, Inspection};
use crate::native_restart::network_recovery::{
    self as recovery, Blank, Recovery, RecoveryPermit, Step, WORK,
};
pub(crate) trait Access: SectorStorage<BodyRegion> {
    fn erase_network(
        &mut self,
        owner: &mut Inspection,
        authority: &RecoveryPermit,
        sector: u8,
    ) -> Result<(), &'static str>;
    fn read_network(
        &mut self,
        owner: &mut Inspection,
        at: u32,
        out: &mut [u8],
    ) -> Result<(), &'static str>;
}
enum Stage {
    Pending(Option<Draft>),
    Erasing(Recovery<Inspection>),
    Cleaning(Blank<Inspection>),
    Done,
    Failed,
}
pub(crate) struct PendingJob {
    core: CheckpointJob,
    stage: Stage,
    clean: Option<payload::network::Cursor>,
    work: [u8; WORK],
    sequence: u32,
    completed: Option<CommittedCustody>,
    entered_live: bool,
    finished: bool,
}
impl PendingJob {
    pub(super) fn new(core: CheckpointJob, draft: Draft, clean: payload::network::Cursor) -> Self {
        Self {
            core,
            stage: Stage::Pending(Some(draft)),
            clean: Some(clean),
            work: [0; WORK],
            sequence: 0,
            completed: None,
            entered_live: false,
            finished: false,
        }
    }
    pub(crate) fn operation(&self) -> Option<Operation> {
        match &self.stage {
            Stage::Pending(_) | Stage::Cleaning(_) => self.core.operation(),
            Stage::Erasing(job) => job.next_step().map(|step| match step {
                Step::Erase(_) => Operation::EraseSector,
                Step::Read(_) => Operation::ReadbackChunk,
            }),
            _ => None,
        }
    }
    pub(crate) fn step(&self) -> u32 {
        self.sequence
    }
    pub(crate) fn needs_chunk(&self) -> bool {
        match &self.stage {
            Stage::Pending(_) | Stage::Cleaning(_) => {
                self.core.is_complete() || self.core.needs_chunk()
            }
            Stage::Erasing(job) => job.complete(),
            _ => false,
        }
    }
    pub(crate) fn is_complete(&self) -> bool {
        matches!(self.stage, Stage::Done)
    }
    pub(crate) fn prepare_chunk(
        &mut self,
        owner: &SealedBodyLifecycleSession,
    ) -> Result<(), &'static str> {
        let result = self.prepare(owner);
        if result.is_err() {
            self.stage = Stage::Failed;
            self.core.poisoned = true;
        }
        result
    }
    fn prepare(&mut self, owner: &SealedBodyLifecycleSession) -> Result<(), &'static str> {
        match &mut self.stage {
            Stage::Pending(draft) if self.core.is_complete() => {
                let receipt = self.core.finish(owner)?;
                // Pending is durable now. Cancellation must remain fenced even
                // before the first data erase or Clean checkpoint operation.
                self.entered_live = true;
                self.core.adopted = false;
                let draft = draft.take().ok_or("network-live-snapshot")?;
                let inspection = draft
                    .into_closed(&receipt)
                    .map_err(|_| "network-live-snapshot")?;
                let store = self.core.lease.0.as_ref().ok_or("restart-not-ready")?;
                let job = recovery::begin_live(store, owner, &self.core.root, inspection)
                    .map_err(|(_, e)| e)?;
                self.completed = Some(receipt);
                self.stage = Stage::Erasing(job);
                Ok(())
            }
            Stage::Erasing(job) if job.complete() => {
                let Stage::Erasing(job) = core::mem::replace(&mut self.stage, Stage::Failed) else {
                    unreachable!()
                };
                let blank = job.finish().map_err(|_| "network-live-incomplete")?;
                recovery::check_live_blank(
                    self.core.lease.0.as_ref().ok_or("restart-not-ready")?,
                    owner,
                    &self.core.root,
                    &blank,
                )?;
                self.core
                    .begin_clean(owner, self.clean.take().ok_or("network-live-clean")?)?;
                self.stage = Stage::Cleaning(blank);
                Ok(())
            }
            Stage::Cleaning(_) if self.core.is_complete() => {
                let receipt = self.core.finish(owner)?;
                let Stage::Cleaning(blank) = core::mem::replace(&mut self.stage, Stage::Failed)
                else {
                    unreachable!()
                };
                let proof = recovery::finish_live(
                    self.core.lease.0.as_ref().ok_or("restart-not-ready")?,
                    owner,
                    &self.core.root,
                    blank,
                )
                .map_err(|(_, e)| e)?;
                journal::publish_reclaimed(proof);
                self.completed = Some(receipt);
                self.finished = true;
                self.stage = Stage::Done;
                Ok(())
            }
            Stage::Pending(_) => self.core.prepare_chunk(owner),
            Stage::Cleaning(blank) => {
                recovery::check_live_blank(
                    self.core.lease.0.as_ref().ok_or("restart-not-ready")?,
                    owner,
                    &self.core.root,
                    blank,
                )?;
                self.core.prepare_chunk(owner)
            }
            _ => Err("network-live-phase"),
        }
    }
    pub(crate) fn advance(&mut self, access: &mut impl Access) -> Result<bool, &'static str> {
        let result = match &mut self.stage {
            Stage::Pending(_) | Stage::Cleaning(_) => self.core.advance(access).map(|_| ()),
            Stage::Erasing(job) => job
                .step_with(&mut self.work, |owner, authority, step, work| match step {
                    Step::Erase(sector) => access.erase_network(owner, authority, sector),
                    Step::Read(at) => access.read_network(owner, at, work),
                })
                .map(|_| ()),
            _ => Err("network-live-phase"),
        };
        if let Err(e) = result {
            self.core.poisoned = true;
            return Err(e);
        }
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or("network-live-step-exhausted")?;
        Ok(false)
    }
    pub(crate) fn finish(
        mut self: Box<Self>,
        _owner: &SealedBodyLifecycleSession,
    ) -> Result<CommittedCustody, &'static str> {
        if !self.finished || !self.is_complete() {
            return Err("network-live-incomplete");
        }
        self.completed.take().ok_or("network-live-receipt")
    }
}
impl Drop for PendingJob {
    fn drop(&mut self) {
        if self.entered_live && !self.finished {
            self.core.effects = true;
            self.core.adopted = false;
        }
    }
}
