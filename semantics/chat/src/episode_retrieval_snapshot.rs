//! Immutable retrieval custody over an existing episode-memory Current.
//!
//! Resource references are inert provenance, never read authority. This adapter
//! uses the explicit content-version convention SHA256(exact canonical memory
//! bytes); it neither opens that resource nor changes memory/checkpoint policy.
use alloc::{string::String, vec::Vec};
use conduit_core::{
    validate_canonical_structured_value, BorrowedTemporalInstant, BoundedResourceRef,
    TemporalScale, ValidatedCanonicalStructuredValue,
};
use sha2::{Digest, Sha256};

#[path = "episode_retrieval_snapshot/validation.rs"]
mod validation;
use validation::*;
type View<'a> = ValidatedCanonicalStructuredValue<'a>;
type SnapshotRecords<'a> = [Option<(u64, View<'a>)>; 4];
pub const MAXIMUM_RETRIEVAL_CONFIGURATION_BYTES: usize = 65536;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpisodeRetrievalSnapshotRefusal {
    MemoryBound,
    ExperienceBound,
    ConfigurationBound,
    MemoryType,
    ExperienceType,
    IntentType,
    ContextPolicyType,
    Malformed,
    OwnerIdentity,
    OwnerMismatch,
    MemoryOrder,
    InvalidTime,
    UnsupportedTime,
    UnsupportedClockBasis,
    ClockMismatch,
    RetrievalAfterReference,
    InvalidSource,
    SourceProfile,
    SourceExtent,
    SourceVersion,
    SnapshotMismatch,
    EpisodeNotFound,
}
use EpisodeRetrievalSnapshotRefusal as R;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EpisodeRetrievalOwner<'a> {
    pub body_id: &'a str,
    pub wake_id: &'a str,
    pub wake_sequence: u64,
}
#[derive(Clone, Copy)]
pub struct EpisodeRetrievalSnapshotInput<'a> {
    pub memory: &'a [u8],
    pub experience: &'a [u8],
    pub expected: EpisodeRetrievalOwner<'a>,
    pub reference: BorrowedTemporalInstant<'a>,
    pub retrieved: BorrowedTemporalInstant<'a>,
    /// Explicit semantic clock class, never inferred from native clock text.
    pub clock_basis: conduit_ai::BorrowedTemporalClockBasis<'a>,
    pub intent: &'a [u8],
    pub context_policy: &'a [u8],
    pub source: &'a BoundedResourceRef,
}
pub struct PreparedEpisodeRetrievalSnapshot {
    memory_type: Vec<u8>,
    experience_type: Vec<u8>,
    intent_type: Vec<u8>,
    policy_type: Vec<u8>,
    memory_profile: String,
}
impl PreparedEpisodeRetrievalSnapshot {
    pub fn prepare() -> Self {
        let memory = crate::episode_memory_state_type();
        Self {
            memory_profile: memory
                .profile()
                .expect("native memory profile")
                .value_kind()
                .as_str()
                .into(),
            memory_type: memory.canonical_bytes().expect("native memory Type"),
            experience_type: crate::episode_experience_type()
                .canonical_bytes()
                .expect("native experience Type"),
            intent_type: conduit_ai::TemporalRetrievalIntent::semantic_type()
                .expect("native intent Type")
                .canonical_bytes()
                .unwrap(),
            policy_type: conduit_ai::ContextSelectionPolicy::semantic_type()
                .expect("native context policy Type")
                .canonical_bytes()
                .unwrap(),
        }
    }
    /// Validate all custody before publishing a snapshot. No allocation occurs.
    /// Configuration bytes are bound with their exact native Types/domains;
    /// operation-specific policy checks remain with the downstream selector.
    /// The raw SHA256 resource version convention is local to this adapter,
    /// not a general resource-version or resource-authority rule.
    pub fn capture<'a>(
        &self,
        input: EpisodeRetrievalSnapshotInput<'a>,
    ) -> Result<BorrowedEpisodeRetrievalSnapshot<'a>, R> {
        if input.memory.len() > crate::MAXIMUM_EPISODE_MEMORY_BYTES {
            return Err(R::MemoryBound);
        }
        if input.experience.len() > crate::MAXIMUM_EPISODE_EXPERIENCE_BYTES {
            return Err(R::ExperienceBound);
        }
        if input.intent.len() > MAXIMUM_RETRIEVAL_CONFIGURATION_BYTES
            || input.context_policy.len() > MAXIMUM_RETRIEVAL_CONFIGURATION_BYTES
        {
            return Err(R::ConfigurationBound);
        }
        let memory = checked(input.memory, &self.memory_type, R::MemoryType)?;
        checked(input.experience, &self.experience_type, R::ExperienceType)?;
        checked(input.intent, &self.intent_type, R::IntentType)?;
        checked(
            input.context_policy,
            &self.policy_type,
            R::ContextPolicyType,
        )?;
        validate_owner(input.expected)?;
        validate_time(input.reference)?;
        validate_time(input.retrieved)?;
        if input.reference.clock_basis != input.retrieved.clock_basis {
            return Err(R::ClockMismatch);
        }
        let clock_label = match input.clock_basis {
            conduit_ai::BorrowedTemporalClockBasis::UnixEpochMilliseconds => {
                conduit_core::UNIX_UTC_CLOCK_BASIS
            }
            conduit_ai::BorrowedTemporalClockBasis::MonotonicMilliseconds(label) => {
                if label.is_empty() || label.len() > 128 {
                    return Err(R::UnsupportedClockBasis);
                }
                label
            }
        };
        if clock_label != input.reference.clock_basis {
            return Err(R::ClockMismatch);
        }
        if input.retrieved.ticks > input.reference.ticks {
            return Err(R::RetrievalAfterReference);
        }
        input.source.validate().map_err(|_| R::InvalidSource)?;
        if input.source.content_profile.as_str() != self.memory_profile {
            return Err(R::SourceProfile);
        }
        if input.source.extent.bytes != input.memory.len() as u64 {
            return Err(R::SourceExtent);
        }
        let version: [u8; 32] = Sha256::digest(input.memory).into();
        if input.source.lifetime.version.digest() != version {
            return Err(R::SourceVersion);
        }
        let (records, count) = validate_memory(memory, input.expected)?;
        Ok(BorrowedEpisodeRetrievalSnapshot {
            input,
            identity: binding(input),
            records,
            count,
        })
    }
}
impl Default for PreparedEpisodeRetrievalSnapshot {
    fn default() -> Self {
        Self::prepare()
    }
}
/// Every view and configuration byte remains borrowed from the same capture.
/// A caller must retain these bytes until downstream selection is resolved.
pub struct BorrowedEpisodeRetrievalSnapshot<'a> {
    input: EpisodeRetrievalSnapshotInput<'a>,
    identity: [u8; 32],
    records: SnapshotRecords<'a>,
    count: usize,
}
impl<'a> BorrowedEpisodeRetrievalSnapshot<'a> {
    pub fn input(&self) -> EpisodeRetrievalSnapshotInput<'a> {
        self.input
    }
    pub fn identity(&self) -> [u8; 32] {
        self.identity
    }
    pub fn match_identity(&self, identity: &[u8; 32]) -> Result<(), R> {
        if identity == &self.identity {
            Ok(())
        } else {
            Err(R::SnapshotMismatch)
        }
    }
    /// Original records from this captured Current, in its validated order.
    pub fn records(&self) -> impl Iterator<Item = (u64, View<'a>)> + '_ {
        self.records[..self.count].iter().flatten().copied()
    }
    pub fn record_count(&self) -> usize {
        self.count
    }
    pub fn original_record(&self, episode: u64) -> Result<View<'a>, R> {
        self.records[..self.count]
            .iter()
            .flatten()
            .find(|(number, _)| *number == episode)
            .map(|(_, record)| *record)
            .ok_or(R::EpisodeNotFound)
    }
}
fn checked<'a>(bytes: &'a [u8], expected: &[u8], wrong: R) -> Result<View<'a>, R> {
    let view = validate_canonical_structured_value(bytes).map_err(|_| R::Malformed)?;
    if view.type_bytes() != expected {
        return Err(wrong);
    }
    Ok(view)
}
fn validate_time(time: BorrowedTemporalInstant<'_>) -> Result<(), R> {
    time.validate().map_err(|_| R::InvalidTime)?;
    if time.scale != TemporalScale::Milliseconds
        || time.resolution_ticks != 1
        || time.uncertainty_ticks != 0
    {
        return Err(R::UnsupportedTime);
    }
    Ok(())
}
fn part(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}
fn bind_time(hash: &mut Sha256, time: BorrowedTemporalInstant<'_>) {
    part(hash, &time.ticks.to_le_bytes());
    part(hash, b"milliseconds");
    part(hash, time.clock_basis.as_bytes());
    part(hash, &time.resolution_ticks.to_le_bytes());
    part(hash, &time.uncertainty_ticks.to_le_bytes());
}
fn binding(input: EpisodeRetrievalSnapshotInput<'_>) -> [u8; 32] {
    let mut hash = Sha256::new();
    part(&mut hash, b"conduit/episode-retrieval-snapshot@1");
    for bytes in [
        input.memory,
        input.experience,
        input.expected.body_id.as_bytes(),
        input.expected.wake_id.as_bytes(),
        &input.expected.wake_sequence.to_le_bytes(),
        input.intent,
        input.context_policy,
    ] {
        part(&mut hash, bytes)
    }
    match input.clock_basis {
        conduit_ai::BorrowedTemporalClockBasis::UnixEpochMilliseconds => {
            part(&mut hash, b"unix_epoch_milliseconds");
            part(&mut hash, conduit_core::UNIX_UTC_CLOCK_BASIS.as_bytes());
        }
        conduit_ai::BorrowedTemporalClockBasis::MonotonicMilliseconds(label) => {
            part(&mut hash, b"monotonic_milliseconds");
            part(&mut hash, label.as_bytes());
        }
    }
    bind_time(&mut hash, input.reference);
    bind_time(&mut hash, input.retrieved);
    let source = input.source;
    part(&mut hash, &source.identity.digest());
    part(&mut hash, &source.lifetime.version.digest());
    part(&mut hash, source.content_profile.as_str().as_bytes());
    part(&mut hash, source.access_class.as_str().as_bytes());
    part(&mut hash, &source.extent.bytes.to_le_bytes());
    match source.extent.items {
        Some(n) => {
            part(&mut hash, &[1]);
            part(&mut hash, &n.to_le_bytes())
        }
        None => part(&mut hash, &[0]),
    }
    match &source.lifetime.expires_at {
        Some(time) => {
            part(&mut hash, &[1]);
            part(&mut hash, &time.ticks.to_le_bytes());
            part(
                &mut hash,
                match time.scale {
                    TemporalScale::Seconds => b"seconds",
                    TemporalScale::Milliseconds => b"milliseconds",
                    TemporalScale::Microseconds => b"microseconds",
                    TemporalScale::Nanoseconds => b"nanoseconds",
                },
            );
            part(&mut hash, time.clock_basis.as_bytes());
            part(&mut hash, &time.resolution_ticks.to_le_bytes());
            part(&mut hash, &time.uncertainty_ticks.to_le_bytes())
        }
        None => part(&mut hash, &[0]),
    }
    hash.finalize().into()
}
