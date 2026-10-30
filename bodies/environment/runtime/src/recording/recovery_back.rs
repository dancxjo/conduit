//! Ordinary kernel endpoint for content restored by the selected snapshot Back.
//! No storage request, generation selection or durable acknowledgement is made.
use super::*;
use alloc::vec;
use conduit_core as c;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};

/// Same JSON value Kind as the existing snapshot-read Fore, with retained
/// semantic recovery as its meaning. Provider, context and finite work admission
/// remain independent realization facts supplied by Root.
pub fn recovery_kind() -> c::Kind {
    let snapshot = conduit_semantic_catalog::resource_snapshot_semantic_contract(false);
    c::Kind {
        kind_id: "environment/recover-recording".into(),
        kind_contract_revision: "conduit.environment/recording-recovery@1".into(),
        startup_parameters: vec![],
        shorthand: None,
        inputs: vec![c::PortDescriptor {
            port_id: "chunk".into(),
            value_kind: snapshot.outputs[0].value_kind.clone(),
            direction: c::PortDirection::Input,
            temporal: c::PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        }],
        outputs: vec![],
        configuration: vec![],
        semantic_laws: vec![c::KindSemanticLaw::Terminal(
            c::KindTerminalBehavior::CompletesWhenInputsClose,
        )],
        limits: c::CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_CHUNK_BYTES as u32,
        },
    }
}

/// Install authored meaning in the ordinary checker and expansion catalogs.
/// This registers no initialized Back, Host offer, storage or authority.
pub fn install_recovery_kind(
    startup: &mut conduit_plot::StartupCatalog,
    profiles: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    let kind = recovery_kind();
    startup.insert(conduit_plot::KindSignature {
        kind: kind.kind_id.as_str().into(),
        startup_parameters: vec![],
    })?;
    startup.insert_fore(kind.kind_id.as_str(), kind.checked_front())?;
    profiles
        .insert_kind(kind)
        .map_err(|error| alloc::format!("{error:?}"))
}

pub struct RecordingRecoveryBack {
    current: PreparedRecordingHistory,
    pending: PreparedRecordingHistory,
    staged: bool,
    cancelled: bool,
}
impl RecordingRecoveryBack {
    /// Two finite recovery workspaces are reserved before Play. Only one is
    /// visible; the other validates a candidate without changing committed state.
    /// Context is independently admitted producer provenance, not input JSON.
    pub fn prepare(
        context: RecordingContext<'_>,
        contract: &ObservationContract,
        maximum_chunks: u16,
    ) -> Result<Self, RecordingHistoryRefusal> {
        let duplicate = RecordingContext {
            host: context.host,
            boot: context.boot,
            plan: context.plan,
            play: context.play,
        };
        Ok(Self {
            current: PreparedRecordingHistory::prepare(context, contract, maximum_chunks)?,
            pending: PreparedRecordingHistory::prepare(duplicate, contract, maximum_chunks)?,
            staged: false,
            cancelled: false,
        })
    }
    pub fn recovered(&self) -> RecoveredHistory {
        self.current.recovered()
    }
    /// A Mask borrows committed semantic quantities. This is recovery evidence,
    /// not proof that a publication or a new storage write has completed.
    pub fn sample(&mut self, index: usize) -> Option<RecordedSample<'_>> {
        self.current.sample(index)
    }
}
impl<const P: usize> StepBack<P> for RecordingRecoveryBack {
    fn step(&mut self, io: &mut StepIo<P>, input: &StepInputBytes<'_, P>) -> StepOutcome {
        self.staged = false;
        if self.cancelled {
            return fail(FailureCode::Cancelled, 1);
        }
        if io.input_abnormal(PortId(0)).is_some() {
            return fail(FailureCode::InvalidInput, 2);
        }
        let Some(reference) = io.input(PortId(0)) else {
            return if io.input_closed(PortId(0)) {
                if io.consume_closed(PortId(0)).is_err() {
                    fail(FailureCode::InvalidLifecycle, 3)
                } else {
                    StepOutcome::Complete
                }
            } else {
                StepOutcome::Await
            };
        };
        let Some(bytes) = input.input(PortId(0)) else {
            return fail(FailureCode::InvalidInput, 4);
        };
        if bytes.len() != reference.byte_len as usize {
            return fail(FailureCode::InvalidInput, 5);
        }
        let Ok(json) = core::str::from_utf8(bytes) else {
            return fail(FailureCode::InvalidInput, 6);
        };
        if let Err(error) = self.pending.stage_from(&self.current, json) {
            use RecordingHistoryRefusal as H;
            use RecordingReadRefusal as R;
            let (code, detail) = match error {
                H::Capacity => (FailureCode::StorageExhausted, 7),
                H::Content(error) => (
                    FailureCode::InvalidInput,
                    match error {
                        R::Sequence => 8,
                        R::ClockReversed => 9,
                        R::ContextOrRepresentation => 10,
                        R::Extent => 12,
                        R::Hex => 13,
                        R::Disposition => 14,
                        R::NoncanonicalFailure => 15,
                        R::Observation(_) => 16,
                    },
                ),
                H::ZeroChunks | H::Preparation(_) => (FailureCode::InvalidLifecycle, 17),
            };
            return fail(code, detail);
        }
        if io.consume_fuel(1).is_err() || io.consume(PortId(0)).is_err() {
            return fail(FailureCode::InvalidLifecycle, 11);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn step_committed(&mut self) {
        if self.staged && !self.cancelled {
            core::mem::swap(&mut self.current, &mut self.pending);
            self.staged = false;
        }
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.staged = false;
        // Cancellation cannot erase already committed recovery.
    }
}
fn fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code,
        detail: 2300 + detail,
    })
}
