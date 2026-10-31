//! Private validated archive summaries and exact JSON spans. A summary alone
//! grants no custody: the native Store pairs it with an affine journal snapshot.
use super::{profile, Retained, SCHEMA};
use alloc::{boxed::Box, string::String, vec::Vec};
use conduit_body::*;
use serde::{
    de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor},
    Deserialize, Deserializer,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Summary {
    pub count: u64,
    pub body: Option<BodyId>,
    pub head: Option<[u8; 32]>,
    pub last_sequence: u64,
    pub last_sign: Option<conduit_core::SignId>,
}
impl Summary {
    pub fn append(&mut self, segment: &BodyBiographyArchiveSegment) -> Result<(), &'static str> {
        segment.validate().map_err(|_| "restart-archive")?;
        profile::ensure_archive_no_holds(segment)?;
        if segment.ordinal
            != self
                .count
                .checked_add(1)
                .ok_or("restart-archive-capacity")?
            || segment.previous_digest != self.head
            || self
                .body
                .as_ref()
                .is_some_and(|body| body != &segment.body_id)
        {
            return Err("restart-archive-chain");
        }
        self.count = segment.ordinal;
        self.body = Some(segment.body_id.clone());
        self.head = Some(segment.digest);
        let record = segment.records.last().ok_or("restart-archive")?;
        self.last_sequence = record.sequence;
        self.last_sign = Some(record.sign_id.clone());
        Ok(())
    }
    pub fn validate_head(&self, evidence: &BodyBiographyEvidence) -> Result<(), &'static str> {
        if self.count == 0 {
            if evidence
                .compaction
                .as_ref()
                .is_some_and(|c| c.sealed_segments != 0 || c.archive_head_digest.is_some())
            {
                return Err("restart-missing-archive");
            }
            return Ok(());
        }
        let compact = evidence.compaction.as_ref().ok_or("restart-archive-head")?;
        if self.body.as_ref() != Some(&evidence.body_id)
            || self.count != compact.sealed_segments
            || self.head != compact.archive_head_digest
            || self.last_sequence > compact.through_sequence
            || (self.last_sequence == compact.through_sequence
                && self.last_sign.as_ref() != Some(&compact.through_sign_id))
        {
            return Err("restart-archive-head");
        }
        Ok(())
    }
}
struct OneArchive(BodyBiographyArchiveSegment);
impl<'de> Deserialize<'de> for OneArchive {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        profile::deserialize_archive(d).map(Self)
    }
}
impl<'de> Deserialize<'de> for Summary {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Archives;
        impl<'de> Visitor<'de> for Archives {
            type Value = Summary;
            fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str("complete ordered archive chain")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Summary, A::Error> {
                let mut summary = Summary::default();
                while let Some(OneArchive(segment)) = sequence.next_element()? {
                    summary.append(&segment).map_err(de::Error::custom)?;
                }
                Ok(summary)
            }
        }
        d.deserialize_seq(Archives)
    }
}
/// Borrowed already authenticated source metadata for parked restoration. It is
/// not reconstructible from incoming JSON: the owning Store pins its snapshot.
pub(crate) struct Custody<'a> {
    pub summary: &'a Summary,
    pub prefix: Option<&'a super::wire::Receipt>,
    pub local_count: u64,
}
pub(crate) struct Resident {
    pub(super) framing: Option<super::frames::Validated>,
    pub(crate) proof: Option<super::rewrite::AuthenticatedReceipts>,
    pub(super) validation: Option<super::rewrite::ValidatedReceipts>,
    pub layouts: super::layout::Layouts,
    pub network: Option<super::network::Cursor>,
    pub retained: Retained,
    pub summary: Summary,
    pub prefix: Option<super::wire::Receipt>,
    pub local_count: u64,
    pub span: Span,
}
// Parsed custody remains private until the shallower post-decode signature
// boundary succeeds. Neither this wrapper nor its contents can authorize restore.
pub(super) struct PendingResident(Box<Resident>);
impl PendingResident {
    pub(super) fn with_framing(
        mut self,
        framing: Option<super::frames::Parsed>,
    ) -> Result<Self, &'static str> {
        self.0.framing = framing.map(|f| f.validate(self.0.span)).transpose()?;
        Ok(self)
    }
    #[inline(never)]
    pub(super) fn validate_receipts(
        self,
        body: &BodyId,
        key: Option<&ed25519_dalek::VerifyingKey>,
    ) -> Result<Resident, &'static str> {
        self.validate_boxed(body, key).map(|resident| *resident)
    }
    #[inline(never)]
    pub(super) fn validate_boxed(
        self,
        body: &BodyId,
        key: Option<&ed25519_dalek::VerifyingKey>,
    ) -> Result<Box<Resident>, &'static str> {
        let mut resident = self.0;
        resident.validation = Some(super::rewrite::ValidatedReceipts::verify_network(
            body,
            key,
            resident.prefix.as_ref(),
            &resident.layouts,
            resident.network.as_ref(),
        )?);
        Ok(resident)
    }
}
struct Evidence(BodyBiographyEvidence);
impl<'de> Deserialize<'de> for Evidence {
    #[inline(never)]
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        profile::deserialize_evidence(d).map(Self)
    }
}
struct Suffix(Summary);
impl<'de> Visitor<'de> for Suffix {
    type Value = Summary;
    fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("archive suffix after authenticated prefix")
    }
    fn visit_seq<A: SeqAccess<'de>>(mut self, mut sequence: A) -> Result<Summary, A::Error> {
        while let Some(OneArchive(segment)) = sequence.next_element()? {
            self.0.append(&segment).map_err(de::Error::custom)?;
        }
        Ok(self.0)
    }
}
impl<'de> DeserializeSeed<'de> for Suffix {
    type Value = Summary;
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Summary, D::Error> {
        d.deserialize_seq(self)
    }
}
fn prefix_summary(body: &BodyId, prefix: &super::wire::Receipt) -> Summary {
    Summary {
        count: prefix.summary.count,
        body: Some(body.clone()),
        head: prefix.summary.head.map(|v| v.0),
        last_sequence: prefix.summary.last_sequence,
        last_sign: prefix.summary.last_sign.clone().map(Into::into),
    }
}
// Keep rich DTO decoding out of the envelope dispatch so canonical readers
// share one implementation instead of cloning it into every field branch.
#[inline(never)]
fn read_admissions<'de, A: MapAccess<'de>>(
    map: &mut A,
) -> Result<Option<AdmissionManager>, A::Error> {
    map.next_value()
}
#[inline(never)]
fn read_receipt<'de, A: MapAccess<'de>>(map: &mut A) -> Result<super::wire::Receipt, A::Error> {
    map.next_value()
}
struct Envelope<'a> {
    body: &'a BodyId,
    key: Option<&'a ed25519_dalek::VerifyingKey>,
    custody: Option<Custody<'a>>,
}
struct Decoded {
    layouts: super::layout::Layouts,
    network: Option<super::network::Cursor>,
    schema: String,
    evidence: BodyBiographyEvidence,
    admissions: Option<AdmissionManager>,
    summary: Summary,
    prefix: Option<super::wire::Receipt>,
}
impl<'de> DeserializeSeed<'de> for Envelope<'_> {
    type Value = Decoded;
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Decoded, D::Error> {
        d.deserialize_map(self)
    }
}
impl<'de> Visitor<'de> for Envelope<'_> {
    type Value = Decoded;
    fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("complete restart envelope with authenticated archive custody")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Decoded, A::Error> {
        let (mut schema, mut evidence, mut summary, mut prefix, mut admissions) =
            (None, None, None, None, None);
        let mut admissions_seen = false;
        let mut layouts = None;
        let mut network = None;
        while let Some(field) = map.next_key::<String>()? {
            match field.as_str() {
                "schema" if schema.is_none() => schema = Some(map.next_value::<String>()?),
                "evidence" if evidence.is_none() => {
                    evidence = Some(map.next_value::<Evidence>()?.0)
                }
                "biography_prefix" if prefix.is_none() => {
                    if summary.is_some() {
                        return Err(de::Error::custom("prefix must precede archive suffix"));
                    }
                    let receipt = read_receipt(&mut map)?;
                    self.key
                        .ok_or_else(|| de::Error::custom("prefix requires pinned Root"))?;
                    // Only provisional chain accounting until the shallower
                    // post-EOF receipt boundary verifies the signature.
                    prefix = Some(receipt);
                }
                "archives" if summary.is_none() => {
                    summary = Some(if let Some(custody) = self.custody.as_ref() {
                        let _: de::IgnoredAny = map.next_value()?;
                        custody.summary.clone()
                    } else {
                        let initial = prefix
                            .as_ref()
                            .map_or_else(Summary::default, |p| prefix_summary(self.body, p));
                        map.next_value_seed(Suffix(initial))?
                    });
                }
                "observation_layouts" if layouts.is_none() => {
                    let raw: super::layout::Layouts = map.next_value()?;
                    // Preserve the no-key refusal even for an empty @3 array.
                    if self.key.is_none() {
                        return Err(de::Error::custom("layouts require pinned Root"));
                    }
                    layouts = Some(raw);
                }
                "network_cursor" if network.is_none() => {
                    self.key
                        .ok_or_else(|| de::Error::custom("network cursor requires pinned Root"))?;
                    network = Some(map.next_value::<super::network::Cursor>()?);
                }
                "admissions" if !admissions_seen => {
                    admissions_seen = true;
                    admissions = read_admissions(&mut map)?;
                }
                _ => return Err(de::Error::custom("unknown or duplicate restart field")),
            }
        }
        let schema = schema.ok_or_else(|| de::Error::custom("restart schema missing"))?;
        if (schema == SCHEMA && prefix.is_some())
            || (schema == super::SCHEMA_V2 && prefix.is_none())
            || (schema != SCHEMA
                && schema != super::SCHEMA_V2
                && schema != super::SCHEMA_V3
                && schema != super::SCHEMA_V4)
            || (schema == super::SCHEMA_V3 && layouts.is_none())
            || (schema != super::SCHEMA_V3 && schema != super::SCHEMA_V4 && layouts.is_some())
            || ((schema == super::SCHEMA_V4) != network.is_some())
        {
            return Err(de::Error::custom("restart schema/prefix mismatch"));
        }
        if let Some(custody) = self.custody.as_ref() {
            let same = match (prefix.as_ref(), custody.prefix, self.key) {
                (None, None, _) => true,
                (Some(actual), Some(expected), Some(key)) => {
                    actual.transcript(&key.to_bytes()) == expected.transcript(&key.to_bytes())
                        && actual.signature == expected.signature
                }
                _ => false,
            };
            if !same {
                return Err(de::Error::custom("restart retained prefix changed"));
            }
        }
        Ok(Decoded {
            layouts: layouts.unwrap_or_default(),
            network,
            schema,
            evidence: evidence.ok_or_else(|| de::Error::custom("restart evidence missing"))?,
            admissions,
            summary: summary.ok_or_else(|| de::Error::custom("restart archives missing"))?,
            prefix,
        })
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Span {
    pub start: usize,
    pub end: usize,
}

// Decode only the ASCII key we recognize, without a String or token allocation.
fn archive_key(token: &[u8]) -> Result<bool, ()> {
    let bytes = token
        .strip_prefix(b"\"")
        .and_then(|b| b.strip_suffix(b"\""))
        .ok_or(())?;
    let mut at = 0;
    let mut index = 0;
    let mut matches = true;
    while at < bytes.len() {
        let mut byte = bytes[at];
        at += 1;
        if byte == b'\\' {
            byte = *bytes.get(at).ok_or(())?;
            at += 1;
            byte = match byte {
                b'"' | b'\\' | b'/' => byte,
                b'b' => 8,
                b'f' => 12,
                b'n' => 10,
                b'r' => 13,
                b't' => 9,
                b'u' => {
                    let mut value = 0u16;
                    for _ in 0..4 {
                        let digit = *bytes.get(at).ok_or(())?;
                        at += 1;
                        let digit = match digit {
                            b'0'..=b'9' => digit - b'0',
                            b'a'..=b'f' => digit - b'a' + 10,
                            b'A'..=b'F' => digit - b'A' + 10,
                            _ => return Err(()),
                        };
                        value = value * 16 + u16::from(digit);
                    }
                    if value > 127 {
                        matches = false;
                        0
                    } else {
                        value as u8
                    }
                }
                _ => return Err(()),
            };
        }
        if b"archives".get(index) != Some(&byte) {
            matches = false;
        }
        index += 1;
    }
    Ok(matches && index == 8)
}
// This observer sees all uncompressed bytes, including parser read-ahead. The
// canonical typed decoder independently validates the whole document and EOF.
pub(crate) struct SpanObserver {
    at: usize,
    depth: usize,
    string: bool,
    escape: bool,
    key: bool,
    expect_key: bool,
    key_bytes: [u8; 64],
    key_len: usize,
    archive_key: bool,
    value: bool,
    archive_depth: Option<usize>,
    span: Option<Span>,
    segment_start: Option<usize>,
    pub largest_segment: usize,
    pub segments: u64,
}
impl Default for SpanObserver {
    fn default() -> Self {
        Self {
            at: 0,
            depth: 0,
            string: false,
            escape: false,
            key: false,
            expect_key: false,
            key_bytes: [0; 64],
            key_len: 0,
            archive_key: false,
            value: false,
            archive_depth: None,
            span: None,
            segment_start: None,
            largest_segment: 0,
            segments: 0,
        }
    }
}
impl Drop for SpanObserver {
    fn drop(&mut self) {
        self.key_bytes.fill(0);
    }
}
impl SpanObserver {
    pub fn push(&mut self, bytes: &[u8]) -> Result<(), ()> {
        for &byte in bytes {
            let offset = self.at;
            self.at = self.at.checked_add(1).ok_or(())?;
            if self.string {
                if self.key {
                    if self.key_len >= self.key_bytes.len() {
                        return Err(());
                    }
                    self.key_bytes[self.key_len] = byte;
                    self.key_len += 1;
                }
                if self.escape {
                    self.escape = false;
                    continue;
                }
                if byte == b'\\' {
                    self.escape = true;
                    continue;
                }
                if byte == b'"' {
                    self.string = false;
                    if self.key {
                        self.archive_key = archive_key(&self.key_bytes[..self.key_len])?;
                        self.expect_key = false;
                        self.key_bytes.fill(0);
                    }
                }
                continue;
            }
            if byte.is_ascii_whitespace() {
                continue;
            }
            if self.value {
                self.value = false;
                if self.archive_key {
                    if byte != b'[' || self.span.is_some() {
                        return Err(());
                    }
                    self.archive_depth = Some(self.depth + 1);
                    self.span = Some(Span {
                        start: offset + 1,
                        end: 0,
                    });
                }
            }
            if byte == b'{' && self.archive_depth == Some(self.depth) {
                self.segment_start = Some(offset);
            }
            if byte == b'}'
                && self
                    .archive_depth
                    .is_some_and(|depth| self.depth == depth + 1)
            {
                let start = self.segment_start.take().ok_or(())?;
                self.largest_segment = self.largest_segment.max(offset + 1 - start);
                self.segments = self.segments.checked_add(1).ok_or(())?;
            }
            match byte {
                b'"' => {
                    self.string = true;
                    self.key = self.depth == 1 && self.expect_key;
                    if self.key {
                        self.key_len = 1;
                        self.key_bytes[0] = b'"';
                    }
                }
                b'{' | b'[' => {
                    self.depth = self.depth.checked_add(1).ok_or(())?;
                    if self.depth == 1 {
                        self.expect_key = true;
                    }
                }
                b'}' | b']' => {
                    if self.archive_depth == Some(self.depth) {
                        if byte != b']' {
                            return Err(());
                        }
                        self.span.as_mut().ok_or(())?.end = offset;
                        self.archive_depth = None;
                    }
                    self.depth = self.depth.checked_sub(1).ok_or(())?;
                }
                b':' if self.depth == 1 => self.value = true,
                b',' if self.depth == 1 => self.expect_key = true,
                _ => {}
            }
        }
        Ok(())
    }
    pub fn finish(&self) -> Result<Span, &'static str> {
        let span = self.span.ok_or("restart-archive-span")?;
        if self.depth != 0 || self.string || self.archive_depth.is_some() || span.end < span.start {
            return Err("restart-archive-span");
        }
        Ok(span)
    }
}
#[inline(never)]
pub(super) fn decode(
    read: &mut dyn FnMut(&mut [u8; 128]) -> Result<usize, ()>,
    expected_body: &BodyId,
    custody: Option<&Summary>,
) -> Result<PendingResident, &'static str> {
    decode_with_prefix(
        read,
        expected_body,
        None,
        custody.map(|summary| Custody {
            summary,
            prefix: None,
            local_count: summary.count,
        }),
    )
}
#[inline(never)]
pub(super) fn decode_with_prefix(
    read: &mut dyn FnMut(&mut [u8; 128]) -> Result<usize, ()>,
    expected_body: &BodyId,
    key: Option<&ed25519_dalek::VerifyingKey>,
    custody: Option<Custody<'_>>,
) -> Result<PendingResident, &'static str> {
    let expected_local = custody.as_ref().map(|c| c.local_count);
    let mut observer = SpanObserver::default();
    let mut observed = |chunk: &mut [u8; 128]| {
        let n = read(chunk)?;
        if n > 128 {
            return Err(());
        }
        observer.push(&chunk[..n])?;
        Ok(n)
    };
    let decoded = super::read_json::from_reader_seed(
        &mut observed,
        super::MAX_PLAIN_BYTES,
        Envelope {
            body: expected_body,
            key,
            custody,
        },
    )
    .map_err(|_| "restart-encoding")?;
    finish_decoded(decoded, observer, expected_body, expected_local)
}

