//! Snapshot publication through the ordinary Host Call Back. No store, scheduler,
//! retry, grant issuer or synthetic durable acknowledgement lives here.
use super::*;
use alloc::{rc::Rc, vec::Vec};
use conduit_core::BoundedResourceRef;
use conduit_kernel::{
    scheduler::{HostCallBack, StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, HostCallDisposition, PortId,
};
use core::cell::Cell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordedRange {
    pub first_sequence: u64,
    pub last_sequence: u64,
    pub samples: usize,
}
/// Observation of this one generation's completed publication, not portable
/// authority or a receipt supplied by an external caller. Clone during Root
/// preparation; reads during Play do not allocate.
#[derive(Clone)]
pub struct RecordingCommitObserver(Rc<CommittedGeneration>);
struct CommittedGeneration {
    range: Cell<Option<RecordedRange>>,
    reference: Vec<u8>,
}
impl RecordingCommitObserver {
    pub fn committed(&self) -> Option<RecordedRange> {
        self.0.range.get()
    }
    /// Exact generation selected when this Back was prepared. Identity data
    /// alone acknowledges nothing; only `committed()` observes kernel commit.
    pub fn encoded_reference(&self) -> &[u8] {
        &self.0.reference
    }
}
#[derive(Clone, Copy)]
enum Staged {
    Requested(RecordedRange),
    Published(RecordedRange),
}
pub struct RecordingSnapshotBack {
    call: HostCallBack,
    reader: PreparedRecordingReader,
    samples: usize,
    content_bytes: usize,
    pending: Option<RecordedRange>,
    staged: Option<Staged>,
    observer: RecordingCommitObserver,
    cancelled: bool,
}
impl RecordingSnapshotBack {
    /// Root must install this under the exact resource snapshot publish contract,
    /// with its ordinary current placement, resource and independent authority.
    /// These content checks do not authorize the Host Call. One Back publishes
    /// at most one generation; additional flushes require additional admission.
    pub fn prepare(
        context: RecordingContext<'_>,
        contract: &ObservationContract,
        samples: usize,
        reference: &BoundedResourceRef,
    ) -> Result<(Self, RecordingCommitObserver), RecordingSnapshotRefusal> {
        let content_bytes = PreparedRecordingSnapshot::content_bytes(
            RecordingContext {
                host: context.host,
                boot: context.boot,
                plan: context.plan,
                play: context.play,
            },
            contract,
            samples,
        )?;
        let snapshot = PreparedRecordingSnapshot::prepare(
            RecordingContext {
                host: context.host,
                boot: context.boot,
                plan: context.plan,
                play: context.play,
            },
            contract,
            samples,
            reference,
        )?;
        let observer = RecordingCommitObserver(Rc::new(CommittedGeneration {
            range: Cell::new(None),
            reference: snapshot.expected_reference().to_vec(),
        }));
        Ok((
            Self {
                call: HostCallBack::new(MAXIMUM_CHUNK_BYTES as u32),
                reader: PreparedRecordingReader::prepare(context, contract)?,
                samples,
                content_bytes,
                pending: None,
                staged: None,
                observer: observer.clone(),
                cancelled: false,
            },
            observer,
        ))
    }
}
impl<const P: usize> StepBack<P> for RecordingSnapshotBack {
    fn step(&mut self, io: &mut StepIo<P>, input: &StepInputBytes<'_, P>) -> StepOutcome {
        self.staged = None;
        if self.cancelled {
            return fail(FailureCode::Cancelled, 1);
        }
        if io.input_abnormal(PortId(0)).is_some() {
            return fail(FailureCode::InvalidInput, 2);
        }
        let mut staged = None;
        if let Some((_, outcome)) = io.host_completion() {
            let Some(range) = self.pending else {
                return fail(FailureCode::InvalidLifecycle, 3);
            };
            if outcome.disposition == HostCallDisposition::Completed {
                let Some(output) = outcome.output else {
                    return fail(FailureCode::InvalidInput, 4);
                };
                if output.value.byte_len as usize != self.observer.encoded_reference().len()
                    || input.host_output() != Some(self.observer.encoded_reference())
                {
                    return fail(FailureCode::InvalidInput, 5);
                }
                staged = Some(Staged::Published(range));
            }
        } else if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() || self.observer.committed().is_some() {
                return fail(FailureCode::InvalidLifecycle, 6);
            }
            let Some(bytes) = input.input(PortId(0)) else {
                return fail(FailureCode::InvalidInput, 7);
            };
            if bytes.len() != self.content_bytes || value.byte_len as usize != bytes.len() {
                return fail(FailureCode::InvalidInput, 8);
            }
            let Ok(json) = core::str::from_utf8(bytes) else {
                return fail(FailureCode::InvalidInput, 9);
            };
            if self.reader.read(json).is_err() || self.reader.len() != self.samples {
                return fail(FailureCode::InvalidInput, 10);
            }
            let first_sequence = self
                .reader
                .sample(0)
                .expect("nonempty selected chunk")
                .sequence;
            let last_sequence = self
                .reader
                .sample(self.samples - 1)
                .expect("selected sample count")
                .sequence;
            staged = Some(Staged::Requested(RecordedRange {
                first_sequence,
                last_sequence,
                samples: self.samples,
            }));
        }
        let outcome = self.call.step(io, input);
        if matches!(outcome, StepOutcome::Progress) {
            self.staged = staged;
        }
        outcome
    }
    fn step_committed(&mut self) {
        <HostCallBack as StepBack<P>>::step_committed(&mut self.call);
        match self.staged.take() {
            Some(Staged::Requested(range)) => self.pending = Some(range),
            Some(Staged::Published(range)) => {
                self.pending = None;
                self.observer.0.range.set(Some(range));
            }
            None => {}
        }
    }
    fn cancel(&mut self) {
        <HostCallBack as StepBack<P>>::cancel(&mut self.call);
        self.cancelled = true;
        self.pending = None;
        self.staged = None;
        // A later cancellation cannot erase an already committed publication.
    }
}
fn fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code,
        detail: 2200 + detail,
    })
}
