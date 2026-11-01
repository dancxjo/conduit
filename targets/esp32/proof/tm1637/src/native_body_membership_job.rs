//! Scheduled membership custody. A quiet permit is necessary, not a replay
//! timing qualification: the caller must independently admit every phase.
use super::{DurablyAdmittedMembership, OwnedRefusal, PendingCheckpoint};
use crate::{
    native_body_checkpoint::incremental::SectorStorage, native_evidence_region::BodyRegion,
};
use alloc::boxed::Box;
use conduit_body::{PortableMembershipAdmissionRequest, SealedBodyLifecycleSession};
use conduit_core::{BootId, HostId};
use conduit_monotonic_wait::flash_window::FlashPermit;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Start,
    Checkpoint(crate::native_restart::owned_checkpoint::Phase),
    Refused,
}
struct Start {
    owner: Box<SealedBodyLifecycleSession>,
    root: [u8; 32],
    host: HostId,
    boot: BootId,
    request: PortableMembershipAdmissionRequest,
}
enum State {
    Start(Box<Start>),
    Checkpoint(Box<PendingCheckpoint>),
    Refused(Box<SealedBodyLifecycleSession>, &'static str),
    Moving,
}
/// The actual owner and all request backing remain affine across scheduling.
#[must_use]
pub(crate) struct Job {
    state: State,
    step: u32,
}
pub(crate) type ReserveRefusal = (
    Box<SealedBodyLifecycleSession>,
    HostId,
    BootId,
    PortableMembershipAdmissionRequest,
    &'static str,
);
fn backing<T>() -> Option<Box<core::mem::MaybeUninit<T>>> {
    let raw = unsafe { alloc::alloc::alloc(core::alloc::Layout::new::<T>()) }
        .cast::<core::mem::MaybeUninit<T>>();
    if raw.is_null() {
        None
    } else {
        Some(unsafe { Box::from_raw(raw) })
    }
}
impl Job {
    /// Allocate both owner envelopes before taking custody or mutating Body.
    pub(crate) fn reserve(
        owner: Box<SealedBodyLifecycleSession>,
        root: [u8; 32],
        host: HostId,
        boot: BootId,
        request: PortableMembershipAdmissionRequest,
    ) -> Result<Box<Self>, ReserveRefusal> {
        let Some(job) = backing::<Self>() else {
            return Err((owner, host, boot, request, "membership-job-capacity"));
        };
        let Some(start) = backing::<Start>() else {
            return Err((owner, host, boot, request, "membership-start-capacity"));
        };
        let start = Box::write(
            start,
            Start {
                owner,
                root,
                host,
                boot,
                request,
            },
        );
        Ok(Box::write(
            job,
            Self {
                state: State::Start(start),
                step: 0,
            },
        ))
    }
    pub(crate) fn phase(&self) -> Phase {
        match &self.state {
            State::Start(_) => Phase::Start,
            State::Checkpoint(job) => Phase::Checkpoint(job.phase()),
            State::Refused(_, _) => Phase::Refused,
            State::Moving => unreachable!("synchronous owner transition"),
        }
    }
    #[cfg(test)]
    pub(crate) fn capture_prefix_bytes(&self) -> Option<usize> {
        match &self.state {
            State::Checkpoint(job) => job.capture_prefix_bytes(),
            _ => None,
        }
    }
    pub(crate) fn cached_bytes(&self) -> usize {
        match &self.state {
            State::Checkpoint(job) => job.cached_bytes(),
            _ => 0,
        }
    }
    pub(crate) fn step(&self) -> u32 {
        self.step
    }
    pub(crate) fn owner(&self) -> &SealedBodyLifecycleSession {
        match &self.state {
            State::Start(start) => &start.owner,
            State::Checkpoint(job) => job.owner(),
            State::Refused(owner, _) => owner,
            State::Moving => unreachable!("synchronous owner transition"),
        }
    }
    /// Exactly one scheduled operation, including initial canonical admission.
    /// The permit grants a quiet window; it does not qualify replay cost.
    pub(crate) fn advance(
        &mut self,
        permit: &FlashPermit<'_, '_>,
        storage: &mut impl SectorStorage<BodyRegion>,
        now_millis: u64,
    ) -> Result<(), &'static str> {
        let next = self.step.checked_add(1).ok_or("membership-job-step")?;
        if matches!(&self.state, State::Start(_)) {
            let State::Start(start) = core::mem::replace(&mut self.state, State::Moving) else {
                unreachable!()
            };
            let Start {
                owner,
                root,
                host,
                boot,
                request,
            } = *start;
            return match super::begin_owned(owner, &root, &host, &boot, &request, now_millis) {
                Ok(job) => {
                    self.state = State::Checkpoint(job);
                    self.step = next;
                    Ok(())
                }
                Err(OwnedRefusal::Unstaged(owner, reason)) => {
                    self.state = State::Refused(owner, reason);
                    self.step = next;
                    Err(reason)
                }
                Err(OwnedRefusal::Staged(job, reason)) => {
                    self.state = State::Checkpoint(job);
                    self.step = next;
                    Err(reason)
                }
            };
        }
        let State::Checkpoint(job) = &mut self.state else {
            return Err("membership-job-phase");
        };
        use crate::native_restart::owned_checkpoint::Phase as P;
        let result = match job.phase() {
            P::Prepare => job.prepare(permit),
            P::Capture => job.capture(permit),
            P::FrozenChunk | P::ReplayChunk => job.prepare_chunk(permit),
            P::Physical(_) => job.advance(storage),
            P::Finish => job.finish_source(permit),
            _ => return Err("membership-job-phase"),
        };
        // An attempted owned phase is never resubmitted with an old ordinal,
        // even when its terminal error requires retained reconciliation.
        self.step = next;
        result
    }
    pub(crate) fn finish(
        mut self: Box<Self>,
    ) -> Result<(Box<SealedBodyLifecycleSession>, DurablyAdmittedMembership), Box<Self>> {
        if !matches!(&self.state, State::Checkpoint(_)) {
            return Err(self);
        }
        let State::Checkpoint(job) = core::mem::replace(&mut self.state, State::Moving) else {
            unreachable!()
        };
        match super::finish_owned(job) {
            Ok(done) => Ok(done),
            Err(job) => {
                self.state = State::Checkpoint(job);
                Err(self)
            }
        }
    }
    /// Cancel only an unexecuted Start, recovering original request backing.
    /// No canonical admission, Store lease, or mutation has happened here.
    pub(crate) fn release_unstarted(
        mut self: Box<Self>,
    ) -> Result<
        (
            Box<SealedBodyLifecycleSession>,
            HostId,
            BootId,
            PortableMembershipAdmissionRequest,
        ),
        Box<Self>,
    > {
        if !matches!(&self.state, State::Start(_)) {
            return Err(self);
        }
        let State::Start(start) = core::mem::replace(&mut self.state, State::Moving) else {
            unreachable!()
        };
        let Start {
            owner,
            host,
            boot,
            request,
            ..
        } = *start;
        Ok((owner, host, boot, request))
    }
    pub(crate) fn release_refused(
        mut self: Box<Self>,
    ) -> Result<(Box<SealedBodyLifecycleSession>, &'static str), Box<Self>> {
        if !matches!(&self.state, State::Refused(_, _)) {
            return Err(self);
        }
        let State::Refused(owner, reason) = core::mem::replace(&mut self.state, State::Moving)
        else {
            unreachable!()
        };
        Ok((owner, reason))
    }
}
