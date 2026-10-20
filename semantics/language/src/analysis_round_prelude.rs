//! Finite canonical request prelude for the existing shared round traversal.
//!
//! This stops after empty-round and first-active decode-complete Source gates.
//! It cannot construct/apply a round, model score, current commitment or Play proof.
//! Observed bytes become Source-law-admitted only after exact checked-program
//! admission. Actual execution custody belongs to the later target driver.
use super::*;
use alloc::sync::Arc;
use conduit_plot::{rust_binding::NativeRustBinding, PreparedPortableExpressionEvaluator};
#[path = "analysis_round_frames.rs"]
mod frames;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreludeRefusal {
    Preparation,
    Bound,
    Lifecycle,
    ForeignLease,
    Pressure,
    Evaluation,
    ForeignOutput,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreludeOperation {
    EmptyRound,
    DecodeComplete { survivor: usize },
}
impl PreludeOperation {
    pub fn name(self) -> &'static str {
        match self {
            Self::EmptyRound => "language/analysis-round-empty",
            Self::DecodeComplete { .. } => "language-parser-decode-complete",
        }
    }
}
struct RequestFrame {
    operation: PreludeOperation,
    input: Vec<u8>,
}
struct Identity {
    context: [String; 5],
    generation: u64,
    prior: Vec<u8>,
    lexical: Vec<u8>,
    frames: Vec<RequestFrame>,
}
struct Record {
    evaluators: Vec<PreparedPortableExpressionEvaluator>,
    observed: Vec<u8>,
    admitted: bool,
}
/// Unique private request membership and ordinal; not an execution attestation.
pub struct PreludeSourceLease {
    identity: Arc<Identity>,
    ordinal: usize,
}
pub struct PreludeSubmitFailure {
    pub refusal: PreludeRefusal,
    pub lease: PreludeSourceLease,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreludeProgress {
    Ready,
    AwaitingSource,
    Admitting,
    SourceAdmitted { ordinal: usize },
    Complete,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Ready,
    Awaiting,
    Admitting,
    Complete,
    Failed,
    Cancelled,
}
pub struct CanonicalRoundPrelude {
    identity: Arc<Identity>,
    records: Vec<Record>,
    ordinal: usize,
    program: usize,
    state: State,
}
pub struct PreparedPreludeEvidence {
    identity: Arc<Identity>,
    records: Vec<Record>,
}
pub struct PreludeRequestView<'a> {
    pub operation: PreludeOperation,
    pub canonical_input: &'a [u8],
    pub ordinal: usize,
}
impl CanonicalRoundPrelude {
    pub fn prepare(
        owner: &AnalysisEpochOwner<'_>,
        programs: &PreparedFallbackRoundPrograms,
    ) -> Result<Self, PreludeRefusal> {
        if owner.session.is_closed() {
            return Err(PreludeRefusal::Lifecycle);
        }
        let frames = frames::prepare(owner)?;
        let mut records = Vec::with_capacity(frames.len());
        let mut bytes = 0usize;
        for frame in &frames {
            let chain = programs
                .operation(frame.operation.name())
                .ok_or(PreludeRefusal::Preparation)?;
            let first = chain.first().ok_or(PreludeRefusal::Preparation)?;
            let input_bound = first
                .maximum_prepared_input_bytes()
                .map_err(|_| PreludeRefusal::Preparation)? as usize;
            if frame.input.len() > input_bound || frame.input.len() > 65536 {
                return Err(PreludeRefusal::Bound);
            }
            let last = chain.last().ok_or(PreludeRefusal::Preparation)?;
            let maximum = last
                .maximum_prepared_output_bytes()
                .map_err(|_| PreludeRefusal::Preparation)? as usize;
            bytes = bytes
                .checked_add(frame.input.len())
                .and_then(|n| n.checked_add(maximum))
                .ok_or(PreludeRefusal::Bound)?;
            if maximum > 65536 || bytes > MAXIMUM_ROUND_BYTES {
                return Err(PreludeRefusal::Bound);
            }
            let evaluators = chain
                .iter()
                .map(|program| {
                    PreparedPortableExpressionEvaluator::new(program)
                        .map_err(|_| PreludeRefusal::Preparation)
                })
                .collect::<Result<Vec<_>, _>>()?;
            records.push(Record {
                evaluators,
                observed: Vec::with_capacity(maximum),
                admitted: false,
            });
        }
        let identity = Arc::new(Identity {
            context: round_context(owner),
            generation: owner.generation(),
            prior: owner
                .beam()
                .clone()
                .encode()
                .map_err(|_| PreludeRefusal::Preparation)?,
            lexical: owner
                .lexical()
                .tape()
                .clone()
                .encode()
                .map_err(|_| PreludeRefusal::Preparation)?,
            frames,
        });
        if identity.prior.len() > 65536 || identity.lexical.len() > 65536 {
            return Err(PreludeRefusal::Bound);
        }
        Ok(Self {
            identity,
            records,
            ordinal: 0,
            program: 0,
            state: State::Ready,
        })
    }
    pub fn source_id(&self) -> &'static str {
        crate::parser_commitment::COMMITMENT_SOURCE_ID
    }
    pub fn generation(&self) -> u64 {
        self.identity.generation
    }
    pub fn context(&self) -> &[String; 5] {
        &self.identity.context
    }
    pub fn original_beam_canonical(&self) -> &[u8] {
        &self.identity.prior
    }
    pub fn original_lexical_canonical(&self) -> &[u8] {
        &self.identity.lexical
    }
    /// Canonical input/output arena payload capacity only. Evaluators have
    /// additional finite prepared storage; this is not a total RSS claim.
    pub fn payload_capacity(&self) -> usize {
        self.identity
            .frames
            .iter()
            .map(|f| f.input.capacity())
            .sum::<usize>()
            + self
                .records
                .iter()
                .map(|r| r.observed.capacity())
                .sum::<usize>()
            + self.identity.prior.capacity()
            + self.identity.lexical.capacity()
    }
    pub fn issue(&mut self) -> Result<PreludeSourceLease, PreludeRefusal> {
        if self.state != State::Ready {
            return Err(PreludeRefusal::Lifecycle);
        }
        self.state = State::Awaiting;
        Ok(PreludeSourceLease {
            identity: self.identity.clone(),
            ordinal: self.ordinal,
        })
    }
    pub fn request(
        &self,
        lease: &PreludeSourceLease,
    ) -> Result<PreludeRequestView<'_>, PreludeRefusal> {
        if !Arc::ptr_eq(&self.identity, &lease.identity) || lease.ordinal != self.ordinal {
            return Err(PreludeRefusal::ForeignLease);
        }
        if self.state != State::Awaiting {
            return Err(PreludeRefusal::Lifecycle);
        }
        let frame = &self.identity.frames[self.ordinal];
        Ok(PreludeRequestView {
            operation: frame.operation,
            canonical_input: &frame.input,
            ordinal: self.ordinal,
        })
    }
    /// Read-only retained request inspection, including after terminal refusal.
    /// This does not mint a request lease or reopen execution.
    pub fn retained_request(&self, ordinal: usize) -> Option<PreludeRequestView<'_>> {
        self.identity
            .frames
            .get(ordinal)
            .map(|frame| PreludeRequestView {
                operation: frame.operation,
                canonical_input: &frame.input,
                ordinal,
            })
    }
    pub fn submit_observed(
        &mut self,
        lease: PreludeSourceLease,
        output: &[u8],
    ) -> Result<(), PreludeSubmitFailure> {
        let refusal = self.request(&lease).err().or_else(|| {
            (output.len() > self.records[self.ordinal].observed.capacity())
                .then_some(PreludeRefusal::Pressure)
        });
        if let Some(refusal) = refusal {
            return Err(PreludeSubmitFailure { refusal, lease });
        }
        let record = &mut self.records[self.ordinal];
        record.observed.clear();
        record.observed.extend_from_slice(output);
        self.program = 0;
        self.state = State::Admitting;
        Ok(())
    }
    /// Advance ONE existing prepared Source program, with no prefix replay.
    pub fn step_admission(&mut self) -> Result<PreludeProgress, PreludeRefusal> {
        match self.state {
            State::Ready => return Ok(PreludeProgress::Ready),
            State::Awaiting => return Ok(PreludeProgress::AwaitingSource),
            State::Complete => return Ok(PreludeProgress::Complete),
            State::Failed | State::Cancelled => return Err(PreludeRefusal::Lifecycle),
            State::Admitting => (),
        }
        let record = &mut self.records[self.ordinal];
        let (previous, next) = record.evaluators.split_at_mut(self.program);
        let input = if self.program == 0 {
            self.identity.frames[self.ordinal].input.as_slice()
        } else {
            previous[self.program - 1].output_bytes()
        };
        if next[0].evaluate(input).is_err() {
            self.state = State::Failed;
            return Err(PreludeRefusal::Evaluation);
        }
        self.program += 1;
        if self.program < record.evaluators.len() {
            return Ok(PreludeProgress::Admitting);
        }
        if record
            .evaluators
            .last()
            .ok_or(PreludeRefusal::Evaluation)?
            .output_bytes()
            != record.observed
        {
            self.state = State::Failed;
            return Err(PreludeRefusal::ForeignOutput);
        }
        record.admitted = true;
        let ordinal = self.ordinal;
        self.ordinal += 1;
        self.state = if self.ordinal == self.records.len() {
            State::Complete
        } else {
            State::Ready
        };
        Ok(PreludeProgress::SourceAdmitted { ordinal })
    }
    pub fn observed_capacity(&self, ordinal: usize) -> Option<usize> {
        self.records
            .get(ordinal)
            .map(|record| record.observed.capacity())
    }
    pub fn observed(&self, ordinal: usize) -> Option<&[u8]> {
        self.records
            .get(ordinal)
            .map(|record| record.observed.as_slice())
    }
    pub fn admitted(&self, ordinal: usize) -> Option<&[u8]> {
        self.records
            .get(ordinal)
            .filter(|record| record.admitted)
            .map(|record| record.observed.as_slice())
    }
    /// Cancellation preserves input frames, pending identity and observed evidence.
    pub fn cancel(&mut self) {
        self.state = State::Cancelled;
    }
    pub fn finish(self) -> Result<PreparedPreludeEvidence, Self> {
        if self.state != State::Complete {
            return Err(self);
        }
        Ok(PreparedPreludeEvidence {
            identity: self.identity,
            records: self.records,
        })
    }
}
impl PreparedPreludeEvidence {
    pub fn source_id(&self) -> &'static str {
        crate::parser_commitment::COMMITMENT_SOURCE_ID
    }
    pub fn original_beam_canonical(&self) -> &[u8] {
        &self.identity.prior
    }
    pub fn original_lexical_canonical(&self) -> &[u8] {
        &self.identity.lexical
    }
    pub fn len(&self) -> usize {
        self.records.len()
    }
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
    pub fn observation(&self, ordinal: usize) -> Option<(&'static str, &[u8], &[u8])> {
        self.records.get(ordinal).map(|record| {
            (
                self.identity.frames[ordinal].operation.name(),
                self.identity.frames[ordinal].input.as_slice(),
                record.observed.as_slice(),
            )
        })
    }
}