// Validation and final Resident construction do not surround the streaming
// serde recursion. The completed envelope moves into this later cold phase.
#[inline(never)]
fn finish_decoded(
    decoded: Decoded,
    observer: SpanObserver,
    expected_body: &BodyId,
    expected_local: Option<u64>,
) -> Result<PendingResident, &'static str> {
    let Decoded {
        layouts,
        network,
        schema,
        evidence,
        admissions,
        summary,
        prefix,
    } = decoded;
    if &evidence.body_id != expected_body {
        return Err("restart-body-binding");
    }
    profile::ensure_no_holds(&evidence, &[])?;
    evidence.validate().map_err(|_| "restart-biography")?;
    summary.validate_head(&evidence)?;
    if admissions
        .as_ref()
        .is_some_and(|a| a.body_id != evidence.body_id)
    {
        return Err("restart-admission-body");
    }
    let local_count = summary
        .count
        .checked_sub(prefix.as_ref().map_or(0, |p| p.summary.count))
        .ok_or("restart-local-count")?;
    if observer.segments != local_count || expected_local.is_some_and(|n| n != local_count) {
        return Err("restart-local-count");
    }
    // Allocate the final backing here so callers never retain another large
    // by-value Resident while this bounded parser is active.
    let layout = core::alloc::Layout::new::<Resident>();
    // SAFETY: this nonzero exact layout is initialized before any publication.
    let pointer = unsafe { alloc::alloc::alloc(layout) }.cast::<core::mem::MaybeUninit<Resident>>();
    if pointer.is_null() {
        return Err("restart-resident-backing-capacity");
    }
    // Finish the fallible span before creating an owning allocation.
    let span = match observer.finish() {
        Ok(span) => span,
        Err(error) => {
            // SAFETY: pointer is the unique uninitialized allocation above.
            unsafe { alloc::alloc::dealloc(pointer.cast(), layout) };
            return Err(error);
        }
    };
    // SAFETY: pointer is non-null, uniquely owned and has the exact Box layout.
    let backing = unsafe { Box::from_raw(pointer) };
    Ok(PendingResident(Box::write(
        backing,
        Resident {
            framing: None,
            validation: None,
            proof: None,
            layouts,
            network,
            retained: Retained {
                schema,
                evidence,
                archives: Vec::new(),
                admissions,
            },
            summary,
            prefix,
            local_count,
            span,
        },
    )))
}
