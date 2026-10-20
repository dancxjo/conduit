//! Shared retained complete-round traversal. Target execution remains an external driver.
use super::*;
#[cfg(not(target_has_atomic = "ptr"))]
use alloc::rc::Rc as Arc;
#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc;
use conduit_plot::{
    rust_binding::{NativeRustBinding, PreparedNativeRefusal},
    CheckedSyntaxDocument,
};
#[path = "analysis_round_completion.rs"]
mod completion;
#[path = "analysis_round_machine.rs"]
mod machine;
#[path = "analysis_round_material.rs"]
mod material;
pub use completion::CursorCompletionFailure;
#[path = "analysis_round_storage.rs"]
mod storage;
use machine::{Machine, Work};
use material::Materializer;
use storage::*;
pub use storage::{CursorObservation, CursorRequest, CursorSourceLease, CursorSubmitFailure};
#[path = "analysis_round_hosted.rs"]
pub(super) mod hosted;
#[cfg(all(
    feature = "parser-model",
    target_has_atomic = "8",
    target_has_atomic = "ptr"
))]
#[path = "analysis_round_numeric.rs"]
pub mod numerical;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CursorRefusal {
    Preparation,
    Bounds,
    Pressure,
    Lifecycle,
    Closed,
    ForeignLease,
    ForeignOutput,
    Canonical,
    Evaluation,
    Native(PreparedNativeRefusal),
    Cancelled,
    Model,
    Numerical,
    #[cfg(all(
        feature = "parser-model",
        target_has_atomic = "ptr",
        target_has_atomic = "8"
    ))]
    NumericalChannel(conduit_ai::integer_categorical_step::channel::CategoricalChannelRefusal),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorProgress {
    Preparing,
    ReadySource,
    AwaitingSource,
    AdmittingSource,
    AdmittingNative,
    ReadyNumerical,
    Complete,
    Failed,
}
#[derive(Clone, Copy)]
enum Stage {
    Build,
    InputLaw(Option<&'static str>, &'static str),
    Ready(Option<&'static str>),
    Awaiting(Option<&'static str>),
    Replay(Option<&'static str>),
    OutputLaw,
    LocalLaw,
    Numerical,
    Complete,
    Failed,
}
/// Membership of one prepared cursor instance; grants no epoch/current authority.
#[derive(Clone)]
pub struct CursorInstanceHandle(Arc<RoundIdentity>);
impl CursorInstanceHandle {
    pub fn matches(&self, cursor: &PreparedRoundCursor<'_>) -> bool {
        Arc::ptr_eq(&self.0, &cursor.identity)
    }
}
pub struct PreparedRoundCursor<'d> {
    identity: Arc<RoundIdentity>,
    machine: Machine,
    material: Materializer,
    native: NativeBank<'d>,
    io: SourceIo,
    stage: Stage,
    failure: Option<CursorRefusal>,
    #[cfg(all(
        feature = "parser-model",
        target_has_atomic = "8",
        target_has_atomic = "ptr"
    ))]
    numerical: Vec<numerical::CursorNumericalEvidence>,
    #[cfg(all(
        feature = "parser-model",
        target_has_atomic = "8",
        target_has_atomic = "ptr"
    ))]
    numeric_binding: Option<Arc<()>>,
}
impl<'d> PreparedRoundCursor<'d> {
    pub fn instance_handle(&self) -> CursorInstanceHandle {
        CursorInstanceHandle(self.identity.clone())
    }
    pub fn prepare_fallback(
        owner: &AnalysisEpochOwner<'_>,
        document: &'d CheckedSyntaxDocument,
        programs: &PreparedFallbackRoundPrograms,
    ) -> R<Self> {
        Self::prepare(
            owner,
            document,
            programs,
            AnalysisRoundMode::ZeroScoreFallback,
            0,
        )
    }
    fn prepare(
        owner: &AnalysisEpochOwner<'_>,
        document: &'d CheckedSyntaxDocument,
        programs: &PreparedFallbackRoundPrograms,
        mode: AnalysisRoundMode,
        invocation: u64,
    ) -> R<Self> {
        if owner.session.is_closed() {
            return Err(CursorRefusal::Closed);
        }
        if document != programs.checked_document() {
            return Err(CursorRefusal::Preparation);
        }
        if document.source_document_id.as_str() != crate::parser_commitment::COMMITMENT_SOURCE_ID {
            return Err(CursorRefusal::Preparation);
        }
        let machine = Machine::prepare(owner, mode, invocation)?;
        let identity = Arc::new(RoundIdentity {
            owner_instance: owner.instance_handle(),
            context: round_context(owner),
            generation: owner.generation,
            prior: machine.prior_beam.clone(),
            lexical: owner
                .lexical()
                .tape()
                .clone()
                .encode()
                .map_err(|_| CursorRefusal::Preparation)?,
        });
        Ok(Self {
            io: SourceIo::prepare(identity.clone(), programs)?,
            identity,
            machine,
            material: Materializer::prepare(document)?,
            native: NativeBank::prepare(document, material::TYPES)?,
            stage: Stage::Build,
            failure: None,
            #[cfg(all(
                feature = "parser-model",
                target_has_atomic = "8",
                target_has_atomic = "ptr"
            ))]
            numerical: Vec::with_capacity(learned::MAX_LEARNED_REQUESTS_PER_ROUND),
            #[cfg(all(
                feature = "parser-model",
                target_has_atomic = "8",
                target_has_atomic = "ptr"
            ))]
            numeric_binding: None,
        })
    }
    pub fn mode(&self) -> AnalysisRoundMode {
        self.machine.mode
    }
    pub fn context(&self) -> &[String; 5] {
        &self.identity.context
    }
    pub fn generation(&self) -> u64 {
        self.identity.generation
    }
    pub fn prior_canonical(&self) -> &[u8] {
        &self.identity.prior
    }
    pub fn lexical_canonical(&self) -> &[u8] {
        &self.identity.lexical
    }
    pub fn source_id(&self) -> &'static str {
        crate::parser_commitment::COMMITMENT_SOURCE_ID
    }
    pub fn observation_count(&self) -> usize {
        self.io.len()
    }
    pub fn observation(&self, index: usize) -> Option<CursorObservation<'_>> {
        self.io.observation(index)
    }
    pub fn expected_source_output(&self) -> Option<&[u8]> {
        self.io.expected_output()
    }
    pub fn failure(&self) -> Option<&CursorRefusal> {
        self.failure.as_ref()
    }
    pub fn pending_native(
        &self,
    ) -> Option<&conduit_plot::rust_binding::PreparedNativeAdmission<'d>> {
        self.native.pending()
    }
    pub fn next_identity(&self) -> u64 {
        self.machine.identity
    }
    pub fn result_canonical(&self) -> R<&[u8]> {
        if !matches!(self.stage, Stage::Complete) {
            return Err(CursorRefusal::Lifecycle);
        }
        Ok(&self.machine.next)
    }
    pub fn progress(&self) -> CursorProgress {
        match self.stage {
            Stage::Build | Stage::InputLaw(..) => CursorProgress::Preparing,
            Stage::Ready(_) => CursorProgress::ReadySource,
            Stage::Awaiting(_) => CursorProgress::AwaitingSource,
            Stage::Replay(_) => CursorProgress::AdmittingSource,
            Stage::OutputLaw | Stage::LocalLaw => CursorProgress::AdmittingNative,
            Stage::Numerical => CursorProgress::ReadyNumerical,
            Stage::Complete => CursorProgress::Complete,
            Stage::Failed => CursorProgress::Failed,
        }
    }
    fn fail<T>(&mut self, reason: CursorRefusal) -> R<T> {
        self.failure = Some(reason.clone());
        self.stage = Stage::Failed;
        Err(reason)
    }
    pub fn cancel(&mut self) {
        self.native.cancel();
        self.failure = Some(CursorRefusal::Cancelled);
        self.stage = Stage::Failed;
    }
    /// One existing Source program, one Native scope, or finite request construction.
    pub fn step(&mut self) -> R<CursorProgress> {
        if let Some(reason) = &self.failure {
            return Err(reason.clone());
        }
        let result = self.step_inner();
        match result {
            Ok(()) => Ok(self.progress()),
            Err(CursorRefusal::Pressure) => Err(CursorRefusal::Pressure),
            Err(reason) => self.fail(reason),
        }
    }
    fn step_inner(&mut self) -> R<()> {
        match self.stage {
            Stage::Build => match self.machine.build(&mut self.material)? {
                Work::Source(operation, input, output) => {
                    self.native.begin(input, &self.machine.query)?;
                    self.stage = Stage::InputLaw(output, operation);
                }
                Work::Native(ty) => {
                    self.native.begin(ty, &self.machine.query)?;
                    self.stage = Stage::LocalLaw;
                }
                Work::Numerical => self.stage = Stage::Numerical,
                Work::Complete => self.stage = Stage::Complete,
            },
            Stage::InputLaw(output, operation) => {
                if self.native.step()? {
                    // Retire only after the bounded I/O arena accepts the exact query.
                    self.io.start(operation, &self.machine.query)?;
                    self.native.retire()?;
                    self.stage = Stage::Ready(output);
                }
            }
            Stage::Replay(output) => {
                if self.io.replay_step()? {
                    if let Some(ty) = output {
                        self.native.begin(ty, self.io.output()?)?;
                        self.stage = Stage::OutputLaw;
                    } else {
                        self.machine.receive(self.io.output()?)?;
                        self.io.mark_admitted()?;
                        self.stage = Stage::Build;
                    }
                }
            }
            Stage::OutputLaw => {
                if self.native.step()? {
                    self.machine.receive(self.io.output()?)?;
                    self.native.retire()?;
                    self.io.mark_admitted()?;
                    self.stage = Stage::Build;
                }
            }
            Stage::LocalLaw => {
                if self.native.step()? {
                    self.machine.native_complete()?;
                    self.native.retire()?;
                    self.stage = Stage::Build;
                }
            }
            Stage::Ready(_) | Stage::Awaiting(_) | Stage::Numerical | Stage::Complete => (),
            Stage::Failed => return Err(CursorRefusal::Lifecycle),
        }
        Ok(())
    }
    pub fn issue_source(&mut self) -> R<CursorSourceLease> {
        let Stage::Ready(output) = self.stage else {
            return Err(CursorRefusal::Lifecycle);
        };
        let lease = self.io.issue()?;
        self.stage = Stage::Awaiting(output);
        Ok(lease)
    }
    pub fn source_request(&self, lease: &CursorSourceLease) -> R<CursorRequest<'_>> {
        if !matches!(self.stage, Stage::Awaiting(_)) {
            return Err(CursorRefusal::Lifecycle);
        }
        self.io.request(lease)
    }
    pub fn submit_source(
        &mut self,
        lease: CursorSourceLease,
        output: &[u8],
    ) -> Result<(), CursorSubmitFailure> {
        let Stage::Awaiting(ty) = self.stage else {
            return Err(CursorSubmitFailure {
                refusal: CursorRefusal::Lifecycle,
                lease,
            });
        };
        self.io.submit(lease, output)?;
        self.stage = Stage::Replay(ty);
        Ok(())
    }
}
