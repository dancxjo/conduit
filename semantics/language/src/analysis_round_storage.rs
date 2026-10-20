//! Fixed canonical storage and exact Source replay for the shared round cursor.
use super::*;
use alloc::vec::Vec;
use conduit_core::*;
use conduit_plot::{rust_binding::*, *};
use core::ops::Range;
pub(super) const BYTES: usize = 65536;
pub(super) type R<T> = Result<T, CursorRefusal>;
pub(super) fn buffer() -> Vec<u8> {
    Vec::with_capacity(BYTES)
}
pub(super) fn put(out: &mut Vec<u8>, bytes: &[u8]) -> R<()> {
    if bytes.len() > out.capacity() {
        return Err(CursorRefusal::Pressure);
    }
    out.clear();
    out.extend_from_slice(bytes);
    Ok(())
}
pub(super) fn view(bytes: &[u8]) -> R<ValidatedCanonicalStructuredValue<'_>> {
    validate_canonical_structured_value(bytes).map_err(|_| CursorRefusal::Canonical)
}
pub(super) fn field<'a>(
    v: ValidatedCanonicalStructuredValue<'a>,
    name: &str,
) -> R<ValidatedCanonicalStructuredValue<'a>> {
    v.record_field(name)
        .map_err(|_| CursorRefusal::Canonical)?
        .ok_or(CursorRefusal::Canonical)
}
pub(super) fn member<'a>(
    bytes: &'a [u8],
    path: &[&str],
) -> R<ValidatedCanonicalStructuredValue<'a>> {
    let mut value = view(bytes)?;
    for part in path {
        value = field(value, part)?;
    }
    Ok(value)
}
pub(super) fn copy_value(out: &mut Vec<u8>, value: ValidatedCanonicalStructuredValue<'_>) -> R<()> {
    if value.type_bytes().len() + value.value_node().len() > out.capacity() {
        return Err(CursorRefusal::Pressure);
    }
    out.clear();
    out.extend_from_slice(value.type_bytes());
    out.extend_from_slice(value.value_node());
    Ok(())
}
pub(super) fn number(value: ValidatedCanonicalStructuredValue<'_>) -> R<u64> {
    let bytes = value
        .primitive_representation_bytes("value/u64")
        .map_err(|_| CursorRefusal::Canonical)?;
    Ok(u64::from_le_bytes(
        bytes.try_into().map_err(|_| CursorRefusal::Canonical)?,
    ))
}
pub(super) fn boolean(value: ValidatedCanonicalStructuredValue<'_>) -> R<bool> {
    InfoBool::decode(
        value
            .primitive_representation_bytes("value/bool")
            .map_err(|_| CursorRefusal::Canonical)?,
    )
    .map(|v| v.get())
    .map_err(|_| CursorRefusal::Canonical)
}
pub(super) struct RoundIdentity {
    pub owner_instance: crate::analysis_epoch_owner::AnalysisEpochInstanceHandle,
    pub context: [String; 5],
    pub generation: u64,
    pub prior: Vec<u8>,
    pub lexical: Vec<u8>,
}
pub struct CursorSourceLease {
    pub(super) identity: Arc<RoundIdentity>,
    pub(super) ordinal: usize,
}
pub struct CursorSubmitFailure {
    pub refusal: CursorRefusal,
    pub lease: CursorSourceLease,
}
pub struct CursorRequest<'a> {
    pub operation: &'static str,
    pub input: &'a [u8],
    pub ordinal: usize,
}
pub struct CursorObservation<'a> {
    pub operation: &'static str,
    pub input: &'a [u8],
    pub output: &'a [u8],
    pub admitted: bool,
}
struct Operation {
    name: &'static str,
    evaluators: Vec<PreparedPortableExpressionEvaluator>,
    output_bound: usize,
}
struct Frame {
    operation: usize,
    input: Range<usize>,
    output: Range<usize>,
    admitted: bool,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum IoState {
    Idle,
    Ready,
    Awaiting,
    Replay,
    Complete,
    Failed,
}
pub(super) struct SourceIo {
    identity: Arc<RoundIdentity>,
    operations: Vec<Operation>,
    frames: Vec<Frame>,
    arena: Vec<u8>,
    current: Vec<u8>,
    scratch: Vec<u8>,
    state: IoState,
    program: usize,
}
impl SourceIo {
    pub fn prepare(
        identity: Arc<RoundIdentity>,
        programs: &PreparedFallbackRoundPrograms,
    ) -> R<Self> {
        let mut operations = Vec::new();
        for (name, _) in programs.programs() {
            let chain = programs.operation(name).ok_or(CursorRefusal::Preparation)?;
            let last = chain.last().ok_or(CursorRefusal::Preparation)?;
            let output_bound = last
                .maximum_prepared_output_bytes()
                .map_err(|_| CursorRefusal::Preparation)? as usize;
            let output_bound = output_bound.min(BYTES); // Explicit transport subset; actual larger frames refuse.
            let evaluators = chain
                .iter()
                .map(|p| {
                    PreparedPortableExpressionEvaluator::new(p)
                        .map_err(|_| CursorRefusal::Preparation)
                })
                .collect::<R<Vec<_>>>()?;
            operations.push(Operation {
                name,
                evaluators,
                output_bound,
            });
        }
        Ok(Self {
            identity,
            operations,
            frames: Vec::with_capacity(MAXIMUM_ROUND_OBSERVATIONS),
            arena: Vec::with_capacity(MAXIMUM_ROUND_BYTES),
            current: buffer(),
            scratch: buffer(),
            state: IoState::Idle,
            program: 0,
        })
    }
    pub fn start(&mut self, name: &str, input: &[u8]) -> R<()> {
        if !matches!(self.state, IoState::Idle | IoState::Complete) {
            return Err(CursorRefusal::Lifecycle);
        }
        let operation = self
            .operations
            .iter()
            .position(|v| v.name == name)
            .ok_or(CursorRefusal::Preparation)?;
        let maximum = input
            .len()
            .checked_add(self.operations[operation].output_bound)
            .and_then(|v| self.arena.len().checked_add(v))
            .ok_or(CursorRefusal::Bounds)?;
        if input.len() > BYTES
            || self.frames.len() == self.frames.capacity()
            || maximum > self.arena.capacity()
        {
            return Err(CursorRefusal::Pressure);
        }
        put(&mut self.current, input)?;
        let start = self.arena.len();
        self.arena.extend_from_slice(input);
        self.frames.push(Frame {
            operation,
            input: start..self.arena.len(),
            output: self.arena.len()..self.arena.len(),
            admitted: false,
        });
        self.state = IoState::Ready;
        self.program = 0;
        Ok(())
    }
    pub fn issue(&mut self) -> R<CursorSourceLease> {
        if self.state != IoState::Ready {
            return Err(CursorRefusal::Lifecycle);
        }
        self.state = IoState::Awaiting;
        Ok(CursorSourceLease {
            identity: self.identity.clone(),
            ordinal: self.frames.len() - 1,
        })
    }
    pub fn request(&self, lease: &CursorSourceLease) -> R<CursorRequest<'_>> {
        if !Arc::ptr_eq(&lease.identity, &self.identity) || lease.ordinal + 1 != self.frames.len() {
            return Err(CursorRefusal::ForeignLease);
        }
        if self.state != IoState::Awaiting {
            return Err(CursorRefusal::Lifecycle);
        }
        let frame = &self.frames[lease.ordinal];
        Ok(CursorRequest {
            operation: self.operations[frame.operation].name,
            input: &self.arena[frame.input.clone()],
            ordinal: lease.ordinal,
        })
    }
    pub fn submit(
        &mut self,
        lease: CursorSourceLease,
        output: &[u8],
    ) -> Result<(), CursorSubmitFailure> {
        let result = (|| {
            self.request(&lease)?;
            let frame = self.frames.last().ok_or(CursorRefusal::Lifecycle)?;
            if output.len() > self.operations[frame.operation].output_bound
                || self.arena.len() + output.len() > self.arena.capacity()
            {
                return Err(CursorRefusal::Pressure);
            }
            Ok(())
        })();
        if let Err(refusal) = result {
            return Err(CursorSubmitFailure { refusal, lease });
        }
        let start = self.arena.len();
        self.arena.extend_from_slice(output);
        self.frames.last_mut().expect("checked frame").output = start..self.arena.len();
        self.state = IoState::Replay;
        Ok(())
    }
    pub fn replay_step(&mut self) -> R<bool> {
        if self.state != IoState::Replay {
            return Err(CursorRefusal::Lifecycle);
        }
        let frame = self.frames.last().ok_or(CursorRefusal::Lifecycle)?;
        let chain = &mut self.operations[frame.operation].evaluators;
        if let Some(program) = chain.get_mut(self.program) {
            let output = program
                .evaluate(&self.current)
                .map_err(|_| CursorRefusal::Evaluation)?;
            put(&mut self.scratch, output)?;
            core::mem::swap(&mut self.current, &mut self.scratch);
            self.program += 1;
            return Ok(false);
        }
        if self.current != self.arena[frame.output.clone()] {
            self.state = IoState::Failed;
            return Err(CursorRefusal::ForeignOutput);
        }
        self.state = IoState::Complete;
        Ok(true)
    }
    pub fn expected_output(&self) -> Option<&[u8]> {
        if matches!(self.state, IoState::Complete | IoState::Failed) {
            Some(&self.current)
        } else {
            None
        }
    }
    pub fn output(&self) -> R<&[u8]> {
        if self.state != IoState::Complete {
            return Err(CursorRefusal::Lifecycle);
        }
        Ok(&self.arena[self
            .frames
            .last()
            .ok_or(CursorRefusal::Lifecycle)?
            .output
            .clone()])
    }
    pub fn mark_admitted(&mut self) -> R<()> {
        self.frames
            .last_mut()
            .ok_or(CursorRefusal::Lifecycle)?
            .admitted = true;
        Ok(())
    }
    pub fn observation(&self, index: usize) -> Option<CursorObservation<'_>> {
        let frame = self.frames.get(index)?;
        Some(CursorObservation {
            operation: self.operations[frame.operation].name,
            input: &self.arena[frame.input.clone()],
            output: &self.arena[frame.output.clone()],
            admitted: frame.admitted,
        })
    }
    pub fn len(&self) -> usize {
        self.frames.len()
    }
}
pub(super) struct NativeBank<'d> {
    admissions: Vec<PreparedNativeAdmission<'d>>,
    selected: Option<usize>,
}
impl<'d> NativeBank<'d> {
    pub fn prepare(document: &'d CheckedSyntaxDocument, types: &[&str]) -> R<Self> {
        let bounds = PreparedNativeBounds {
            canonical_bytes: BYTES,
            scopes: 4096,
            invariants: 4096,
        };
        let mut admissions = Vec::new();
        for name in types {
            let native = document
                .native_types
                .iter()
                .find(|t| t.name == *name)
                .ok_or(CursorRefusal::Preparation)?;
            admissions.push(
                PreparedNativeAdmission::new(document, &native.identity, bounds)
                    .map_err(CursorRefusal::Native)?,
            );
        }
        Ok(Self {
            admissions,
            selected: None,
        })
    }
    pub fn begin(&mut self, ty: &str, bytes: &[u8]) -> R<()> {
        let index = self
            .admissions
            .iter()
            .position(|v| v.native_type().name == ty)
            .ok_or(CursorRefusal::Preparation)?;
        self.admissions[index]
            .begin(bytes)
            .map_err(CursorRefusal::Native)?;
        self.selected = Some(index);
        Ok(())
    }
    pub fn step(&mut self) -> R<bool> {
        let admission = &mut self.admissions[self.selected.ok_or(CursorRefusal::Lifecycle)?];
        Ok(admission.step().map_err(CursorRefusal::Native)? == PreparedNativeProgress::Admitted)
    }
    pub fn retire(&mut self) -> R<()> {
        self.admissions[self.selected.take().ok_or(CursorRefusal::Lifecycle)?]
            .retire_admitted()
            .map_err(CursorRefusal::Native)
    }
    pub fn cancel(&mut self) {
        if let Some(index) = self.selected {
            self.admissions[index].cancel();
        }
    }
    pub fn pending(&self) -> Option<&PreparedNativeAdmission<'d>> {
        self.admissions.get(self.selected?)
    }
}
