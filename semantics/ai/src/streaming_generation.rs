//! Portable bounded accounting for monotonic generated-text deltas.

use alloc::string::String;
use serde::{Deserialize, Serialize};

pub const MAXIMUM_GENERATED_TEXT_CHUNK_BYTES: usize = 4 * 1024;
/// Canonical JSON envelope headroom for sequence plus escaped text syntax.
pub const MAXIMUM_GENERATED_TEXT_CHUNK_VALUE_BYTES: usize =
    MAXIMUM_GENERATED_TEXT_CHUNK_BYTES + 128;
pub const MAXIMUM_GENERATED_TEXT_CHUNKS: u64 = 4_096;
pub const MAXIMUM_GENERATED_TEXT_IN_FLIGHT_ITEMS: u16 = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedTextChunk {
    pub sequence: u64,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GeneratedTextFlowTerminal {
    Completed,
    OutputBoundExhausted,
    Backpressured,
    Cancelled,
    ProviderLost,
    NonMonotonic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedTextFlowEvidence {
    pub chunks: u64,
    pub generated_bytes: u64,
    pub terminal: GeneratedTextFlowTerminal,
    pub retained_private_text: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratedTextFlowRefusal {
    WrongSequence,
    EmptyChunk,
    ChunkOverflow,
    ChunkCountOverflow,
    OutputOverflow,
    AlreadyTerminal,
}

pub fn encode_generated_text_chunk(
    chunk: &GeneratedTextChunk,
) -> Result<alloc::vec::Vec<u8>, GeneratedTextFlowRefusal> {
    validate_chunk(chunk)?;
    let encoded =
        serde_json::to_vec(chunk).map_err(|_| GeneratedTextFlowRefusal::ChunkOverflow)?;
    if encoded.len() > MAXIMUM_GENERATED_TEXT_CHUNK_VALUE_BYTES {
        return Err(GeneratedTextFlowRefusal::ChunkOverflow);
    }
    Ok(encoded)
}

pub fn decode_generated_text_chunk(
    encoded: &[u8],
) -> Result<GeneratedTextChunk, GeneratedTextFlowRefusal> {
    if encoded.len() > MAXIMUM_GENERATED_TEXT_CHUNK_VALUE_BYTES {
        return Err(GeneratedTextFlowRefusal::ChunkOverflow);
    }
    let chunk: GeneratedTextChunk =
        serde_json::from_slice(encoded).map_err(|_| GeneratedTextFlowRefusal::ChunkOverflow)?;
    validate_chunk(&chunk)?;
    if encode_generated_text_chunk(&chunk)? != encoded {
        return Err(GeneratedTextFlowRefusal::ChunkOverflow);
    }
    Ok(chunk)
}

fn validate_chunk(chunk: &GeneratedTextChunk) -> Result<(), GeneratedTextFlowRefusal> {
    if chunk.sequence >= MAXIMUM_GENERATED_TEXT_CHUNKS {
        return Err(GeneratedTextFlowRefusal::ChunkCountOverflow);
    }
    if chunk.text.is_empty() {
        return Err(GeneratedTextFlowRefusal::EmptyChunk);
    }
    if chunk.text.len() > MAXIMUM_GENERATED_TEXT_CHUNK_BYTES {
        return Err(GeneratedTextFlowRefusal::ChunkOverflow);
    }
    Ok(())
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
        if chunk.sequence != self.next_sequence {
            return Err(GeneratedTextFlowRefusal::WrongSequence);
        }
        if self.next_sequence >= MAXIMUM_GENERATED_TEXT_CHUNKS {
            return Err(GeneratedTextFlowRefusal::ChunkCountOverflow);
        }
        let bytes = chunk.text.len();
        if bytes == 0 {
            return Err(GeneratedTextFlowRefusal::EmptyChunk);
        }
        if bytes > MAXIMUM_GENERATED_TEXT_CHUNK_BYTES {
            return Err(GeneratedTextFlowRefusal::ChunkOverflow);
        }
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
        GeneratedTextFlowEvidence {
            chunks: self.next_sequence,
            generated_bytes: self.generated_bytes,
            terminal: self.terminal.expect("terminal was inserted"),
            retained_private_text: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_deltas_reconstruct_exactly_without_runtime_retention() {
        let mut flow = BoundedGeneratedTextFlow::new(32).unwrap();
        let chunks = [
            GeneratedTextChunk {
                sequence: 0,
                text: "Hello ".into(),
            },
            GeneratedTextChunk {
                sequence: 1,
                text: "world.".into(),
            },
        ];
        for chunk in &chunks {
            flow.admit(chunk).unwrap();
        }
        let reconstructed = chunks
            .iter()
            .map(|chunk| chunk.text.as_str())
            .collect::<String>();
        assert_eq!(reconstructed, "Hello world.");
        assert_eq!(
            flow.finish(GeneratedTextFlowTerminal::Completed),
            GeneratedTextFlowEvidence {
                chunks: 2,
                generated_bytes: 12,
                terminal: GeneratedTextFlowTerminal::Completed,
                retained_private_text: false,
            }
        );
    }

    #[test]
    fn sequence_chunk_and_total_bounds_refuse_distinctly() {
        let mut flow = BoundedGeneratedTextFlow::new(4).unwrap();
        assert_eq!(
            flow.admit(&GeneratedTextChunk {
                sequence: 1,
                text: "a".into()
            }),
            Err(GeneratedTextFlowRefusal::WrongSequence)
        );
        assert_eq!(
            flow.admit(&GeneratedTextChunk {
                sequence: 0,
                text: String::new()
            }),
            Err(GeneratedTextFlowRefusal::EmptyChunk)
        );
        flow.admit(&GeneratedTextChunk {
            sequence: 0,
            text: "four".into(),
        })
        .unwrap();
        assert_eq!(
            flow.admit(&GeneratedTextChunk {
                sequence: 1,
                text: "!".into()
            }),
            Err(GeneratedTextFlowRefusal::OutputOverflow)
        );
    }
}
