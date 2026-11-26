//! One selected immutable publication through the existing kernel Host Call Back.
//! Observation is process-local commit evidence, never storage or disclosure authority.
use alloc::{rc::Rc, vec::Vec};
use conduit_kernel::{
    scheduler::{HostCallBack, StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, HostCallDisposition, PortId,
};
use core::cell::Cell;
/// Domain validation retains its prepared workspace and returns finite metadata.
/// Recurring validation must use its admitted storage and work without growth.
pub trait SnapshotContentValidator {
    type Metadata: Copy;
    fn validate(&mut self, bytes: &[u8]) -> Result<Self::Metadata, Failure>;
}
/// Observation of this one generation's completed publication, not portable
/// authority or a receipt supplied by an external caller. Clone during Root
/// preparation; reads during Play do not allocate.
#[derive(Clone)]
pub struct SnapshotCommitObserver<M: Copy>(Rc<CommittedGeneration<M>>);
struct CommittedGeneration<M: Copy> {
    content: Cell<Option<CommittedContent<M>>>,
    reference: Vec<u8>,
    content_bytes: usize,
    digest_domain: &'static str,
}
#[derive(Clone, Copy)]
struct CommittedContent<M: Copy> {
    metadata: M,
    digest: [u8; 32],
}
impl<M: Copy> SnapshotCommitObserver<M> {
    pub fn committed(&self) -> Option<M> {
        self.0.content.get().map(|content| content.metadata)
    }
    /// Match the exact original chunk only after this Back's output commits.
    /// Matching metadata, references or a successful service completion
    /// cannot substitute for this process-local observation of kernel commit.
    /// This bounded digest comparison allocates nothing and grants no storage,
    /// disclosure or Body authority. It is not a portable commit receipt.
    pub fn committed_content_matches(&self, bytes: &[u8]) -> bool {
        bytes.len() == self.0.content_bytes
            && self.0.content.get().is_some_and(|content| {
                content.digest == content_digest(self.0.digest_domain, bytes)
            })
    }
    /// Exact generation selected when this Back was prepared. Identity data
    /// alone acknowledges nothing; only `committed()` observes kernel commit.
    pub fn encoded_reference(&self) -> &[u8] {
        &self.0.reference
    }
}
#[derive(Clone, Copy)]
enum Staged<M: Copy> {
    Requested(CommittedContent<M>),
    Published(CommittedContent<M>),
}
pub struct SnapshotPublicationBack<V: SnapshotContentValidator> {
    call: HostCallBack,
    validator: V,
    content_bytes: usize,
    pending: Option<CommittedContent<V::Metadata>>,
    staged: Option<Staged<V::Metadata>>,
    observer: SnapshotCommitObserver<V::Metadata>,
    cancelled: bool,
}
impl<V: SnapshotContentValidator> SnapshotPublicationBack<V> {
    /// Trusted preparation supplies the exact admitted generation's encoded output,
    /// finite input extent and domain validator. The caller separately admits
    /// construction memory, validator workspace and recurring work.
    /// This creates no storage grant.
    pub fn prepare(
        validator: V,
        content_bytes: usize,
        maximum_input_bytes: u32,
        reference: Vec<u8>,
        digest_domain: &'static str,
    ) -> Result<(Self, SnapshotCommitObserver<V::Metadata>), crate::SnapshotRefusal> {
        if content_bytes > maximum_input_bytes as usize
            || reference.is_empty()
            || reference.len() > maximum_input_bytes as usize
        {
            return Err(crate::SnapshotRefusal::ContentExtent);
        }
        let observer = SnapshotCommitObserver(Rc::new(CommittedGeneration {
            content: Cell::new(None),
            reference,
            content_bytes,
            digest_domain,
        }));
        Ok((
            Self {
                call: HostCallBack::new(maximum_input_bytes),
                validator,
                content_bytes,
                pending: None,
                staged: None,
                observer: observer.clone(),
                cancelled: false,
            },
            observer,
        ))
    }
    pub fn commit_observer(&self) -> SnapshotCommitObserver<V::Metadata> {
        self.observer.clone()
    }
}
impl<V: SnapshotContentValidator, const P: usize> StepBack<P> for SnapshotPublicationBack<V> {
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
            let metadata = match self.validator.validate(bytes) {
                Ok(metadata) => metadata,
                Err(failure) => return StepOutcome::Fail(failure),
            };
            staged = Some(Staged::Requested(CommittedContent {
                metadata,
                digest: content_digest(self.observer.0.digest_domain, bytes),
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
                self.observer.0.content.set(Some(range));
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
fn content_digest(domain: &str, bytes: &[u8]) -> [u8; 32] {
    conduit_core::semantic_digest(domain, bytes)
}
fn fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code,
        detail: 2200 + detail,
    })
}
