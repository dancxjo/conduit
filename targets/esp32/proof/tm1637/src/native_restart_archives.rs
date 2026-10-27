//! Private validated archive summaries and exact JSON spans. A summary alone
//! grants no custody: the native Store pairs it with an affine journal snapshot.
use super::{profile, Retained, SCHEMA};
use alloc::{string::String, vec::Vec};
use conduit_body::*;
use serde::{
    de::{self, SeqAccess, Visitor},
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
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckedEnvelope {
    schema: String,
    #[serde(deserialize_with = "profile::deserialize_evidence")]
    evidence: BodyBiographyEvidence,
    archives: Summary,
    #[serde(default)]
    admissions: Option<AdmissionManager>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CustodiedEnvelope {
    schema: String,
    #[serde(deserialize_with = "profile::deserialize_evidence")]
    evidence: BodyBiographyEvidence,
    archives: de::IgnoredAny,
    #[serde(default)]
    admissions: Option<AdmissionManager>,
}
pub(crate) struct Resident {
    pub retained: Retained,
    pub summary: Summary,
    pub span: Span,
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
pub(crate) fn decode(
    read: &mut dyn FnMut(&mut [u8; 128]) -> Result<usize, ()>,
    expected_body: &BodyId,
    custody: Option<&Summary>,
) -> Result<Resident, &'static str> {
    let mut observer = SpanObserver::default();
    let mut observed = |chunk: &mut [u8; 128]| {
        let n = read(chunk)?;
        if n > 128 {
            return Err(());
        }
        observer.push(&chunk[..n])?;
        Ok(n)
    };
    let (schema, evidence, admissions, summary) = if let Some(summary) = custody {
        let envelope: CustodiedEnvelope =
            super::read_json::from_reader(&mut observed, super::MAX_PLAIN_BYTES)
                .map_err(|_| "restart-encoding")?;
        let _ = envelope.archives;
        (
            envelope.schema,
            envelope.evidence,
            envelope.admissions,
            summary.clone(),
        )
    } else {
        let envelope: CheckedEnvelope =
            super::read_json::from_reader(&mut observed, super::MAX_PLAIN_BYTES)
                .map_err(|_| "restart-encoding")?;
        (
            envelope.schema,
            envelope.evidence,
            envelope.admissions,
            envelope.archives,
        )
    };
    if schema != SCHEMA || &evidence.body_id != expected_body {
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
    Ok(Resident {
        retained: Retained {
            schema,
            evidence,
            archives: Vec::new(),
            admissions,
        },
        summary,
        span: observer.finish()?,
    })
}
