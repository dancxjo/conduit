//! Hosted examples of materially different bindings to the core step contract.
//!
//! These Rust traits are conveniences, not the normative implementation ABI.

use conduit_core::{
    Id, ImplementationError, ImplementationMachine, StepObservation, StepOutcome, StepUsage,
    WakeInterest, WakeInterestKind,
};

/// Owned wake interest used at hosted/foreign boundaries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedWakeInterest {
    pub kind: WakeInterestKind,
    pub subject: String,
}

/// Owned outcome used by the direct native Rust binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OwnedStepOutcome {
    Progress,
    Pending,
    Yielded,
    Completed,
    Failed { code: String },
}

/// Complete direct-binding reply.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedStepReply {
    pub outcome: OwnedStepOutcome,
    pub interests: Vec<OwnedWakeInterest>,
}

/// Optional direct native Rust convenience binding.
pub trait NativeStepImplementation {
    fn step(&mut self, maximum_work: u32) -> OwnedStepReply;
}

/// Direct-call adapter. The executor still validates every reply.
pub struct NativeStepBinding<T> {
    implementation: T,
}

impl<T> NativeStepBinding<T> {
    #[must_use]
    pub const fn new(implementation: T) -> Self {
        Self { implementation }
    }

    #[must_use]
    pub fn into_inner(self) -> T {
        self.implementation
    }
}

impl<T: NativeStepImplementation> NativeStepBinding<T> {
    pub fn step(
        &mut self,
        machine: &mut ImplementationMachine,
        maximum_work: u32,
        executor_usage: StepUsage,
    ) -> Result<StepObservation, ImplementationError> {
        let reply = self.implementation.step(maximum_work);
        observe_owned_reply(machine, &reply, executor_usage)
    }
}

/// Request message suitable for a process or WASM-component transport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ForeignStepRequest {
    pub protocol_version: u16,
    pub sequence: u64,
    pub maximum_work: u32,
}

/// Reply message suitable for a process or WASM-component transport.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForeignStepReply {
    pub protocol_version: u16,
    pub outcome: String,
    pub failure_code: Option<String>,
    pub interests: Vec<OwnedWakeInterest>,
}

/// Message exchange supplied by a process/WASM-specific host adapter.
pub trait MessageStepEndpoint {
    fn exchange(&mut self, request: ForeignStepRequest) -> ForeignStepReply;
}

/// Message adapter with explicit protocol/version decoding.
pub struct MessageStepBinding<T> {
    endpoint: T,
    next_sequence: u64,
}

impl<T> MessageStepBinding<T> {
    #[must_use]
    pub const fn new(endpoint: T) -> Self {
        Self {
            endpoint,
            next_sequence: 0,
        }
    }

    #[must_use]
    pub fn into_inner(self) -> T {
        self.endpoint
    }
}

impl<T: MessageStepEndpoint> MessageStepBinding<T> {
    pub fn step(
        &mut self,
        machine: &mut ImplementationMachine,
        maximum_work: u32,
        executor_usage: StepUsage,
    ) -> Result<StepObservation, ImplementationError> {
        let sequence = self.next_sequence;
        let reply = self.endpoint.exchange(ForeignStepRequest {
            protocol_version: 0,
            sequence,
            maximum_work,
        });
        if reply.protocol_version != 0 {
            return Err(ImplementationError::InvalidProfile);
        }
        self.next_sequence = sequence
            .checked_add(1)
            .ok_or(ImplementationError::StepBoundExceeded)?;
        let outcome = match reply.outcome.as_str() {
            "progress" => OwnedStepOutcome::Progress,
            "pending" => OwnedStepOutcome::Pending,
            "yielded" => OwnedStepOutcome::Yielded,
            "completed" => OwnedStepOutcome::Completed,
            "failed" => OwnedStepOutcome::Failed {
                code: reply
                    .failure_code
                    .clone()
                    .ok_or(ImplementationError::InvalidProfile)?,
            },
            _ => return Err(ImplementationError::InvalidProfile),
        };
        observe_owned_reply(
            machine,
            &OwnedStepReply {
                outcome,
                interests: reply.interests,
            },
            executor_usage,
        )
    }
}

fn observe_owned_reply(
    machine: &mut ImplementationMachine,
    reply: &OwnedStepReply,
    executor_usage: StepUsage,
) -> Result<StepObservation, ImplementationError> {
    let interests = reply
        .interests
        .iter()
        .map(|interest| {
            Id::new(&interest.subject)
                .map(|subject| WakeInterest {
                    kind: interest.kind,
                    subject,
                })
                .map_err(|_| ImplementationError::UnqualifiedPending)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let outcome = match &reply.outcome {
        OwnedStepOutcome::Progress => StepOutcome::Progress,
        OwnedStepOutcome::Pending => StepOutcome::Pending(&interests),
        OwnedStepOutcome::Yielded => StepOutcome::Yielded,
        OwnedStepOutcome::Completed => StepOutcome::Completed,
        OwnedStepOutcome::Failed { code } => StepOutcome::Failed {
            code: Id::new(code).map_err(|_| ImplementationError::InvalidProfile)?,
        },
    };
    machine.observe_step(outcome, executor_usage)
}
