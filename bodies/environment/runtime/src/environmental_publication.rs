//! Sensor-independent environmental publications through the ordinary kernel
//! input-consumption commit boundary. No direct DTO-to-Face publication API.
#[cfg(any(
    feature = "local-planning",
    feature = "environmental-source",
    all(test, feature = "face-projection")
))]
use alloc::vec;
use alloc::vec::Vec;
#[cfg(any(
    feature = "local-planning",
    feature = "environmental-source",
    all(test, feature = "face-projection")
))]
use conduit_environment_measurement::source_observation::MAXIMUM_BYTES;
use conduit_environment_measurement::source_observation::{ObservationContract, SourceObservation};
pub use conduit_environment_measurement::source_observation::{
    SourceFailure, SourceOutcome, SourceSample,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};

/// The producer's selected Source edge must be bound to this Back's sole input
/// by ordinary verified lowering. Preparing storage alone grants no execution.
/// Snapshot constructors are private: only committed kernel input becomes visible.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicationScope {
    pub(crate) host: conduit_core::HostId,
    pub(crate) boot: conduit_core::BootId,
    pub(crate) plan: conduit_core::PlanId,
    pub(crate) play: conduit_core::ActivePlayId,
    pub(crate) source: conduit_core::SourceDocumentId,
    pub(crate) checked: conduit_core::CheckedPlotId,
    pub(crate) expanded: conduit_core::ExpandedPlotId,
}
pub struct EnvironmentalPublicationBack {
    scope: PublicationScope,
    contract: ObservationContract,
    pending: Vec<u8>,
    pending_length: usize,
    current: Vec<u8>,
    pending_value: Option<(u64, Option<u64>)>,
    committed_ms: Option<u64>,
    last_source_counter: Option<u64>,
    sequence: u64,
    maximum_events: u32,
    cancelled: bool,
}
#[derive(Clone, Copy)]
pub struct CommittedEnvironmentalObservation<'a> {
    scope: &'a PublicationScope,
    sequence: u64,
    contract: &'a ObservationContract,
    canonical: &'a [u8],
}
impl<'a> CommittedEnvironmentalObservation<'a> {
    pub fn scope(self) -> &'a PublicationScope {
        self.scope
    }
    pub fn sequence(self) -> u64 {
        self.sequence
    }
    pub fn value(self) -> SourceObservation {
        self.contract
            .decode(self.canonical)
            .expect("private committed canonical value")
    }
    pub fn canonical(self) -> &'a [u8] {
        self.canonical
    }
}
impl EnvironmentalPublicationBack {
    #[cfg(any(
        feature = "local-planning",
        feature = "environmental-source",
        all(test, feature = "face-projection")
    ))]
    pub(crate) fn prepare(
        scope: PublicationScope,
        maximum_events: u32,
    ) -> Result<Self, conduit_core::StructuredInfoRefusal> {
        if maximum_events == 0 {
            return Err(conduit_core::StructuredInfoRefusal::CanonicalEncodingTooLarge);
        }
        Ok(Self {
            scope,
            contract: ObservationContract::prepare()?,
            pending: vec![0; MAXIMUM_BYTES],
            pending_length: 0,
            current: Vec::with_capacity(MAXIMUM_BYTES),
            pending_value: None,
            committed_ms: None,
            last_source_counter: None,
            sequence: 0,
            maximum_events,
            cancelled: false,
        })
    }
    pub fn scope(&self) -> &PublicationScope {
        &self.scope
    }
    pub fn snapshot(&self) -> Option<CommittedEnvironmentalObservation<'_>> {
        self.committed_ms
            .map(|_| CommittedEnvironmentalObservation {
                scope: &self.scope,
                sequence: self.sequence,
                contract: &self.contract,
                canonical: &self.current,
            })
    }
}
fn fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}
impl<const P: usize> StepBack<P> for EnvironmentalPublicationBack {
    fn step(&mut self, io: &mut StepIo<P>, input: &StepInputBytes<'_, P>) -> StepOutcome {
        self.pending_value = None;
        if self.cancelled {
            return fail(FailureCode::Cancelled, 1);
        }
        if io.input_abnormal(PortId(0)).is_some() {
            return fail(FailureCode::InvalidInput, 1);
        }
        let Some(reference) = io.input(PortId(0)) else {
            return if io.input_closed(PortId(0)) {
                if io.consume_closed(PortId(0)).is_ok() {
                    StepOutcome::Complete
                } else {
                    fail(FailureCode::InvalidLifecycle, 1)
                }
            } else {
                StepOutcome::Await
            };
        };
        if self.sequence >= u64::from(self.maximum_events) {
            return fail(FailureCode::InvalidInput, 2);
        }
        let Some(bytes) = input.input(PortId(0)) else {
            return fail(FailureCode::InvalidInput, 3);
        };
        if bytes.len() != reference.byte_len as usize {
            return fail(FailureCode::InvalidInput, 4);
        }
        let Ok(value) = self.contract.decode(bytes) else {
            return fail(FailureCode::InvalidInput, 5);
        };
        if self
            .committed_ms
            .is_some_and(|prior| value.observed_at_host_boot_ms < prior)
        {
            return fail(FailureCode::InvalidInput, 6);
        }
        if let SourceOutcome::Available(sample) = value.outcome {
            if self
                .last_source_counter
                .is_some_and(|prior| sample.source_counter <= prior)
            {
                return fail(FailureCode::InvalidInput, 7);
            }
        }
        if io.consume_fuel(1).is_err() {
            return fail(FailureCode::InvalidLifecycle, 2);
        }
        self.pending[..bytes.len()].copy_from_slice(bytes);
        if io.consume(PortId(0)).is_err() {
            return fail(FailureCode::InvalidLifecycle, 3);
        }
        self.pending_value = Some((
            value.observed_at_host_boot_ms,
            match value.outcome {
                SourceOutcome::Available(sample) => Some(sample.source_counter),
                SourceOutcome::Unavailable(_) => None,
            },
        ));
        // Length stays within the preallocated envelope; no allocation in Play.
        self.pending_length = bytes.len();
        StepOutcome::Progress
    }
    fn step_committed(&mut self) {
        if self.cancelled {
            return;
        }
        if let Some((observed_ms, source_counter)) = self.pending_value.take() {
            self.current.clear();
            self.current
                .extend_from_slice(&self.pending[..self.pending_length]);
            if let Some(counter) = source_counter {
                self.last_source_counter = Some(counter);
            }
            self.committed_ms = Some(observed_ms);
            self.sequence += 1;
        }
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.pending_value = None;
    }
}

#[cfg(test)]
#[path = "environmental_publication_tests.rs"]
mod tests;
