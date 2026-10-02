//! Portable bounded accounting for monotonic generated-text deltas.

use crate::{
    GeneratedTextChunk, GeneratedTextFlowEvidence, GeneratedTextFlowRefusal,
    GeneratedTextFlowTerminal,
};
use alloc::string::String;

pub const MAXIMUM_GENERATED_TEXT_CHUNK_BYTES: usize = 4 * 1024;
const GENERATED_TEXT_CHUNK_MAGIC: &[u8; 8] = b"CDTGTC01";
const GENERATED_TEXT_CHUNK_HEADER_BYTES: usize = 8 + 8 + 4;
/// Fixed canonical envelope: magic + sequence + text length + raw UTF-8.
pub const MAXIMUM_GENERATED_TEXT_CHUNK_VALUE_BYTES: usize =
    GENERATED_TEXT_CHUNK_HEADER_BYTES + MAXIMUM_GENERATED_TEXT_CHUNK_BYTES;
pub const MAXIMUM_GENERATED_TEXT_CHUNKS: u64 = 4_096;
pub const MAXIMUM_GENERATED_TEXT_IN_FLIGHT_ITEMS: u16 = 8;

pub fn encode_generated_text_chunk(
    chunk: &GeneratedTextChunk,
) -> Result<alloc::vec::Vec<u8>, GeneratedTextFlowRefusal> {
    let mut encoded =
        alloc::vec::Vec::with_capacity(GENERATED_TEXT_CHUNK_HEADER_BYTES + chunk.text().len());
    encoded.extend_from_slice(GENERATED_TEXT_CHUNK_MAGIC);
    encoded.extend_from_slice(&chunk.sequence().to_le_bytes());
    encoded.extend_from_slice(&(chunk.text().len() as u32).to_le_bytes());
    encoded.extend_from_slice(chunk.text().as_bytes());
    Ok(encoded)
}

pub fn decode_generated_text_chunk(
    encoded: &[u8],
) -> Result<GeneratedTextChunk, GeneratedTextFlowRefusal> {
    if encoded.len() < GENERATED_TEXT_CHUNK_HEADER_BYTES
        || encoded.len() > MAXIMUM_GENERATED_TEXT_CHUNK_VALUE_BYTES
        || encoded.get(..8) != Some(GENERATED_TEXT_CHUNK_MAGIC)
    {
        return Err(GeneratedTextFlowRefusal::ChunkOverflow);
    }
    let sequence = u64::from_le_bytes(
        encoded[8..16]
            .try_into()
            .map_err(|_| GeneratedTextFlowRefusal::ChunkOverflow)?,
    );
    let text_len = u32::from_le_bytes(
        encoded[16..20]
            .try_into()
            .map_err(|_| GeneratedTextFlowRefusal::ChunkOverflow)?,
    ) as usize;
    if GENERATED_TEXT_CHUNK_HEADER_BYTES
        .checked_add(text_len)
        .filter(|length| *length == encoded.len())
        .is_none()
    {
        return Err(GeneratedTextFlowRefusal::ChunkOverflow);
    }
    let text = String::from(
        core::str::from_utf8(&encoded[GENERATED_TEXT_CHUNK_HEADER_BYTES..])
            .map_err(|_| GeneratedTextFlowRefusal::ChunkOverflow)?,
    );
    if sequence >= MAXIMUM_GENERATED_TEXT_CHUNKS {
        return Err(GeneratedTextFlowRefusal::ChunkCountOverflow);
    }
    if text.is_empty() {
        return Err(GeneratedTextFlowRefusal::EmptyChunk);
    }
    if text.len() > MAXIMUM_GENERATED_TEXT_CHUNK_BYTES {
        return Err(GeneratedTextFlowRefusal::ChunkOverflow);
    }
    GeneratedTextChunk::new(sequence, text).map_err(|_| GeneratedTextFlowRefusal::ChunkOverflow)
}

pub struct BoundedGeneratedTextFlow {
    maximum_output_bytes: u64,
    next_sequence: u64,
    generated_bytes: u64,
    terminal: Option<GeneratedTextFlowTerminal>,
}

impl BoundedGeneratedTextFlow {
    pub fn new(maximum_output_bytes: u64) -> Option<Self> {
        (maximum_output_bytes > 0 && maximum_output_bytes <= super::MAXIMUM_LLM_OUTPUT_BYTES)
            .then_some(Self {
                maximum_output_bytes,
                next_sequence: 0,
                generated_bytes: 0,
                terminal: None,
            })
    }

