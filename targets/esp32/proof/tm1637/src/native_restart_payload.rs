//! Complete private restart envelope; no pristine-body replacement or lost
//! archive segment can satisfy its canonical continuation checks.
use alloc::{string::String, vec::Vec};
use conduit_body::{AdmissionManager, BodyBiographyArchiveSegment, BodyBiographyEvidence};
use serde::{Deserialize, Serialize};
#[path = "native_restart_archives.rs"]
pub(crate) mod archive;
#[path = "native_restart_compress.rs"]
mod compress;
#[path = "native_restart_framed_rewrite.rs"]
mod framed_rewrite;
#[path = "native_restart_frames.rs"]
pub(super) mod frames;
#[path = "native_restart_profile.rs"]
mod profile;
#[path = "native_restart_read_json.rs"]
mod read_json;
#[path = "native_restart_rewrite.rs"]
pub(super) mod rewrite;
pub const MAX_PLAIN_BYTES: usize = 128 * 1024;
pub const SCHEMA: &str = "conduit.environment/root-restart@1";
#[path = "native_restart_network.rs"]
pub(crate) mod network;
pub(crate) const SCHEMA_V4: &str = "conduit.environment/root-restart@4";
pub(crate) const SCHEMA_V3: &str = "conduit.environment/root-restart@3";
#[path = "native_restart_layout.rs"]
pub(crate) mod layout;
pub(crate) const SCHEMA_V2: &str = "conduit.environment/root-restart@2";
#[path = "native_biography_custody_wire.rs"]
pub(crate) mod wire;
pub const MAX_BYTES: usize = 32768 - 160;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Retained {
    pub schema: String,
    #[serde(deserialize_with = "profile::deserialize_evidence")]
    pub evidence: BodyBiographyEvidence,
    #[serde(deserialize_with = "profile::deserialize_archives")]
    pub archives: Vec<BodyBiographyArchiveSegment>,
    #[serde(default)]
    pub admissions: Option<AdmissionManager>,
}
#[derive(Serialize)]
struct Snapshot<'a> {
    schema: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    network_cursor: Option<&'a network::Cursor>,
    evidence: profile::EvidenceRef<'a>,
    archives: Archives<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    admissions: Option<&'a AdmissionManager>,
}
struct Archives<'a>(
    &'a [BodyBiographyArchiveSegment],
    &'a [BodyBiographyArchiveSegment],
);
impl Serialize for Archives<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(self.0.len() + self.1.len()))?;
        for segment in self.0.iter().chain(self.1) {
            seq.serialize_element(&profile::ArchiveRef::from(segment))?;
        }
        seq.end()
    }
}
pub fn validate(
    evidence: &BodyBiographyEvidence,
    archives: &[BodyBiographyArchiveSegment],
) -> Result<(), &'static str> {
    validate_chain(evidence, archives, &[])
}
fn validate_chain(
    evidence: &BodyBiographyEvidence,
    archives: &[BodyBiographyArchiveSegment],
    pending: &[BodyBiographyArchiveSegment],
) -> Result<(), &'static str> {
    profile::ensure_no_holds(evidence, archives)?;
    profile::ensure_no_holds(evidence, pending)?;
    evidence.validate().map_err(|_| "restart-biography")?;
    let mut previous = None;
    let mut ordinal = 0u64;
    for segment in archives.iter().chain(pending) {
        segment.validate().map_err(|_| "restart-archive")?;
        ordinal = ordinal.checked_add(1).ok_or("restart-archive-capacity")?;
        if segment.body_id != evidence.body_id
            || segment.ordinal != ordinal
            || segment.previous_digest != previous
        {
            return Err("restart-archive-chain");
        }
        previous = Some(segment.digest);
    }
    match archives.iter().chain(pending).last() {
        Some(head) => head
            .validate_as_head_of(evidence)
            .map_err(|_| "restart-archive-head")?,
        None if evidence.compaction.as_ref().is_some_and(|summary| {
            summary.sealed_segments != 0 || summary.archive_head_digest.is_some()
        }) =>
        {
            return Err("restart-missing-archive")
        }
        None => (),
    }
    Ok(())
}
pub fn encode(
    evidence: &BodyBiographyEvidence,
    archives: &[BodyBiographyArchiveSegment],
    pending: &[BodyBiographyArchiveSegment],
) -> Result<Vec<u8>, &'static str> {
    validate_chain(evidence, archives, pending)?;
    let snapshot = Snapshot {
        schema: SCHEMA,
        network_cursor: None,
        evidence: evidence.into(),
        archives: Archives(archives, pending),
        admissions: None,
    };
    let mut size = 2048;
    loop {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(size)
            .map_err(|_| "restart-memory")?;
        bytes.resize(size, 0);
        match serde_json_core::to_slice(&snapshot, &mut bytes) {
            Ok(n) => {
                bytes.truncate(n);
                return Ok(bytes);
            }
            Err(serde_json_core::ser::Error::BufferFull) if size < MAX_BYTES => {
                size = (size * 2).min(MAX_BYTES)
            }
            Err(_) => return Err("restart-capacity-or-encoding"),
        }
    }
}
#[cfg(not(target_os = "none"))]
pub fn decode(
    bytes: &[u8],
    expected_body: &conduit_body::BodyId,
) -> Result<Retained, &'static str> {
    if bytes.len() > MAX_BYTES {
        return Err("restart-capacity");
    }
    if matches!(bytes.first(), Some(b'B' | b'F')) {
        let mut at = 0;
        return decode_streamed(
            |out| {
                let n = 128.min(bytes.len() - at);
                out[..n].copy_from_slice(&bytes[at..at + n]);
                at += n;
                Ok(n)
            },
            expected_body,
        );
    }
    let retained: Retained = serde_json::from_slice(bytes).map_err(|_| "restart-encoding")?;
    validate_retained(retained, expected_body)
}
/// Identical strict typed DTO and canonical validation, with only bounded input
/// chunks retained. Complete EOF is required before the result can escape.
struct PrefixedReader<R> {
    read: R,
    prefix: [u8; 128],
    length: Option<usize>,
}
impl<R: FnMut(&mut [u8; 128]) -> Result<usize, ()>> PrefixedReader<R> {
    fn new(mut read: R) -> Result<Self, &'static str> {
        let mut prefix = [0; 128];
        let length = read(&mut prefix).map_err(|_| "restart-read")?;
        if length > 128 {
            return Err("restart-read");
        }
        Ok(Self {
            read,
            prefix,
            length: Some(length),
        })
    }
    fn read(&mut self, out: &mut [u8; 128]) -> Result<usize, ()> {
        if let Some(length) = self.length.take() {
            out[..length].copy_from_slice(&self.prefix[..length]);
            self.prefix.fill(0);
            Ok(length)
        } else {
            (self.read)(out)
        }
    }
}
impl<R> Drop for PrefixedReader<R> {
    fn drop(&mut self) {
        self.prefix.fill(0);
    }
}
// One concrete trait-object entrance keeps the complete DTO deserializer shared
// between compressed and legacy media, and initial and parked restoration.
#[inline(never)]
fn decode_plain(
    read: &mut dyn FnMut(&mut [u8; 128]) -> Result<usize, ()>,
    expected_body: &conduit_body::BodyId,
) -> Result<Retained, &'static str> {
    let retained = read_json::from_reader(read, MAX_PLAIN_BYTES).map_err(|_| "restart-encoding")?;
    validate_retained(retained, expected_body)
}
fn with_plain<T>(
    read: impl FnMut(&mut [u8; 128]) -> Result<usize, ()>,
    consume: impl FnOnce(&mut dyn FnMut(&mut [u8; 128]) -> Result<usize, ()>) -> Result<T, &'static str>,
) -> Result<T, &'static str> {
    with_plain_framing(read, consume).map(|(value, _)| value)
}
// The dispatcher owns no codec arena. Only the selected outlined branch owns
// one; callbacks borrow it synchronously and no Decoder crosses a return ABI.
fn with_plain_framing<T>(
    read: impl FnMut(&mut [u8; 128]) -> Result<usize, ()>,
    consume: impl FnOnce(&mut dyn FnMut(&mut [u8; 128]) -> Result<usize, ()>) -> Result<T, &'static str>,
) -> Result<(T, Option<frames::Parsed>), &'static str> {
    let mut prefixed = PrefixedReader::new(read)?;
    let kind = if prefixed.length == Some(0) {
        0
    } else {
        prefixed.prefix[0]
    };
    let mut consume = Some(consume);
    let mut callback = |reader: &mut dyn FnMut(&mut [u8; 128]) -> Result<usize, ()>| {
        let consume = consume.take().ok_or("restart-consumer-reused")?;
        consume(reader)
    };
    let mut source = |out: &mut [u8; 128]| prefixed.read(out);
    match kind {
        b'F' => with_framed_plain(&mut source, &mut callback),
        b'B' => with_compressed_plain(&mut source, &mut callback).map(|v| (v, None)),
        _ => callback(&mut source).map(|v| (v, None)),
    }
}
#[inline(never)]
#[allow(
    clippy::type_complexity,
    reason = "borrowed higher-ranked reader and consumer keep codec arenas out of caller frames"
)]
fn with_compressed_plain<T>(
    source: &mut dyn FnMut(&mut [u8; 128]) -> Result<usize, ()>,
    consume: &mut dyn FnMut(
        &mut dyn FnMut(&mut [u8; 128]) -> Result<usize, ()>,
    ) -> Result<T, &'static str>,
) -> Result<T, &'static str> {
    let mut decoder = compress::Decoder::new(source, MAX_PLAIN_BYTES, MAX_BYTES);
    consume(&mut |out| decoder.read(out).map_err(|_| ()))
}
#[inline(never)]
#[allow(
    clippy::type_complexity,
    reason = "borrowed higher-ranked reader and consumer keep codec arenas out of caller frames"
)]
fn with_framed_plain<T>(
    source: &mut dyn FnMut(&mut [u8; 128]) -> Result<usize, ()>,
    consume: &mut dyn FnMut(
        &mut dyn FnMut(&mut [u8; 128]) -> Result<usize, ()>,
    ) -> Result<T, &'static str>,
) -> Result<(T, Option<frames::Parsed>), &'static str> {
    frames::Decoder::with(source, |decoder| {
        let value = consume(&mut |out| decoder.read(out))?;
        let parsed = decoder.finish().map_err(|_| "restart-frame-eof")?;
        Ok((value, Some(parsed)))
    })
    .map_err(|_| "restart-frame")?
}
pub(crate) fn decode_resident(
    read: impl FnMut(&mut [u8; 128]) -> Result<usize, ()>,
    body: &conduit_body::BodyId,
    custody: Option<&archive::Summary>,
) -> Result<archive::Resident, &'static str> {
    let (pending, framing) =
        with_plain_framing(read, |plain| archive::decode(plain, body, custody))?;
    pending.with_framing(framing)?.validate_receipts(body, None)
}
/// Context-bound decoder. Root supplies the key derived from its first fully
/// authenticated seed read and compares the second seed before publication.
pub(crate) fn decode_resident_with_prefix(
    read: impl FnMut(&mut [u8; 128]) -> Result<usize, ()>,
    body: &conduit_body::BodyId,
    root: &[u8; 32],
    custody: Option<archive::Custody<'_>>,
) -> Result<archive::Resident, &'static str> {
    let key = ed25519_dalek::VerifyingKey::from_bytes(root).map_err(|_| "restart-root-key")?;
    let (pending, framing) = with_plain_framing(read, |plain| {
        archive::decode_with_prefix(plain, body, Some(&key), custody)
    })?;
    pending
        .with_framing(framing)?
        .validate_receipts(body, Some(&key))
}
/// Native restore entrance: preserve pointer-sized custody across parser and
/// journal frames; signatures still run only after complete bounded decoding.
#[inline(never)]
pub(crate) fn decode_resident_boxed_with_prefix(
    read: impl FnMut(&mut [u8; 128]) -> Result<usize, ()>,
    body: &conduit_body::BodyId,
    root: &[u8; 32],
    custody: Option<archive::Custody<'_>>,
) -> Result<alloc::boxed::Box<archive::Resident>, &'static str> {
    let key = ed25519_dalek::VerifyingKey::from_bytes(root).map_err(|_| "restart-root-key")?;
    let (pending, framing) = with_plain_framing(read, |plain| {
        archive::decode_with_prefix(plain, body, Some(&key), custody)
    })?;
    pending
        .with_framing(framing)?
        .validate_boxed(body, Some(&key))
}
/// Export only the archive array from an already validated affine snapshot.
/// Output is provisional until the caller's authenticated read_snapshot returns
/// successfully. Always consume the complete source, including private suffix
/// bytes, so a truncated or changed checkpoint cannot become a custody offer.
pub(crate) fn export_archive_array<E>(
    read: impl FnMut(&mut [u8; 128]) -> Result<usize, ()>,
    span: archive::Span,
    sink: &mut dyn FnMut(&[u8]) -> Result<(), E>,
) -> Result<(), StreamRefusal<E>> {
    let mut failure = None;
    let result = with_plain(read, |source| {
        let mut observer = archive::SpanObserver::default();
        let mut offset = 0usize;
        let mut chunk = [0; 128];
        let result = (|| {
            sink(b"[").map_err(|error| {
                failure = Some(error);
                "restart-export-sink"
            })?;
            loop {
                let n = source(&mut chunk).map_err(|_| "restart-export-read")?;
                if n > chunk.len() {
                    return Err("restart-export-read");
                }
                if n == 0 {
                    break;
                }
                observer
                    .push(&chunk[..n])
                    .map_err(|_| "restart-export-span")?;
                let end_offset = offset.checked_add(n).ok_or("restart-export-length")?;
                let start = offset.max(span.start);
                let end = end_offset.min(span.end);
                if start < end {
                    sink(&chunk[start - offset..end - offset]).map_err(|error| {
                        failure = Some(error);
                        "restart-export-sink"
                    })?;
                }
                offset = end_offset;
            }
            if observer.finish()? != span {
                return Err("restart-export-span");
            }
            sink(b"]").map_err(|error| {
                failure = Some(error);
                "restart-export-sink"
            })
        })();
        chunk.fill(0);
        result
    });
    if let Some(error) = failure {
        return Err(StreamRefusal::Sink(error));
    }
    result.map_err(|_| StreamRefusal::Invalid)
}
pub fn decode_streamed(
    read: impl FnMut(&mut [u8; 128]) -> Result<usize, ()>,
    expected_body: &conduit_body::BodyId,
) -> Result<Retained, &'static str> {
    with_plain(read, |plain| decode_plain(plain, expected_body))
}
/// Inspect authenticated input without allocating any biography or archive.
/// This is a capacity preflight only; typed canonical validation still follows.
pub fn plain_length(
    read: impl FnMut(&mut [u8; 128]) -> Result<usize, ()>,
) -> Result<usize, &'static str> {
    let mut prefixed = PrefixedReader::new(read)?;
    let mut chunk = [0; 128];
    let result = (|| {
        let mut count = 0usize;
        if prefixed.length != Some(0) && prefixed.prefix[0] == b'F' {
            with_framed_plain(&mut |out| prefixed.read(out), &mut |read| {
                loop {
                    let n = read(&mut chunk).map_err(|_| "restart-frame")?;
                    if n == 0 {
                        break;
                    }
                    count = count.checked_add(n).ok_or("restart-capacity")?;
                }
                Ok(())
            })?;
        } else if prefixed.length != Some(0) && prefixed.prefix[0] == b'B' {
            let mut source = |out: &mut [u8; 128]| prefixed.read(out);
            let source: &mut dyn FnMut(&mut [u8; 128]) -> Result<usize, ()> = &mut source;
            let mut decoder = compress::Decoder::new(source, MAX_PLAIN_BYTES, MAX_BYTES);
            loop {
                let n = decoder
                    .read(&mut chunk)
                    .map_err(|_| "restart-compression")?;
                if n == 0 {
                    break;
                }
                count = count.checked_add(n).ok_or("restart-capacity")?;
            }
        } else {
            loop {
                let n = prefixed.read(&mut chunk).map_err(|_| "restart-read")?;
                if n > 128 {
                    return Err("restart-read");
                }
                if n == 0 {
                    break;
                }
                count = count.checked_add(n).ok_or("restart-capacity")?;
                if count > MAX_BYTES {
                    return Err("restart-capacity");
                }
            }
        }
        Ok(count)
    })();
    chunk.fill(0);
    result
}
fn validate_retained(
    retained: Retained,
    expected_body: &conduit_body::BodyId,
) -> Result<Retained, &'static str> {
    if retained.schema != SCHEMA || &retained.evidence.body_id != expected_body {
        return Err("restart-body-binding");
    }
    validate(&retained.evidence, &retained.archives)?;
    if retained
        .admissions
        .as_ref()
        .is_some_and(|manager| manager.body_id != retained.evidence.body_id)
    {
        return Err("restart-admission-body");
    }
    Ok(retained)
}

