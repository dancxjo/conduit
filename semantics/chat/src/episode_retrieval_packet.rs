//! Native capture codec. Embedded canonical packets preserve exact custody.
use crate::{
    BorrowedEpisodeRetrievalSnapshot, EpisodeRetrievalOwner, EpisodeRetrievalSnapshotInput,
    PreparedEpisodeRetrievalSnapshot,
};
use alloc::vec::Vec;
use conduit_core::*;
use sha2::{Digest, Sha256};

pub const MAXIMUM_EPISODE_RETRIEVAL_CAPTURE_BYTES: usize = 163840;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpisodeRetrievalPacketRefusal {
    Malformed,
    ByteBound,
    Context,
    OutOfOrder,
    Snapshot(crate::EpisodeRetrievalSnapshotRefusal),
    Window(crate::EpisodeRetrievalWindowRefusal),
}
use EpisodeRetrievalPacketRefusal as R;
pub(crate) type View<'a> = ValidatedCanonicalStructuredValue<'a>;
pub(crate) fn field<'a>(v: View<'a>, name: &str) -> Result<View<'a>, R> {
    v.record_field(name)
        .map_err(|_| R::Malformed)?
        .ok_or(R::Malformed)
}
pub(crate) fn leaf(v: View<'_>) -> Result<&[u8], R> {
    let n = v.value_node();
    if n.len() < 5
        || n[0] != 0
        || u32::from_le_bytes(n[1..5].try_into().unwrap()) as usize != n.len() - 5
    {
        return Err(R::Malformed);
    }
    Ok(&n[5..])
}
pub(crate) fn text(v: View<'_>) -> Result<&str, R> {
    core::str::from_utf8(leaf(v)?).map_err(|_| R::Malformed)
}
pub(crate) fn number(v: View<'_>) -> Result<u64, R> {
    Ok(u64::from_le_bytes(
        leaf(v)?.try_into().map_err(|_| R::Malformed)?,
    ))
}
pub(crate) fn put(out: &mut Vec<u8>, b: &[u8]) -> Result<(), R> {
    if b.len() > MAXIMUM_EPISODE_RETRIEVAL_CAPTURE_BYTES.saturating_sub(out.len()) {
        return Err(R::ByteBound);
    }
    out.extend_from_slice(b);
    Ok(())
}
pub(crate) fn name(out: &mut Vec<u8>, name: &str) -> Result<(), R> {
    put(out, &(name.len() as u32).to_le_bytes())?;
    put(out, name.as_bytes())
}
pub(crate) fn scalar(out: &mut Vec<u8>, b: &[u8]) -> Result<(), R> {
    put(out, &[0])?;
    put(out, &(b.len() as u32).to_le_bytes())?;
    put(out, b)
}

pub fn episode_retrieval_capture_type() -> StructuredInfoType {
    crate::EpisodeRetrievalCapture::semantic_type().expect("native retrieval capture")
}
pub fn episode_retrieval_clock_class_type() -> StructuredInfoType {
    crate::EpisodeRetrievalClockClass::semantic_type().expect("native retrieval clock class")
}

/// Prepared decode state; it holds no memory Current or historical store.
pub struct PreparedEpisodeRetrievalPacket {
    capture_type: Vec<u8>,
    snapshot: PreparedEpisodeRetrievalSnapshot,
    source: BoundedResourceRef,
}
impl Default for PreparedEpisodeRetrievalPacket {
    fn default() -> Self {
        Self::prepare()
    }
}
impl PreparedEpisodeRetrievalPacket {
    pub fn prepare() -> Self {
        Self {
            capture_type: episode_retrieval_capture_type().canonical_bytes().unwrap(),
            snapshot: PreparedEpisodeRetrievalSnapshot::prepare(),
            source: BoundedResourceRef {
                identity: ResourceSemanticIdentity::from_digest([0; 32]),
                content_profile: crate::episode_memory_state_type()
                    .profile()
                    .unwrap()
                    .value_kind()
                    .clone(),
                access_class: ResourceClassId::from("resource/episode-memory-snapshot"),
                extent: ResourceExtent {
                    bytes: 0,
                    items: None,
                },
                lifetime: ResourceLifetime {
                    version: ResourceVersionIdentity::from_digest([0; 32]),
                    expires_at: None,
                },
            },
        }
    }
    /// Resource identity names this Body/Wake's captured resident memory. Its
    /// version is SHA256(exact memory packet), not a read grant or relevance rank.
    pub fn decode<'a>(
        &'a mut self,
        bytes: &'a [u8],
    ) -> Result<(BorrowedEpisodeRetrievalSnapshot<'a>, &'a str), R> {
        if bytes.len() > MAXIMUM_EPISODE_RETRIEVAL_CAPTURE_BYTES {
            return Err(R::ByteBound);
        }
        let packet = validate_canonical_structured_value(bytes).map_err(|_| R::Malformed)?;
        if packet.type_bytes() != self.capture_type {
            return Err(R::Malformed);
        }
        let memory = leaf(field(packet, "memory")?)?;
        let experience = leaf(field(packet, "experience")?)?;
        let expected = EpisodeRetrievalOwner {
            body_id: text(field(packet, "body_id")?)?,
            wake_id: text(field(packet, "wake_id")?)?,
            wake_sequence: number(field(packet, "wake_sequence")?)?,
        };
        let experience_view =
            validate_canonical_structured_value(experience).map_err(|_| R::Malformed)?;
        let reference =
            BorrowedTemporalInstant::from_canonical(field(experience_view, "reference")?)
                .map_err(|_| R::Malformed)?;
        let class = field(packet, "clock_class")?;
        let clock_basis = if class
            .variant_payload("monotonic_milliseconds")
            .map_err(|_| R::Malformed)?
            .is_some()
        {
            conduit_ai::BorrowedTemporalClockBasis::MonotonicMilliseconds(reference.clock_basis)
        } else if class
            .variant_payload("unix_epoch_milliseconds")
            .map_err(|_| R::Malformed)?
            .is_some()
        {
            conduit_ai::BorrowedTemporalClockBasis::UnixEpochMilliseconds
        } else {
            return Err(R::Malformed);
        };
        let mut identity = Sha256::new();
        identity.update(b"conduit/episode-resident-memory@1");
        for part in [
            expected.body_id.as_bytes(),
            expected.wake_id.as_bytes(),
            &expected.wake_sequence.to_le_bytes(),
        ] {
            identity.update((part.len() as u64).to_le_bytes());
            identity.update(part);
        }
        self.source.identity = ResourceSemanticIdentity::from_digest(identity.finalize().into());
        self.source.extent.bytes = memory.len() as u64;
        self.source.lifetime.version =
            ResourceVersionIdentity::from_digest(Sha256::digest(memory).into());
        let snapshot = self
            .snapshot
            .capture(EpisodeRetrievalSnapshotInput {
                memory,
                experience,
                expected,
                reference,
                retrieved: reference,
                clock_basis,
                intent: leaf(field(packet, "intent")?)?,
                context_policy: leaf(field(packet, "context_policy")?)?,
                source: &self.source,
            })
            .map_err(R::Snapshot)?;
        Ok((snapshot, text(field(packet, "display_name")?)?))
    }
}