    pub fn admit(&mut self, chunk: &GeneratedTextChunk) -> Result<(), GeneratedTextFlowRefusal> {
        if self.terminal.is_some() {
            return Err(GeneratedTextFlowRefusal::AlreadyTerminal);
        }
        if *chunk.sequence() != self.next_sequence {
            return Err(GeneratedTextFlowRefusal::WrongSequence);
        }
        if self.next_sequence >= MAXIMUM_GENERATED_TEXT_CHUNKS {
            return Err(GeneratedTextFlowRefusal::ChunkCountOverflow);
        }
        let bytes = chunk.text().len();
        let next = self.generated_bytes.saturating_add(bytes as u64);
        if next > self.maximum_output_bytes {
            return Err(GeneratedTextFlowRefusal::OutputOverflow);
        }
        self.generated_bytes = next;
        self.next_sequence = self.next_sequence.saturating_add(1);
        Ok(())
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn finish(&mut self, terminal: GeneratedTextFlowTerminal) -> GeneratedTextFlowEvidence {
        self.terminal.get_or_insert(terminal);
        GeneratedTextFlowEvidence::new(
            self.next_sequence,
            self.generated_bytes,
            self.terminal.expect("terminal was inserted"),
            false,
        )
        .expect("bounded generated-text flow preserves native evidence invariants")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_plot::rust_binding::NativeRustBinding;

    #[test]
    fn generated_text_chunks_are_native_bounded_and_keep_the_exact_codec() {
        for sequence in [0, MAXIMUM_GENERATED_TEXT_CHUNKS - 1] {
            let chunk = GeneratedTextChunk::new(sequence, "x".into()).unwrap();
            assert_eq!(
                GeneratedTextChunk::from_structured(chunk.clone().into_structured().unwrap())
                    .unwrap(),
                chunk
            );
        }
        assert!(GeneratedTextChunk::new(MAXIMUM_GENERATED_TEXT_CHUNKS, "x".into()).is_err());
        assert!(GeneratedTextChunk::new(0, String::new()).is_err());
        assert!(
            GeneratedTextChunk::new(0, "x".repeat(MAXIMUM_GENERATED_TEXT_CHUNK_BYTES + 1)).is_err()
        );

        let chunk = GeneratedTextChunk::new(7, "hi".into()).unwrap();
        let expected = [
            b'C', b'D', b'T', b'G', b'T', b'C', b'0', b'1', 7, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0,
            b'h', b'i',
        ];
        assert_eq!(encode_generated_text_chunk(&chunk).unwrap(), expected);
        assert_eq!(decode_generated_text_chunk(&expected).unwrap(), chunk);
        assert!(!include_str!("streaming_generation.rs")
            .contains(concat!("pub struct ", "GeneratedTextChunk")));
    }

    #[test]
    fn generated_text_flow_evidence_is_native_bounded_and_coherent() {
        for evidence in [
            GeneratedTextFlowEvidence::new(0, 0, GeneratedTextFlowTerminal::ProviderLost, false)
                .unwrap(),
            GeneratedTextFlowEvidence::new(
                MAXIMUM_GENERATED_TEXT_CHUNKS,
                super::super::MAXIMUM_LLM_OUTPUT_BYTES,
                GeneratedTextFlowTerminal::Completed,
                false,
            )
            .unwrap(),
        ] {
            assert_eq!(
                GeneratedTextFlowEvidence::from_structured(evidence.into_structured().unwrap(),)
                    .unwrap(),
                evidence
            );
        }
        assert!(GeneratedTextFlowEvidence::new(
            MAXIMUM_GENERATED_TEXT_CHUNKS + 1,
            1,
            GeneratedTextFlowTerminal::Completed,
            false,
        )
        .is_err());
        assert!(GeneratedTextFlowEvidence::new(
            1,
            super::super::MAXIMUM_LLM_OUTPUT_BYTES + 1,
            GeneratedTextFlowTerminal::Completed,
            false,
        )
        .is_err());
        assert!(
            GeneratedTextFlowEvidence::new(0, 1, GeneratedTextFlowTerminal::Completed, false,)
                .is_err()
        );
        assert!(
            GeneratedTextFlowEvidence::new(1, 0, GeneratedTextFlowTerminal::Completed, false,)
                .is_err()
        );
        assert!(!include_str!("streaming_generation.rs")
            .contains(concat!("pub struct ", "GeneratedTextFlowEvidence")));
    }

    #[test]
    fn ordered_deltas_reconstruct_exactly_without_runtime_retention() {
        let mut flow = BoundedGeneratedTextFlow::new(32).unwrap();
        let chunks = [
            GeneratedTextChunk::new(0, "Hello ".into()).unwrap(),
            GeneratedTextChunk::new(1, "world.".into()).unwrap(),
        ];
        for chunk in &chunks {
            flow.admit(chunk).unwrap();
        }
        let reconstructed = chunks
            .iter()
            .map(|chunk| chunk.text().as_str())
            .collect::<String>();
        assert_eq!(reconstructed, "Hello world.");
        assert_eq!(
            flow.finish(GeneratedTextFlowTerminal::Completed),
            GeneratedTextFlowEvidence::new(2, 12, GeneratedTextFlowTerminal::Completed, false,)
                .unwrap()
        );
    }

    #[test]
    fn sequence_chunk_and_total_bounds_refuse_distinctly() {
        let mut flow = BoundedGeneratedTextFlow::new(4).unwrap();
        assert_eq!(
            flow.admit(&GeneratedTextChunk::new(1, "a".into()).unwrap()),
            Err(GeneratedTextFlowRefusal::WrongSequence)
        );
        assert!(GeneratedTextChunk::new(0, String::new()).is_err());
        flow.admit(&GeneratedTextChunk::new(0, "four".into()).unwrap())
            .unwrap();
        assert_eq!(
            flow.admit(&GeneratedTextChunk::new(1, "!".into()).unwrap()),
            Err(GeneratedTextFlowRefusal::OutputOverflow)
        );
    }
}