#[path = "native_restart_json.rs"]
pub(crate) mod json;
#[derive(Debug)]
pub enum StreamRefusal<E> {
    Sink(E),
    Invalid,
    Capacity,
}
/// Serialize the entire canonical envelope without a document allocation.
/// The caller owns bounded staging and must not publish before journal commit.
pub fn stream<E>(
    evidence: &BodyBiographyEvidence,
    archives: &[BodyBiographyArchiveSegment],
    pending: &[BodyBiographyArchiveSegment],
    sink: &mut dyn FnMut(&[u8]) -> Result<(), E>,
) -> Result<(), StreamRefusal<E>> {
    stream_with_admissions(evidence, archives, pending, None, sink)
}

pub fn stream_with_admissions<E>(
    evidence: &BodyBiographyEvidence,
    archives: &[BodyBiographyArchiveSegment],
    pending: &[BodyBiographyArchiveSegment],
    admissions: Option<&AdmissionManager>,
    sink: &mut dyn FnMut(&[u8]) -> Result<(), E>,
) -> Result<(), StreamRefusal<E>> {
    stream_with_admissions_and_network(evidence, archives, pending, admissions, None, sink)
}

pub fn stream_with_admissions_and_network<E>(
    evidence: &BodyBiographyEvidence,
    archives: &[BodyBiographyArchiveSegment],
    pending: &[BodyBiographyArchiveSegment],
    admissions: Option<&AdmissionManager>,
    network: Option<&network::Cursor>,
    sink: &mut dyn FnMut(&[u8]) -> Result<(), E>,
) -> Result<(), StreamRefusal<E>> {
    validate_chain(evidence, archives, pending).map_err(|_| StreamRefusal::Invalid)?;
    if admissions.is_some_and(|manager| manager.body_id != evidence.body_id) {
        return Err(StreamRefusal::Invalid);
    }
    let snapshot = Snapshot {
        schema: if network.is_some() { SCHEMA_V4 } else { SCHEMA },
        network_cursor: network,
        evidence: evidence.into(),
        archives: Archives(archives, pending),
        admissions,
    };
    let mut failure = None;
    let result = json::stream(&snapshot, &mut |bytes: &[u8]| {
        sink(bytes).map_err(|error| {
            failure = Some(error);
            json::CodecError::Sink
        })
    });
    if let Some(error) = failure {
        return Err(StreamRefusal::Sink(error));
    }
    result.map_err(|_| StreamRefusal::Invalid)
}

