//! Bounded actual-operation observations and replay of existing expression chains.
use super::FallbackRoundRefusal;
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use conduit_plot::{rust_binding::NativeRustBinding, PortableExpressionProgram};
include!(concat!(env!("OUT_DIR"), "/fallback_round_programs.rs"));
pub const MAXIMUM_ROUND_OBSERVATIONS: usize = 1024;
pub const MAXIMUM_ROUND_BYTES: usize = 32 * 1024 * 1024;
#[derive(Clone)]
pub struct FallbackOperationObservation {
    pub operation: String,
    pub input: Vec<u8>,
    pub output: Vec<u8>,
}
/// Remove caller/provider spare capacity only after successful admission. The
/// retained buffers then have canonical payload lengths plus bounded operation
/// names and observation metadata; this is not allocator/RSS accounting.
/// Refused evidence is deliberately returned without normalization.
pub(super) fn compact_observations(
    mut observations: Vec<FallbackOperationObservation>,
) -> Vec<FallbackOperationObservation> {
    for frame in &mut observations {
        frame.operation = core::mem::take(&mut frame.operation)
            .into_boxed_str()
            .into_string();
        frame.input = core::mem::take(&mut frame.input)
            .into_boxed_slice()
            .into_vec();
        frame.output = core::mem::take(&mut frame.output)
            .into_boxed_slice()
            .into_vec();
    }
    observations.into_boxed_slice().into_vec()
}
/// Supplies only actual output of the exact requested pure Source operation.
/// Implementing this trait is not an installed-execution attestation.
pub trait FallbackObservationProvider {
    fn observe(&mut self, operation: &str, input: &[u8]) -> Result<Vec<u8>, FallbackRoundRefusal>;
}
pub struct PreparedFallbackRoundPrograms {
    checked: conduit_plot::CheckedSyntaxDocument,
    operations: Vec<(&'static str, Vec<PortableExpressionProgram>)>,
}
impl PreparedFallbackRoundPrograms {
    pub fn prepare() -> Result<Self, FallbackRoundRefusal> {
        let mut operations = Vec::new();
        for (name, hexes) in FALLBACK_ROUND_PROGRAMS {
            let programs = hexes
                .iter()
                .map(|hex| {
                    PortableExpressionProgram::from_canonical_hex(hex)
                        .map_err(|_| FallbackRoundRefusal::Program)
                })
                .collect::<Result<Vec<_>, _>>()?;
            operations.push((*name, programs));
        }
        let mut startup = conduit_plot::StartupCatalog::new();
        startup
            .insert_value_kind_alias(
                "ResourceRef",
                conduit_core::kind_id(conduit_core::RESOURCE_REFERENCE_INFO_ID),
            )
            .map_err(|_| FallbackRoundRefusal::Program)?;
        let checked = conduit_plot::check_syntax_document(
            &conduit_plot::parse_syntax_document(crate::language_semantic_source()),
            &startup,
        )
        .map_err(|_| FallbackRoundRefusal::Program)?;
        if checked.source_document_id.as_str() != crate::parser_commitment::COMMITMENT_SOURCE_ID {
            return Err(FallbackRoundRefusal::Program);
        }
        Ok(Self {
            operations,
            checked,
        })
    }
    pub(super) fn operation(&self, name: &str) -> Option<&[PortableExpressionProgram]> {
        self.operations
            .iter()
            .find(|(actual, _)| *actual == name)
            .map(|(_, programs)| programs.as_slice())
    }
    pub fn checked_document(&self) -> &conduit_plot::CheckedSyntaxDocument {
        &self.checked
    }
    pub fn programs(&self) -> &'static [(&'static str, &'static [&'static str])] {
        FALLBACK_ROUND_PROGRAMS
    }
}
pub(crate) struct Replay<'a> {
    programs: &'a PreparedFallbackRoundPrograms,
    observations: &'a [FallbackOperationObservation],
    cursor: usize,
    provider: Option<&'a mut dyn FallbackObservationProvider>,
    captured: Vec<FallbackOperationObservation>,
    captured_bytes: usize,
}
impl<'a> Replay<'a> {
    pub fn new(
        programs: &'a PreparedFallbackRoundPrograms,
        observations: &'a [FallbackOperationObservation],
    ) -> Result<Self, FallbackRoundRefusal> {
        use FallbackRoundRefusal::*;
        if observations.len() > MAXIMUM_ROUND_OBSERVATIONS {
            return Err(Bound);
        }
        let mut bytes = 0usize;
        for frame in observations {
            if frame.operation.len() > 128
                || frame.input.len() > 65536
                || frame.output.len() > 65536
            {
                return Err(Bound);
            }
            bytes = bytes
                .checked_add(frame.input.len())
                .and_then(|v| v.checked_add(frame.output.len()))
                .ok_or(Bound)?;
            if bytes > MAXIMUM_ROUND_BYTES {
                return Err(Bound);
            }
        }
        Ok(Self {
            programs,
            observations,
            cursor: 0,
            provider: None,
            captured: Vec::new(),
            captured_bytes: 0,
        })
    }
    pub fn with_provider(
        programs: &'a PreparedFallbackRoundPrograms,
        provider: &'a mut dyn FallbackObservationProvider,
    ) -> Self {
        Self {
            programs,
            observations: &[],
            cursor: 0,
            provider: Some(provider),
            captured: Vec::with_capacity(MAXIMUM_ROUND_OBSERVATIONS),
            captured_bytes: 0,
        }
    }
    pub fn into_captured(self) -> Vec<FallbackOperationObservation> {
        self.captured
    }
    pub fn run<I: NativeRustBinding + Clone, O: NativeRustBinding>(
        &mut self,
        name: &str,
        input: &I,
    ) -> Result<O, FallbackRoundRefusal> {
        let bytes = input
            .clone()
            .encode()
            .map_err(FallbackRoundRefusal::Native)?;
        O::decode(&self.run_encoded(name, bytes)?).map_err(FallbackRoundRefusal::Native)
    }
    fn run_encoded(&mut self, name: &str, bytes: Vec<u8>) -> Result<Vec<u8>, FallbackRoundRefusal> {
        use FallbackRoundRefusal::*;
        if bytes.len() > 65536 {
            return Err(Bound);
        }
        let frame = if let Some(provider) = self.provider.as_mut() {
            if self.captured.len() >= MAXIMUM_ROUND_OBSERVATIONS {
                return Err(Bound);
            }
            let output = provider.observe(name, &bytes)?;
            if output.len() > 65536 {
                return Err(Bound);
            }
            let total = self
                .captured_bytes
                .checked_add(bytes.len())
                .and_then(|v| v.checked_add(output.len()))
                .ok_or(Bound)?;
            if total > MAXIMUM_ROUND_BYTES {
                return Err(Bound);
            }
            self.captured.push(FallbackOperationObservation {
                operation: name.to_string(),
                input: bytes.clone(),
                output,
            });
            self.captured_bytes = total;
            self.captured.last().ok_or(Bound)?
        } else {
            self.observations
                .get(self.cursor)
                .ok_or_else(|| IncompleteTrace {
                    operation: name.to_string(),
                    input: bytes.clone(),
                })?
        };
        if frame.operation != name || frame.input != bytes {
            return Err(ForeignObservation);
        }
        let programs = &self
            .programs
            .operations
            .iter()
            .find(|(n, _)| *n == name)
            .ok_or(Program)?
            .1;
        let mut actual = bytes;
        for program in programs {
            actual = program.evaluate(&actual).map_err(|_| Evaluation)?;
            if actual.len() > 65536 {
                return Err(Bound);
            }
        }
        if actual != frame.output {
            return Err(ForeignObservation);
        }
        self.cursor += 1;
        Ok(actual)
    }
    pub fn finish(self) -> Result<(), FallbackRoundRefusal> {
        if self.provider.is_some() || self.cursor == self.observations.len() {
            Ok(())
        } else {
            Err(FallbackRoundRefusal::ExtraObservation)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn empty_programs() -> PreparedFallbackRoundPrograms {
        PreparedFallbackRoundPrograms {
            operations: Vec::new(),
        }
    }
    #[test]
    fn missing_foreign_and_extra_observations_refuse_without_execution() {
        let programs = empty_programs();
        let mut missing = Replay::new(&programs, &[]).unwrap();
        assert!(matches!(
            missing.run_encoded("expected", Vec::new()),
            Err(FallbackRoundRefusal::IncompleteTrace { .. })
        ));
        let frames = [FallbackOperationObservation {
            operation: "foreign".into(),
            input: Vec::new(),
            output: Vec::new(),
        }];
        let mut foreign = Replay::new(&programs, &frames).unwrap();
        assert!(matches!(
            foreign.run_encoded("expected", Vec::new()),
            Err(FallbackRoundRefusal::ForeignObservation)
        ));
        assert!(matches!(
            foreign.finish(),
            Err(FallbackRoundRefusal::ExtraObservation)
        ));
    }
    #[test]
    fn accepted_observations_drop_unbounded_spare_capacity_without_changing_bytes() {
        let mut operation = String::with_capacity(8192);
        operation.push_str("expected");
        let mut input = Vec::with_capacity(65537);
        input.extend_from_slice(&[1, 2]);
        let mut output = Vec::with_capacity(131072);
        output.extend_from_slice(&[3, 4, 5]);
        let mut frames = Vec::with_capacity(MAXIMUM_ROUND_OBSERVATIONS + 1);
        frames.push(FallbackOperationObservation {
            operation,
            input,
            output,
        });
        let programs = empty_programs();
        // Packet limits describe the same payload before/after compaction.
        assert!(Replay::new(&programs, &frames).is_ok());
        assert!(frames.capacity() > MAXIMUM_ROUND_OBSERVATIONS);
        assert!(frames[0].input.capacity() > 65536);
        let compact = compact_observations(frames);
        assert_eq!(compact.capacity(), compact.len());
        let frame = &compact[0];
        assert_eq!(frame.operation, "expected");
        assert_eq!(frame.input, [1, 2]);
        assert_eq!(frame.output, [3, 4, 5]);
        assert_eq!(frame.operation.capacity(), frame.operation.len());
        assert_eq!(frame.input.capacity(), frame.input.len());
        assert_eq!(frame.output.capacity(), frame.output.len());
        assert!(Replay::new(&programs, &compact).is_ok());
        let empty = compact_observations(Vec::with_capacity(1025));
        assert_eq!(empty.capacity(), 0);
    }
    #[test]
    fn refused_observation_evidence_keeps_original_payload_and_capacity() {
        let mut output = Vec::with_capacity(131072);
        output.extend_from_slice(&[7, 8]);
        let frames = [FallbackOperationObservation {
            operation: "expected".into(),
            input: alloc::vec![0; 65537],
            output,
        }];
        let capacity = frames[0].output.capacity();
        assert!(matches!(
            Replay::new(&empty_programs(), &frames),
            Err(FallbackRoundRefusal::Bound)
        ));
        assert_eq!(frames[0].output, [7, 8]);
        assert_eq!(frames[0].output.capacity(), capacity);
        assert_eq!(frames[0].input.len(), 65537);
    }
    #[test]
    fn oversized_frame_refuses_before_program_lookup_or_replay() {
        let programs = empty_programs();
        let frames = [FallbackOperationObservation {
            operation: "expected".into(),
            input: alloc::vec![0;65537],
            output: Vec::new(),
        }];
        assert!(matches!(
            Replay::new(&programs, &frames),
            Err(FallbackRoundRefusal::Bound)
        ));
    }
}
