//! Borrowed views of actual retained executions. No replacement frames or
//! inferred admission, commitment or playback evidence are manufactured here.
use crate::{
    parser_session_inspection::{
        ExecutionEventKind, ExecutionEventView, InspectionEncoding as E, InspectionFrameSource,
        InspectionFrames, InspectionMaterial as M, InspectionRefusal,
    },
    parser_session_pure_source::PureSourceHistory,
    parser_session_window8_book::{BookAdmission, BookAdmissionOutcome},
    parser_session_window8_ports::PORTS,
};
fn frame(bytes: &[u8]) -> M<'_> {
    M {
        encoding: E::CanonicalStructuredValue,
        bytes,
    }
}
pub(crate) fn admission_event<'a>(
    admission: &'a BookAdmission,
    origin: &'a PureSourceHistory,
    epoch: u64,
    model_calls: u64,
) -> ExecutionEventView<'a> {
    let accepted = admission.outcome == BookAdmissionOutcome::Accepted;
    ExecutionEventView {
        kind: if accepted {
            ExecutionEventKind::Admission
        } else {
            ExecutionEventKind::Refusal
        },
        step_name: admission.gate.name(),
        ordinal: origin.ordinal,
        epoch,
        model_call: Some(model_calls),
        original_program: Some(M {
            encoding: E::ExactExpressionProgramHexLines,
            bytes: origin.original_programs().as_bytes(),
        }),
        source_custody: origin.source_custody().map(|bytes| M {
            encoding: E::ExactUtf8Manifest,
            bytes: bytes.as_bytes(),
        }),
        source_plan: None,
        model_plan: None,
        input_type: embedded_type(&origin.output),
        output_type: embedded_type(&admission.canonical),
        input: frame(&origin.output),
        output: accepted.then(|| frame(&admission.canonical)),
        intermediates: InspectionFrames::empty(),
        rejected_material: (!accepted).then(|| frame(&admission.canonical)),
        refusal: match admission.outcome {
            BookAdmissionOutcome::Accepted => None,
            BookAdmissionOutcome::ProvisionalInvariantRefusal { original_law_index } => {
                Some(InspectionRefusal::NativeInvariant { original_law_index })
            }
        },
    }
}
fn embedded_type(bytes: &[u8]) -> Option<M<'_>> {
    let value = conduit_core::validate_canonical_structured_value(bytes).ok()?;
    Some(M {
        encoding: E::CanonicalType,
        bytes: value.type_bytes(),
    })
}
impl InspectionFrameSource for PureSourceHistory {
    fn frame_count(&self) -> usize {
        self.intermediates.len()
    }
    fn frame(&self, index: usize) -> Option<M<'_>> {
        self.intermediates.get(index).map(|bytes| frame(bytes))
    }
}
pub(crate) fn source_event(
    history: &PureSourceHistory,
    port: usize,
    epoch: u64,
    model_calls: u64,
) -> Option<ExecutionEventView<'_>> {
    let selected = PORTS.get(port)?;
    if !history.matches_fixed(
        selected.original_programs,
        selected.original_custody,
        selected.input,
        selected.output,
    ) {
        return None;
    }
    Some(ExecutionEventView {
        kind: ExecutionEventKind::Source,
        step_name: selected.name,
        ordinal: history.ordinal,
        epoch,
        model_call: Some(model_calls),
        original_program: Some(M {
            encoding: E::ExactExpressionProgramHexLines,
            bytes: history.original_programs().as_bytes(),
        }),
        source_custody: history.source_custody().map(|bytes| M {
            encoding: E::ExactUtf8Manifest,
            bytes: bytes.as_bytes(),
        }),
        source_plan: None,
        model_plan: None,
        input_type: embedded_type(&history.input),
        output_type: embedded_type(&history.output),
        input: frame(&history.input),
        output: Some(frame(&history.output)),
        intermediates: InspectionFrames::new(history),
        rejected_material: None,
        refusal: None,
    })
}