/// Losslessly frame complete canonical JSON inside the authenticated journal.
/// Encoder state is bounded; both deterministic journal passes start fresh.
pub fn stream_compressed_with_admissions<E>(
    evidence: &BodyBiographyEvidence,
    archives: &[BodyBiographyArchiveSegment],
    pending: &[BodyBiographyArchiveSegment],
    admissions: Option<&AdmissionManager>,
    sink: &mut dyn FnMut(&[u8]) -> Result<(), E>,
) -> Result<(), StreamRefusal<E>> {
    stream_compressed_with_admissions_and_network(
        evidence, archives, pending, admissions, None, sink,
    )
}

pub fn stream_compressed_with_admissions_and_network<E>(
    evidence: &BodyBiographyEvidence,
    archives: &[BodyBiographyArchiveSegment],
    pending: &[BodyBiographyArchiveSegment],
    admissions: Option<&AdmissionManager>,
    network: Option<&network::Cursor>,
    sink: &mut dyn FnMut(&[u8]) -> Result<(), E>,
) -> Result<(), StreamRefusal<E>> {
    let mut failure = None;
    let mut capacity = false;
    let result = {
        let mut encoder = compress::Encoder::new(
            |bytes: &[u8]| {
                sink(bytes).map_err(|error| {
                    failure = Some(error);
                })
            },
            MAX_PLAIN_BYTES,
            MAX_BYTES,
        );
        let result = stream_with_admissions_and_network(
            evidence,
            archives,
            pending,
            admissions,
            network,
            &mut |bytes: &[u8]| {
                encoder.write(bytes).map_err(|error| {
                    capacity |= error == compress::Error::Capacity;
                })
            },
        );
        if result.is_err() {
            Err(())
        } else {
            encoder.finish().map(|_| ()).map_err(|error| {
                capacity |= error == compress::Error::Capacity;
            })
        }
    };
    if let Some(error) = failure {
        return Err(StreamRefusal::Sink(error));
    }
    result.map_err(|_| {
        if capacity {
            StreamRefusal::Capacity
        } else {
            StreamRefusal::Invalid
        }
    })
}

