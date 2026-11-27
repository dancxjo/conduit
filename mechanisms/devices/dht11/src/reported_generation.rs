//! Reported latest original generation, not a merged history or storage receipt.
//! The original sealed JSON is preserved byte-for-byte. Earlier accepted chunks
//! affect recovery/outside counts and the first boundary only, never retention.
mod backing;
mod schema;
mod wire;
use crate::{recording::*, ObservationContract};
use alloc::string::String;
pub use backing::encoding_workspace_bound;
pub use schema::{
    maximum_canonical_bytes, value_type, INFO_ID, MAXIMUM_GENERATIONS, MAXIMUM_RECOVERED_SAMPLES,
};
pub use wire::PreparedReportedRecordingGenerationInput;

#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    Preparation,
    Capacity,
    Identity,
    Empty,
    Summary,
    Encoding,
    Recording(RecordingHistoryRefusal),
    Content(RecordingReadRefusal),
    Structured(conduit_core::StructuredInfoRefusal),
}
impl From<conduit_core::StructuredInfoRefusal> for Refusal {
    fn from(value: conduit_core::StructuredInfoRefusal) -> Self {
        Self::Structured(value)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReportedGenerationChannel<'a> {
    pub source_identity: &'a str,
    pub source_name: &'a str,
    pub channel_identity: &'a str,
    pub channel_name: &'a str,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReportedGenerationRecovery {
    pub chunks: u16,
    pub samples: u32,
    pub visible_samples: usize,
    pub samples_outside_view: u32,
    pub first_sequence: u64,
    pub last_sequence: u64,
    /// Recovery proves no completeness before the first accepted generation.
    pub preceding_history_known: bool,
}
struct Metadata {
    producer: [String; 4],
    channels: [[String; 4]; 2],
}
impl Metadata {
    fn prepare(
        context: RecordingContext<'_>,
        channels: [ReportedGenerationChannel<'_>; 2],
    ) -> Result<Self, Refusal> {
        let producer = [
            context.host.as_str(),
            context.boot.as_str(),
            context.plan.as_str(),
            context.play.as_str(),
        ];
        Self::from_text(
            producer,
            channels.map(|c| {
                [
                    c.source_identity,
                    c.source_name,
                    c.channel_identity,
                    c.channel_name,
                ]
            }),
        )
    }
    fn from_text(producer: [&str; 4], channels: [[&str; 4]; 2]) -> Result<Self, Refusal> {
        if producer
            .iter()
            .chain(channels.iter().flatten())
            .any(|value| !schema::text_valid(value))
        {
            return Err(Refusal::Identity);
        }
        let identities = [
            channels[0][0],
            channels[0][2],
            channels[1][0],
            channels[1][2],
        ];
        for (index, identity) in identities.iter().enumerate() {
            if identities[..index].contains(identity) {
                return Err(Refusal::Identity);
            }
        }
        Ok(Self {
            producer: producer.map(String::from),
            channels: channels.map(|c| c.map(String::from)),
        })
    }
    fn channel(&self, index: usize) -> Option<ReportedGenerationChannel<'_>> {
        let c = self.channels.get(index)?;
        Some(ReportedGenerationChannel {
            source_identity: &c[0],
            source_name: &c[1],
            channel_identity: &c[2],
            channel_name: &c[3],
        })
    }
}

/// One existing history reader plus the unchanged latest original JSON.
/// No cross-generation ring, alternative recorder or scheduler is introduced.
pub struct PreparedReportedRecordingGeneration {
    history: PreparedRecordingHistory,
    metadata: Metadata,
    original: String,
    predecessor: Option<(u64, u64)>,
}
impl PreparedReportedRecordingGeneration {
    pub fn prepare(
        context: RecordingContext<'_>,
        contract: &ObservationContract,
        maximum_generations: u16,
        channels: [ReportedGenerationChannel<'_>; 2],
    ) -> Result<Self, Refusal> {
        if !(1..=MAXIMUM_GENERATIONS).contains(&maximum_generations) {
            return Err(Refusal::Capacity);
        }
        let metadata = Metadata::prepare(
            RecordingContext {
                host: context.host,
                boot: context.boot,
                plan: context.plan,
                play: context.play,
            },
            channels,
        )?;
        Ok(Self {
            history: PreparedRecordingHistory::prepare(context, contract, maximum_generations)
                .map_err(Refusal::Recording)?,
            metadata,
            original: String::with_capacity(MAXIMUM_CHUNK_BYTES),
            predecessor: None,
        })
    }
    /// Full validation precedes replacing either latest JSON or its boundary.
    pub fn read_next(&mut self, original: &str) -> Result<(), Refusal> {
        let loaded = self.history.loaded_samples();
        let predecessor = loaded
            .checked_sub(1)
            .and_then(|index| self.history.sample(index))
            .map(|sample| (sample.sequence, sample.sampled_at_boot_ms));
        self.history
            .read_next(original)
            .map_err(Refusal::Recording)?;
        self.original.clear();
        self.original.push_str(original);
        self.predecessor = predecessor;
        Ok(())
    }
    pub fn original_json(&self) -> &str {
        &self.original
    }
    pub fn recovery(&self) -> Result<ReportedGenerationRecovery, Refusal> {
        recovery(self.history.recovered(), self.history.loaded_samples())
    }
    /// Allocating hosted preparation; result is canonical domain Info, not JSON.
    pub fn encode(&self) -> Result<alloc::vec::Vec<u8>, Refusal> {
        wire::encode(
            &self.metadata,
            &self.original,
            self.recovery()?,
            self.predecessor,
        )
    }
}

pub struct ReportedRecordingGeneration {
    metadata: Metadata,
    original: String,
    recovery: ReportedGenerationRecovery,
    predecessor: Option<(u64, u64)>,
    reader: PreparedRecordingReader,
}
impl ReportedRecordingGeneration {
    pub fn producer(&self) -> [&str; 4] {
        self.metadata.producer.each_ref().map(String::as_str)
    }
    pub fn channel(&self, index: usize) -> Option<ReportedGenerationChannel<'_>> {
        self.metadata.channel(index)
    }
    pub fn original_json(&self) -> &str {
        &self.original
    }
    pub fn recovery(&self) -> ReportedGenerationRecovery {
        self.recovery
    }
    pub fn len(&self) -> usize {
        self.reader.len()
    }
    pub fn is_empty(&self) -> bool {
        self.reader.is_empty()
    }
    pub fn sample(&mut self, index: usize) -> Option<RecordedSample<'_>> {
        let mut sample = self.reader.sample(index)?;
        if index == 0 {
            sample.gap_before = self
                .predecessor
                .filter(|(previous, _)| sample.sequence - previous > 1)
                .map(|(previous, _)| (previous + 1, sample.sequence - 1));
        }
        Some(sample)
    }
}
fn recovery(
    value: RecoveredHistory,
    visible: usize,
) -> Result<ReportedGenerationRecovery, Refusal> {
    if visible == 0 {
        return Err(Refusal::Empty);
    }
    Ok(ReportedGenerationRecovery {
        chunks: value.chunks,
        samples: value.samples,
        visible_samples: visible,
        samples_outside_view: value
            .samples
            .checked_sub(visible as u32)
            .ok_or(Refusal::Summary)?,
        first_sequence: value.first_sequence.ok_or(Refusal::Summary)?,
        last_sequence: value.last_sequence.ok_or(Refusal::Summary)?,
        preceding_history_known: false,
    })
}
