//! Finite, append-only request fencing for one explicitly owned print session.
//! This is protocol custody, never a scheduler, spooler, or durable printer log.
use crate::{
    PrintAdmissionRefusal, PrintAttempt, PrintIdentity, PrintLifecycle, PrintOperation,
    PrintProtocolRefusal, PrintProviderReport, PrintRequest, PrintSupport,
};
use conduit_core::ResourceReferenceBinding;

pub const MAXIMUM_PRINT_ATTEMPTS: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrintRegistryRefusal {
    InvalidCapacity,
    DuplicateJobId,
    DuplicateAttemptId,
    CapacityExhausted,
    UnknownAttempt,
    Admission(PrintAdmissionRefusal),
    Protocol(PrintProtocolRefusal),
}

/// Retains every admitted job and attempt ID, including completed, cancelled,
/// rejected and uncertain attempts. Full retention refuses rather than evicts.
///
/// Preparation owns the fixed slot array. Admitting an attempt uses the existing
/// portable print owner; observing a report delegates to its lifecycle machine.
/// There is no remove, automatic retry, provider selection or effect invocation.
/// IDs and lifecycle values use their existing bounded native representations.
///
/// The owner must retain this book while any of its operations can be repeated.
/// A new book is an explicit new custody scope, not evidence that an uncertain
/// effect did not happen. This in-memory fence gives no cross-restart or physical
/// exactly-once guarantee; a real provider needs its own admitted durable proof.
pub struct PrintAttemptRegistry<const N: usize> {
    attempts: [Option<PrintAttempt>; N],
    admitted: usize,
}
impl<const N: usize> PrintAttemptRegistry<N> {
    pub fn prepare() -> Result<Self, PrintRegistryRefusal> {
        if N == 0 || N > MAXIMUM_PRINT_ATTEMPTS {
            return Err(PrintRegistryRefusal::InvalidCapacity);
        }
        Ok(Self {
            attempts: core::array::from_fn(|_| None),
            admitted: 0,
        })
    }
    pub fn admitted(&self) -> usize {
        self.admitted
    }
    pub fn capacity(&self) -> usize {
        N
    }
    /// A changed payload or new attempt ID cannot bypass an existing job ID.
    /// Refusal leaves all existing attempts and their uncertain outcomes intact.
    pub fn admit(
        &mut self,
        request: &PrintRequest,
        support: &PrintSupport,
        binding: &ResourceReferenceBinding,
        attempt_id: PrintIdentity,
    ) -> Result<&PrintOperation, PrintRegistryRefusal> {
        if self
            .attempts
            .iter()
            .flatten()
            .any(|attempt| attempt.operation().job_id() == request.job_id())
        {
            return Err(PrintRegistryRefusal::DuplicateJobId);
        }
        if self
            .attempts
            .iter()
            .flatten()
            .any(|attempt| attempt.operation().attempt_id() == &attempt_id)
        {
            return Err(PrintRegistryRefusal::DuplicateAttemptId);
        }
        if self.admitted == N {
            return Err(PrintRegistryRefusal::CapacityExhausted);
        }
        let attempt = PrintAttempt::prepare(request, support, binding, attempt_id)
            .map_err(PrintRegistryRefusal::Admission)?;
        let index = self.admitted;
        self.attempts[index] = Some(attempt);
        self.admitted += 1;
        Ok(self.attempts[index].as_ref().unwrap().operation())
    }
    pub fn attempt(&self, attempt_id: &PrintIdentity) -> Option<&PrintAttempt> {
        self.attempts
            .iter()
            .flatten()
            .find(|attempt| attempt.operation().attempt_id() == attempt_id)
    }
    pub fn observe(
        &mut self,
        attempt_id: &PrintIdentity,
        report: PrintProviderReport,
    ) -> Result<PrintLifecycle, PrintRegistryRefusal> {
        self.attempts
            .iter_mut()
            .flatten()
            .find(|attempt| attempt.operation().attempt_id() == attempt_id)
            .ok_or(PrintRegistryRefusal::UnknownAttempt)?
            .observe(report)
            .map_err(PrintRegistryRefusal::Protocol)
    }
}
