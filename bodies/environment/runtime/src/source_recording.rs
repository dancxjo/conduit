//! Immutable, bounded content for selected Source snapshots. Encoding preserves
//! the entire canonical value, including its Form. It grants no storage/Body
//! authority and observes no kernel commit or durable acknowledgement.
use alloc::{string::String, vec::Vec};
use conduit_core as c;
use conduit_environment_measurement::source_observation::{ObservationContract, SourceOutcome};
use serde::{Deserialize, Serialize};

pub const MAXIMUM_BYTES: usize = 4096;
pub const MAXIMUM_SAMPLES: usize = 16;
const SCHEMA: &str = "conduit.environment/source-recording@1";

/// Independently pinned by Root from the selected actual native execution.
/// Names supplied by a candidate document are not authentication or authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRecordingContext {
    pub body_id: String,
    pub attachment_id: String,
    pub host_id: c::HostId,
    pub boot_id: c::BootId,
    pub capability_id: c::CapabilityId,
    pub provider_instance_id: c::BaseInstanceId,
    pub provider_generation: u64,
    pub plan_id: c::PlanId,
    pub fragment_id: c::FragmentId,
    pub placement_id: c::PlacementId,
    pub configuration_id: String,
    pub firmware_sha256: [u8; 32],
    pub native_play: c::ActivePlayIdentity,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    Capacity,
    Scope,
    Encoding,
    Observation,
    Order,
    Conflict,
    Empty,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Append {
    Added,
    Duplicate,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    sequence: u64,
    canonical: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    schema: String,
    context: SourceRecordingContext,
    entries: Vec<Entry>,
}
/// One source session and actual native Play per generation. Root assigns
/// recording sequence identities independently of unauthenticated source counters.
/// All encoding/decoding here is bounded preparation, not allocation-free Steps.
pub struct SourceRecordingSnapshot {
    document: Document,
    bytes: Vec<u8>,
}
impl SourceRecordingContext {
    fn validate(&self) -> Result<(), Refusal> {
        let names = [
            self.body_id.as_str(),
            self.attachment_id.as_str(),
            self.host_id.as_str(),
            self.boot_id.as_str(),
            self.capability_id.as_str(),
            self.provider_instance_id.as_str(),
            self.plan_id.as_str(),
            self.fragment_id.as_str(),
            self.placement_id.as_str(),
            self.configuration_id.as_str(),
            self.native_play.active_play_id.as_str(),
        ];
        if names
            .iter()
            .any(|s| s.is_empty() || s.len() > 256 || s.chars().any(char::is_control))
            || self.provider_generation == 0
            || self.native_play.host_id != self.host_id
            || self.native_play.boot_id != self.boot_id
            || self.native_play.plan_id != self.plan_id
            || self.native_play
                != c::bind_active_play(
                    &self.plan_id,
                    &self.host_id,
                    &self.boot_id,
                    self.native_play.play_sequence,
                )
        {
            return Err(Refusal::Scope);
        }
        Ok(())
    }
}
impl SourceRecordingSnapshot {
    pub fn prepare(context: SourceRecordingContext) -> Result<Self, Refusal> {
        context.validate()?;
        let document = Document {
            schema: SCHEMA.into(),
            context,
            entries: Vec::new(),
        };
        let bytes = encode(&document)?;
        Ok(Self { document, bytes })
    }
    /// Idempotence requires the exact original sequence AND bytes. A replay may
    /// match an earlier retained entry; a changed value never replaces it.
    pub fn append(&mut self, sequence: u64, canonical: &[u8]) -> Result<Append, Refusal> {
        if sequence == 0 {
            return Err(Refusal::Order);
        }
        let contract = ObservationContract::prepare().map_err(|_| Refusal::Observation)?;
        let value = contract
            .decode(canonical)
            .map_err(|_| Refusal::Observation)?;
        if let Some(entry) = self
            .document
            .entries
            .iter()
            .find(|entry| entry.sequence == sequence)
        {
            return if unhex(&entry.canonical)? == canonical {
                Ok(Append::Duplicate)
            } else {
                Err(Refusal::Conflict)
            };
        }
        if self.document.entries.len() == MAXIMUM_SAMPLES {
            return Err(Refusal::Capacity);
        }
        if let Some(previous) = self.document.entries.last() {
            let bytes = unhex(&previous.canonical)?;
            let earlier = contract.decode(&bytes).map_err(|_| Refusal::Observation)?;
            if sequence <= previous.sequence
                || value.observed_at_host_boot_ms < earlier.observed_at_host_boot_ms
            {
                return Err(Refusal::Order);
            }
        }
        if let SourceOutcome::Available(sample) = value.outcome {
            // Failures carry no source counter. Compare against the last success
            // so an intervening failure cannot hide a reversed/reused counter.
            for previous in self.document.entries.iter().rev() {
                let bytes = unhex(&previous.canonical)?;
                let earlier = contract.decode(&bytes).map_err(|_| Refusal::Observation)?;
                if let SourceOutcome::Available(earlier) = earlier.outcome {
                    if sample.source_counter <= earlier.source_counter {
                        return Err(Refusal::Order);
                    }
                    break;
                }
            }
        }
        self.document.entries.push(Entry {
            sequence,
            canonical: hex(canonical),
        });
        match encode(&self.document) {
            Ok(bytes) => {
                self.bytes = bytes;
                Ok(Append::Added)
            }
            Err(error) => {
                self.document.entries.pop();
                Err(error)
            }
        }
    }
    /// Supplies content only; no reference or commit is invented here.
    pub fn content(&self) -> Result<&[u8], Refusal> {
        if self.document.entries.is_empty() {
            return Err(Refusal::Empty);
        }
        Ok(&self.bytes)
    }
    pub fn context(&self) -> &SourceRecordingContext {
        &self.document.context
    }
    pub fn len(&self) -> usize {
        self.document.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.document.entries.is_empty()
    }
    pub fn observation(&self, index: usize) -> Result<(u64, Vec<u8>), Refusal> {
        let entry = self
            .document
            .entries
            .get(index)
            .ok_or(Refusal::Observation)?;
        Ok((entry.sequence, unhex(&entry.canonical)?))
    }
    /// Require Root's independently retained original context on recovery.
    /// This is exact correspondence, not signature verification or admission.
    pub fn read(bytes: &[u8], expected: &SourceRecordingContext) -> Result<Self, Refusal> {
        Self::decode(bytes, Some(expected))
    }
    /// Decode bounded canonical candidate evidence without granting origin,
    /// membership or storage authority. Recovery must independently establish
    /// catalog custody, exact content identity and the expected Body before
    /// accepting the embedded original context. Live readers should use read.
    pub fn decode_candidate(bytes: &[u8]) -> Result<Self, Refusal> {
        Self::decode(bytes, None)
    }
    fn decode(bytes: &[u8], expected: Option<&SourceRecordingContext>) -> Result<Self, Refusal> {
        if bytes.len() > MAXIMUM_BYTES {
            return Err(Refusal::Capacity);
        }
        let document: Document = serde_json::from_slice(bytes).map_err(|_| Refusal::Encoding)?;
        if document.schema != SCHEMA || expected.is_some_and(|e| &document.context != e) {
            return Err(Refusal::Scope);
        }
        if document.entries.is_empty() {
            return Err(Refusal::Empty);
        }
        if document.entries.len() > MAXIMUM_SAMPLES {
            return Err(Refusal::Capacity);
        }
        let mut result = Self::prepare(document.context)?;
        for entry in document.entries {
            if result.append(entry.sequence, &unhex(&entry.canonical)?)? != Append::Added {
                return Err(Refusal::Order);
            }
        }
        if result.bytes != bytes {
            return Err(Refusal::Encoding);
        }
        Ok(result)
    }
}
fn encode(document: &Document) -> Result<Vec<u8>, Refusal> {
    let bytes = serde_json::to_vec(document).map_err(|_| Refusal::Encoding)?;
    if bytes.len() > MAXIMUM_BYTES {
        return Err(Refusal::Capacity);
    }
    Ok(bytes)
}
fn hex(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(bytes.len() * 2);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 15) as usize] as char);
    }
    result
}
fn unhex(text: &str) -> Result<Vec<u8>, Refusal> {
    use conduit_environment_measurement::source_observation::MAXIMUM_BYTES as OBSERVATION_BYTES;
    if text.len() > OBSERVATION_BYTES * 2 || text.len() % 2 != 0 {
        return Err(Refusal::Observation);
    }
    let digit = |byte| match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(Refusal::Encoding),
    };
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| Ok((digit(pair[0])? << 4) | digit(pair[1])?))
        .collect()
}