/// Rewrite current truth while preserving the exact prior authenticated archive
/// span. The caller must own the affine journal snapshot that validated it.
pub(crate) fn stream_rewrite<E>(
    evidence: &BodyBiographyEvidence,
    pending: &[BodyBiographyArchiveSegment],
    admissions: &AdmissionManager,
    previous: &archive::Summary,
    previous_span: archive::Span,
    read: impl FnMut(&mut [u8; 128]) -> Result<usize, ()>,
    sink: &mut dyn FnMut(&[u8]) -> Result<(), E>,
) -> Result<(archive::Summary, archive::Span), StreamRefusal<E>> {
    stream_rewrite_inner(
        evidence,
        pending,
        admissions,
        previous,
        previous_span,
        None,
        previous.count,
        None,
        &layout::Layouts::default(),
        None,
        read,
        sink,
    )
}
/// Rewrite a verified external prefix and its exact locally retained suffix.
/// To offload all local archives, the Store supplies its newly Root-endorsed
/// receipt for the existing complete summary and local_count=0. The original
/// source is still consumed completely; only its archive bytes are omitted.
#[allow(clippy::too_many_arguments)]
pub(crate) fn stream_rewrite_with_prefix<E>(
    evidence: &BodyBiographyEvidence,
    pending: &[BodyBiographyArchiveSegment],
    admissions: &AdmissionManager,
    previous: &archive::Summary,
    previous_span: archive::Span,
    prefix: Option<&wire::Receipt>,
    previous_local_count: u64,
    root: &[u8; 32],
    read: impl FnMut(&mut [u8; 128]) -> Result<usize, ()>,
    sink: &mut dyn FnMut(&[u8]) -> Result<(), E>,
) -> Result<(archive::Summary, archive::Span), StreamRefusal<E>> {
    stream_rewrite_with_layout(
        evidence,
        pending,
        admissions,
        previous,
        previous_span,
        prefix,
        previous_local_count,
        root,
        &layout::Layouts::default(),
        read,
        sink,
    )
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn stream_rewrite_with_layout<E>(
    evidence: &BodyBiographyEvidence,
    pending: &[BodyBiographyArchiveSegment],
    admissions: &AdmissionManager,
    previous: &archive::Summary,
    previous_span: archive::Span,
    prefix: Option<&wire::Receipt>,
    previous_local_count: u64,
    root: &[u8; 32],
    layouts: &layout::Layouts,
    read: impl FnMut(&mut [u8; 128]) -> Result<usize, ()>,
    sink: &mut dyn FnMut(&[u8]) -> Result<(), E>,
) -> Result<(archive::Summary, archive::Span), StreamRefusal<E>> {
    stream_rewrite_with_network(
        evidence,
        pending,
        admissions,
        previous,
        previous_span,
        prefix,
        previous_local_count,
        root,
        layouts,
        None,
        read,
        sink,
    )
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn stream_rewrite_with_network<E>(
    evidence: &BodyBiographyEvidence,
    pending: &[BodyBiographyArchiveSegment],
    admissions: &AdmissionManager,
    previous: &archive::Summary,
    previous_span: archive::Span,
    prefix: Option<&wire::Receipt>,
    previous_local_count: u64,
    root: &[u8; 32],
    layouts: &layout::Layouts,
    network: Option<&network::Cursor>,
    read: impl FnMut(&mut [u8; 128]) -> Result<usize, ()>,
    sink: &mut dyn FnMut(&[u8]) -> Result<(), E>,
) -> Result<(archive::Summary, archive::Span), StreamRefusal<E>> {
    stream_rewrite_inner(
        evidence,
        pending,
        admissions,
        previous,
        previous_span,
        prefix,
        previous_local_count,
        Some(root),
        layouts,
        network,
        read,
        sink,
    )
}
#[allow(clippy::too_many_arguments)]
fn stream_rewrite_inner<E>(
    evidence: &BodyBiographyEvidence,
    pending: &[BodyBiographyArchiveSegment],
    admissions: &AdmissionManager,
    previous: &archive::Summary,
    previous_span: archive::Span,
    prefix: Option<&wire::Receipt>,
    previous_local_count: u64,
    root: Option<&[u8; 32]>,
    layouts: &layout::Layouts,
    network: Option<&network::Cursor>,
    read: impl FnMut(&mut [u8; 128]) -> Result<usize, ()>,
    sink: &mut dyn FnMut(&[u8]) -> Result<(), E>,
) -> Result<(archive::Summary, archive::Span), StreamRefusal<E>> {
    let key = if prefix.is_some() || !layouts.is_empty() || network.is_some() {
        Some(
            ed25519_dalek::VerifyingKey::from_bytes(root.ok_or(StreamRefusal::Invalid)?)
                .map_err(|_| StreamRefusal::Invalid)?,
        )
    } else {
        None
    };
    let _checked = rewrite::ValidatedReceipts::verify_network(
        &evidence.body_id,
        key.as_ref(),
        prefix,
        layouts,
        network,
    )
    .map_err(|_| StreamRefusal::Invalid)?;
    let summary = validate_rewrite_state(
        evidence,
        pending,
        admissions,
        previous,
        prefix,
        previous_local_count,
    )?;
    emit_rewrite(
        evidence,
        pending,
        admissions,
        previous,
        previous_span,
        prefix,
        previous_local_count,
        layouts,
        network,
        summary,
        read,
        sink,
    )
}
fn validate_rewrite_state<E>(
    evidence: &BodyBiographyEvidence,
    pending: &[BodyBiographyArchiveSegment],
    admissions: &AdmissionManager,
    previous: &archive::Summary,
    prefix: Option<&wire::Receipt>,
    previous_local_count: u64,
) -> Result<archive::Summary, StreamRefusal<E>> {
    if previous
        .count
        .checked_sub(prefix.map_or(0, |p| p.summary.count))
        != Some(previous_local_count)
    {
        return Err(StreamRefusal::Invalid);
    }
    if let Some(prefix) = prefix {
        if previous_local_count == 0
            && (previous.body.as_ref() != Some(&prefix.body)
                || previous.head != prefix.summary.head.map(|h| h.0)
                || previous.last_sequence != prefix.summary.last_sequence
                || previous.last_sign.as_ref().map(|s| s.as_str())
                    != prefix.summary.last_sign.as_deref())
        {
            return Err(StreamRefusal::Invalid);
        }
    }
    evidence.validate().map_err(|_| StreamRefusal::Invalid)?;
    profile::ensure_no_holds(evidence, pending).map_err(|_| StreamRefusal::Invalid)?;
    if admissions.body_id != evidence.body_id {
        return Err(StreamRefusal::Invalid);
    }
    let mut summary = previous.clone();
    for segment in pending {
        summary
            .append(segment)
            .map_err(|_| StreamRefusal::Invalid)?;
    }
    summary
        .validate_head(evidence)
        .map_err(|_| StreamRefusal::Invalid)?;
    Ok(summary)
}
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn emit_rewrite<E>(
    evidence: &BodyBiographyEvidence,
    pending: &[BodyBiographyArchiveSegment],
    admissions: &AdmissionManager,
    _previous: &archive::Summary,
    previous_span: archive::Span,
    prefix: Option<&wire::Receipt>,
    previous_local_count: u64,
    layouts: &layout::Layouts,
    network: Option<&network::Cursor>,
    summary: archive::Summary,
    read: impl FnMut(&mut [u8; 128]) -> Result<usize, ()>,
    sink: &mut dyn FnMut(&[u8]) -> Result<(), E>,
) -> Result<(archive::Summary, archive::Span), StreamRefusal<E>> {
    let mut failure = None;
    let mut capacity = false;
    let mut observer = archive::SpanObserver::default();
    let result = (|| {
        let mut encoder = compress::Encoder::new(
            |bytes: &[u8]| {
                sink(bytes).map_err(|error| {
                    failure = Some(error);
                })
            },
            MAX_PLAIN_BYTES,
            MAX_BYTES,
        );
        let mut plain = |bytes: &[u8]| -> Result<(), ()> {
            observer.push(bytes)?;
            encoder.write(bytes).map_err(|error| {
                capacity |= error == compress::Error::Capacity;
            })
        };
        plain(b"{\"schema\":")?;
        json::stream(
            &if network.is_some() {
                SCHEMA_V4
            } else if !layouts.is_empty() {
                SCHEMA_V3
            } else if prefix.is_some() {
                SCHEMA_V2
            } else {
                SCHEMA
            },
            &mut |bytes: &[u8]| plain(bytes).map_err(|_| json::CodecError::Sink),
        )
        .map_err(|_| ())?;
        plain(b",\"evidence\":")?;
        json::stream(
            &profile::EvidenceRef::from(evidence),
            &mut |bytes: &[u8]| plain(bytes).map_err(|_| json::CodecError::Sink),
        )
        .map_err(|_| ())?;
        if let Some(prefix) = prefix {
            plain(b",\"biography_prefix\":")?;
            json::stream(prefix, &mut |bytes: &[u8]| {
                plain(bytes).map_err(|_| json::CodecError::Sink)
            })
            .map_err(|_| ())?;
        }
        if !layouts.is_empty() {
            plain(b",\"observation_layouts\":")?;
            json::stream(layouts, &mut |bytes: &[u8]| {
                plain(bytes).map_err(|_| json::CodecError::Sink)
            })
            .map_err(|_| ())?;
        }
        if let Some(network) = network {
            plain(b",\"network_cursor\":")?;
            json::stream(network, &mut |bytes: &[u8]| {
                plain(bytes).map_err(|_| json::CodecError::Sink)
            })
            .map_err(|_| ())?;
        }
        plain(b",\"archives\":[")?;
        with_plain(read, |source| {
            let mut offset = 0usize;
            let mut chunk = [0; 128];
            let result = (|| {
                loop {
                    let n = source(&mut chunk).map_err(|_| "restart-source-read")?;
                    if n > 128 {
                        return Err("restart-source-read");
                    }
                    if n == 0 {
                        break;
                    }
                    let start = offset.max(previous_span.start);
                    let end = (offset + n).min(previous_span.end);
                    if previous_local_count != 0 && start < end {
                        plain(&chunk[start - offset..end - offset])
                            .map_err(|_| "restart-source-write")?;
                    }
                    offset = offset.checked_add(n).ok_or("restart-source-length")?;
                }
                if previous_span.end > offset {
                    return Err("restart-source-span");
                }
                Ok(())
            })();
            chunk.fill(0);
            result
        })
        .map_err(|_| ())?;
        let mut comma = previous_local_count != 0;
        for segment in pending {
            if comma {
                plain(b",")?;
            }
            comma = true;
            json::stream(
                &profile::ArchiveRef::from(segment),
                &mut |bytes: &[u8]| plain(bytes).map_err(|_| json::CodecError::Sink),
            )
            .map_err(|_| ())?;
        }
        plain(b"],\"admissions\":")?;
        json::stream(admissions, &mut |bytes: &[u8]| {
            plain(bytes).map_err(|_| json::CodecError::Sink)
        })
        .map_err(|_| ())?;
        plain(b"}")?;
        encoder.finish().map_err(|error| {
            capacity |= error == compress::Error::Capacity;
        })?;
        Ok::<_, ()>(())
    })();
    if let Some(error) = failure {
        return Err(StreamRefusal::Sink(error));
    }
    result.map_err(|_| {
        if capacity {
            StreamRefusal::Capacity
        } else {
            StreamRefusal::Invalid
        }
    })?;
    if observer.segments
        != previous_local_count
            .checked_add(pending.len() as u64)
            .ok_or(StreamRefusal::Invalid)?
    {
        return Err(StreamRefusal::Invalid);
    }
    Ok((
        summary,
        observer.finish().map_err(|_| StreamRefusal::Invalid)?,
    ))
}

pub(crate) struct ResidentExtent {
    pub span: archive::Span,
    pub segments: u64,
    pub active: usize,
    pub largest: usize,
}
pub(crate) fn resident_extent(
    read: impl FnMut(&mut [u8; 128]) -> Result<usize, ()>,
) -> Result<ResidentExtent, &'static str> {
    with_plain(read, |plain| {
        let mut observer = archive::SpanObserver::default();
        let mut chunk = [0; 128];
        let result = (|| {
            let mut total = 0usize;
            loop {
                let n = plain(&mut chunk).map_err(|_| "restart-preflight")?;
                if n > 128 {
                    return Err("restart-preflight");
                }
                if n == 0 {
                    break;
                }
                observer
                    .push(&chunk[..n])
                    .map_err(|_| "restart-preflight")?;
                total = total.checked_add(n).ok_or("restart-capacity")?;
                if total > MAX_PLAIN_BYTES {
                    return Err("restart-capacity");
                }
            }
            let span = observer.finish()?;
            let active = total
                .checked_sub(span.end.checked_sub(span.start).ok_or("restart-span")?)
                .ok_or("restart-span")?;
            Ok(ResidentExtent {
                span,
                segments: observer.segments,
                active,
                largest: observer.largest_segment,
            })
        })();
        chunk.fill(0);
        result
    })
}

#[cfg(test)]
#[path = "native_restart_prefix_tests.rs"]
mod prefix_tests;

pub(super) fn authenticate_receipts(
    resident: &mut archive::Resident,
    source: &crate::native_body_checkpoint::SourceSnapshot,
) -> Result<(), &'static str> {
    let checked = resident
        .validation
        .take()
        .ok_or("restart-receipt-validation-missing")?;
    resident.proof =
        Some(checked.bind_framing(source, resident.framing.take(), resident.local_count)?);
    Ok(())
}
