//! Canonical borrowed decoding around the shared temporal selection algorithm.
use super::TEMPORAL_EVIDENCE_FLOW_MAX_BYTES as MAX;
use crate::{
    BorrowedTemporalClockBasis, BorrowedTemporalEvidenceCandidate, BorrowedTemporalProvenance,
    BorrowedTemporalReference, BorrowedTemporalRetrievalIntent, EntityBoundary,
    TemporalEvidenceIndexSelection, TemporalEvidenceSelectionRefusal,
    TemporalEvidenceSelectionStatus, TemporalSource, TransitionDirection,
};
use alloc::vec::Vec;
use conduit_core::{validate_canonical_structured_value, ValidatedCanonicalStructuredValue};
mod decode;
use decode::*;
type View<'a> = ValidatedCanonicalStructuredValue<'a>;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemporalEvidenceFlowRefusal {
    Malformed,
    ByteBound,
    Selection(TemporalEvidenceSelectionRefusal),
}
use TemporalEvidenceFlowRefusal as R;
pub struct BoundedTemporalEvidenceFlow {
    window_type: Vec<u8>,
    intent_type: Vec<u8>,
    outcome_type: Vec<u8>,
    output: Vec<u8>,
    selection: TemporalEvidenceIndexSelection,
}
impl BoundedTemporalEvidenceFlow {
    pub fn prepare() -> Self {
        Self {
            window_type: super::temporal_evidence_window_type()
                .canonical_bytes()
                .unwrap(),
            intent_type: crate::TemporalRetrievalIntent::semantic_type()
                .expect("native temporal intent")
                .canonical_bytes()
                .unwrap(),
            outcome_type: super::temporal_evidence_outcome_type()
                .canonical_bytes()
                .unwrap(),
            output: Vec::with_capacity(MAX),
            selection: TemporalEvidenceIndexSelection::new(),
        }
    }
    pub fn output(&self) -> &[u8] {
        &self.output
    }
    pub fn execute(&mut self, window: &[u8], intent: &[u8]) -> Result<&[u8], R> {
        self.output.clear();
        if let Err(error) = self.select(window, intent) {
            self.output.clear();
            return Err(error);
        }
        Ok(&self.output)
    }
    fn select(&mut self, window: &[u8], intent: &[u8]) -> Result<(), R> {
        if window.len() > MAX || intent.len() > MAX {
            return Err(R::ByteBound);
        }
        let window = validate_canonical_structured_value(window).map_err(|_| R::Malformed)?;
        let intent = validate_canonical_structured_value(intent).map_err(|_| R::Malformed)?;
        if window.type_bytes() != self.window_type || intent.type_bytes() != self.intent_type {
            return Err(R::Malformed);
        }
        checked_identity(field(window, "identity")?, 256)?;
        let intent = decode_intent(intent)?;
        let reference = field(window, "reference")?;
        let reference = BorrowedTemporalReference {
            reference_at: number(field(reference, "reference_at")?)?,
            clock_basis: clock(field(reference, "clock_basis")?)?,
        };
        let entries = field(window, "candidates")?;
        // Canonical sequence validation already enforces <=128; retain an
        // explicit execution guard so this loop cannot silently ignore a tail.
        if entries
            .collection_index(128)
            .map_err(|_| R::Malformed)?
            .is_some()
        {
            return Err(R::Selection(
                TemporalEvidenceSelectionRefusal::TooManyCandidates,
            ));
        }
        let empty = BorrowedTemporalEvidenceCandidate {
            identity: "",
            provenance: BorrowedTemporalProvenance {
                event_at: None,
                valid_from: None,
                valid_until: None,
                observed_at: None,
                recorded_at: None,
                ingested_at: None,
                retrieved_at: 0,
                reference_at: 0,
                clock_basis: BorrowedTemporalClockBasis::UnixEpochMilliseconds,
                uncertainty_millis: None,
            },
            source: TemporalSource::Event,
            boundary: None,
            transition: None,
        };
        let mut candidates = [empty; 128];
        let mut count = 0;
        for (index, slot) in candidates.iter_mut().enumerate() {
            let Some(value) = entries
                .collection_index(index as u16)
                .map_err(|_| R::Malformed)?
            else {
                break;
            };
            *slot = decode_candidate(value)?;
            count += 1;
        }
        crate::select_borrowed_temporal_evidence_into(
            reference,
            &candidates[..count],
            intent,
            boolean(field(window, "earliest_history_complete")?)?,
            &mut self.selection,
        )
        .map_err(R::Selection)?;
        put(&mut self.output, &self.outcome_type)?;
        record(&mut self.output, 2)?;
        name(&mut self.output, "selection")?;
        match self.selection.status() {
            TemporalEvidenceSelectionStatus::Selected => {
                put(&mut self.output, &[3])?;
                name(&mut self.output, "selected")?;
                record(&mut self.output, 1)?;
                name(&mut self.output, "identities")?;
                put(&mut self.output, &[1])?;
                put(
                    &mut self.output,
                    &(self.selection.indices().len() as u32).to_le_bytes(),
                )?;
                for index in self.selection.indices() {
                    scalar(&mut self.output, candidates[*index].identity.as_bytes())?;
                }
            }
            TemporalEvidenceSelectionStatus::NoHistory => {
                unit_variant(&mut self.output, "no_history")?
            }
            TemporalEvidenceSelectionStatus::NeedEarlierHistory => {
                unit_variant(&mut self.output, "need_earlier_history")?
            }
            TemporalEvidenceSelectionStatus::BoundaryUnavailable => {
                unit_variant(&mut self.output, "boundary_unavailable")?
            }
        }
        name(&mut self.output, "window_identity")?;
        put(&mut self.output, field(window, "identity")?.value_node())?;
        validate_canonical_structured_value(&self.output).map_err(|_| R::Malformed)?;
        Ok(())
    }
}
fn put(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), R> {
    if bytes.len() > MAX - out.len() {
        return Err(R::ByteBound);
    }
    out.extend_from_slice(bytes);
    Ok(())
}
fn name(out: &mut Vec<u8>, key: &str) -> Result<(), R> {
    put(out, &(key.len() as u32).to_le_bytes())?;
    put(out, key.as_bytes())
}
fn scalar(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), R> {
    put(out, &[0])?;
    put(out, &(bytes.len() as u32).to_le_bytes())?;
    put(out, bytes)
}
fn record(out: &mut Vec<u8>, n: u32) -> Result<(), R> {
    put(out, &[2])?;
    put(out, &n.to_le_bytes())
}
fn unit_variant(out: &mut Vec<u8>, tag: &str) -> Result<(), R> {
    put(out, &[3])?;
    name(out, tag)?;
    scalar(out, &[])
}
