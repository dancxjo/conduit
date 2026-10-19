//! Allocation-free inspection of complete general evidence framing. Domain
//! payloads remain opaque claims; only independent report replay admits them.
use super::*;
mod model;
pub use model::*;
type E = ProjectionEvidenceReadRefusal;
pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], E> {
        let end = self.position.checked_add(n).ok_or(E::Extent)?;
        let bytes = self.bytes.get(self.position..end).ok_or(E::Malformed)?;
        self.position = end;
        Ok(bytes)
    }
    fn byte(&mut self) -> Result<u8, E> {
        Ok(self.take(1)?[0])
    }
    fn text(&mut self) -> Result<&'a str, E> {
        let n = u16::from_le_bytes(self.take(2)?.try_into().unwrap()) as usize;
        if n == 0 || n > 192 {
            return Err(E::Extent);
        }
        core::str::from_utf8(self.take(n)?).map_err(|_| E::Malformed)
    }
    fn optional<T>(
        &mut self,
        read: impl FnOnce(&mut Self) -> Result<T, E>,
    ) -> Result<Option<T>, E> {
        match self.byte()? {
            0 => Ok(None),
            1 => read(self).map(Some),
            _ => Err(E::Malformed),
        }
    }
    fn blob(&mut self, max: usize) -> Result<&'a [u8], E> {
        let n = u32::from_le_bytes(self.take(4)?.try_into().unwrap());
        let n = usize::try_from(n).map_err(|_| E::Extent)?;
        if n > max {
            return Err(E::Extent);
        }
        self.take(n)
    }
    fn section<T>(
        &mut self,
        max: usize,
        mut read: impl FnMut(&mut Self) -> Result<T, E>,
    ) -> Result<Section<'a>, E> {
        let count = self.byte()?;
        if usize::from(count) > max {
            return Err(E::Extent);
        }
        let start = self.position;
        for _ in 0..count {
            read(self)?;
        }
        Ok(Section {
            bytes: &self.bytes[start..self.position],
            count,
        })
    }
    fn terminal(&mut self) -> Result<crate::TerminalInfo, E> {
        crate::TerminalInfo::decode(self.take(crate::TERMINAL_INFO_ENCODED_LEN)?)
            .map_err(|_| E::Malformed)
    }
}
/// Decode the complete finite inventory without allocation or domain callbacks.
/// This does not construct ProjectionReport, authorize a target, or validate
/// opaque policy/source/target/detail bytes. Use verify_projection_evidence with
/// an independently reconstructed admitted report for semantic consumption.
pub fn inspect_projection_evidence(bytes: &[u8]) -> Result<ProjectionEvidenceView<'_>, E> {
    if bytes.len() > MAX_PROJECTION_EVIDENCE_BYTES {
        return Err(E::Extent);
    }
    let mut c = Cursor { bytes, position: 0 };
    if c.take(4)? != MAGIC {
        return Err(E::UnsupportedVersion);
    }
    let codec = c.text()?;
    let source_contract = c.text()?;
    let target_contract = c.text()?;
    let policy = c.text()?;
    let configuration_digest = c.take(32)?.try_into().unwrap();
    let report = c.text()?;
    let source_identity = c.text()?;
    let target_identity = c.optional(Cursor::text)?;
    let projector = c.text()?;
    let requested_route = c.text()?;
    let boundary = c.optional(|c| Ok((c.text()?, c.text()?)))?;
    let policy_bytes = c.blob(65536)?;
    let source_bytes = c.blob(262144)?;
    let target_bytes = c.optional(|c| c.blob(262144))?;
    if target_bytes.is_some() != target_identity.is_some() {
        return Err(E::Malformed);
    }
    let facts = c.section(MAX_PROJECTION_FACTS, fact)?;
    let native = c.section(MAX_PROJECTION_NATIVE_FACTS, native)?;
    let scores = c.section(MAX_PROJECTION_SCORES, score)?;
    let admitted = c.section(MAX_PROJECTION_ATTEMPTS, realization)?;
    let attempts = c.section(MAX_PROJECTION_ATTEMPTS, attempt)?;
    let selected_attempt = c.optional(|c| c.byte().map(usize::from))?;
    if selected_attempt.is_some_and(|i| i >= usize::from(attempts.count)) {
        return Err(E::Malformed);
    }
    let mechanism = mechanism(&mut c)?;
    let diagnostics = c.section(MAX_PROJECTION_DIAGNOSTICS, Cursor::text)?;
    let disposition = match c.byte()? {
        0 => ProjectionDisposition::Completed(ProjectionFidelity::Exact),
        1 => ProjectionDisposition::Completed(ProjectionFidelity::PermittedLossy),
        2 => ProjectionDisposition::Insufficient,
        3 => ProjectionDisposition::Refused(c.terminal()?),
        4 => ProjectionDisposition::Failed(c.terminal()?),
        5 => ProjectionDisposition::Cancelled(c.terminal()?),
        _ => return Err(E::Malformed),
    };
    let preserved = usize::from(c.byte()?);
    let transformed = usize::from(c.byte()?);
    let mut losses = [0; PROJECTION_LOSS_CLASSES];
    for n in &mut losses {
        *n = usize::from(c.byte()?);
    }
    if c.position != bytes.len() {
        return Err(E::Malformed);
    }
    let view = ProjectionEvidenceView {
        codec,
        source_contract,
        target_contract,
        policy,
        configuration_digest,
        report,
        source_identity,
        target_identity,
        projector,
        requested_route,
        boundary,
        policy_bytes,
        source_bytes,
        target_bytes,
        selected_attempt,
        mechanism,
        claimed_summary: ProjectionSummary {
            disposition,
            preserved,
            transformed,
            losses,
        },
        facts,
        native,
        scores,
        admitted,
        attempts,
        diagnostics,
    };
    let (mut actual_preserved, mut actual_transformed, mut actual_losses) =
        (0, 0, [0; PROJECTION_LOSS_CLASSES]);
    for f in view.facts() {
        match f {
            ProjectionEvidenceFact::Preserved { .. } => actual_preserved += 1,
            ProjectionEvidenceFact::Transformed { .. } => actual_transformed += 1,
            ProjectionEvidenceFact::Lost { class, .. } => actual_losses[class as usize] += 1,
        }
    }
    if (preserved, transformed, losses) != (actual_preserved, actual_transformed, actual_losses) {
        return Err(E::Malformed);
    }
    Ok(view)
}
fn items<'a, T>(
    section: Section<'a>,
    parse: fn(&mut Cursor<'a>) -> Result<T, E>,
) -> ProjectionEvidenceItems<'a, T> {
    ProjectionEvidenceItems {
        cursor: Cursor {
            bytes: section.bytes,
            position: 0,
        },
        remaining: section.count,
        parse,
    }
}
impl<'a> ProjectionEvidenceView<'a> {
    pub fn facts(self) -> ProjectionEvidenceItems<'a, ProjectionEvidenceFact<'a>> {
        items(self.facts, fact)
    }
    pub fn native(self) -> ProjectionEvidenceItems<'a, ProjectionEvidenceNative<'a>> {
        items(self.native, native)
    }
    pub fn scores(self) -> ProjectionEvidenceItems<'a, ProjectionEvidenceScore<'a>> {
        items(self.scores, score)
    }
    pub fn admitted(self) -> ProjectionEvidenceItems<'a, ProjectionEvidenceRealization<'a>> {
        items(self.admitted, realization)
    }
    pub fn attempts(self) -> ProjectionEvidenceItems<'a, ProjectionEvidenceAttempt<'a>> {
        items(self.attempts, attempt)
    }
    pub fn diagnostics(self) -> ProjectionEvidenceItems<'a, &'a str> {
        items(self.diagnostics, Cursor::text)
    }
}
fn fact<'a>(c: &mut Cursor<'a>) -> Result<ProjectionEvidenceFact<'a>, E> {
    let tag = c.byte()?;
    let obligation = c.text()?;
    Ok(match tag {
        0 => ProjectionEvidenceFact::Preserved { obligation },
        1 => ProjectionEvidenceFact::Transformed {
            obligation,
            law: c.text()?,
        },
        2 => {
            let class = match c.byte()? {
                0 => ProjectionLoss::Unrepresentable,
                1 => ProjectionLoss::Unrecognized,
                2 => ProjectionLoss::Precision,
                3 => ProjectionLoss::Range,
                4 => ProjectionLoss::Cardinality,
                5 => ProjectionLoss::Ordering,
                6 => ProjectionLoss::TemporalFinality,
                7 => ProjectionLoss::Identity,
                8 => ProjectionLoss::Provenance,
                9 => ProjectionLoss::UnsupportedVariant,
                10 => ProjectionLoss::Truncation,
                11 => ProjectionLoss::Approximation,
                _ => return Err(E::Malformed),
            };
            ProjectionEvidenceFact::Lost {
                obligation,
                class,
                detail: c.blob(MAX_PROJECTION_NATIVE_BYTES)?,
                native_fact: c.optional(Cursor::text)?,
            }
        }
        _ => return Err(E::Malformed),
    })
}
fn native<'a>(c: &mut Cursor<'a>) -> Result<ProjectionEvidenceNative<'a>, E> {
    let n = ProjectionEvidenceNative {
        identity: c.text()?,
        contract: c.text()?,
        provider: c.text()?,
        encoding: c.text()?,
        bytes: c.blob(MAX_PROJECTION_NATIVE_BYTES)?,
    };
    if n.bytes.is_empty() {
        return Err(E::Malformed);
    }
    Ok(n)
}
fn score_value<'a>(c: &mut Cursor<'a>) -> Result<ProjectionEvidenceScoreValue<'a>, E> {
    let s = ProjectionEvidenceScoreValue {
        scale: c.text()?,
        calibration: c.optional(Cursor::text)?,
        producer: c.text()?,
        minimum: i64::from_le_bytes(c.take(8)?.try_into().unwrap()),
        maximum: i64::from_le_bytes(c.take(8)?.try_into().unwrap()),
        value: i64::from_le_bytes(c.take(8)?.try_into().unwrap()),
    };
    if s.minimum > s.maximum || !(s.minimum..=s.maximum).contains(&s.value) {
        return Err(E::Malformed);
    }
    Ok(s)
}
fn score<'a>(c: &mut Cursor<'a>) -> Result<ProjectionEvidenceScore<'a>, E> {
    let tag = c.byte()?;
    let fact = c.text()?;
    match tag {
        0 => Ok(ProjectionEvidenceScore::Native {
            fact,
            score: score_value(c)?,
        }),
        1 => Ok(ProjectionEvidenceScore::Normalized {
            fact,
            original: score_value(c)?,
            projected: score_value(c)?,
            policy: c.text()?,
        }),
        _ => Err(E::Malformed),
    }
}
fn realization<'a>(c: &mut Cursor<'a>) -> Result<ProjectionEvidenceRealization<'a>, E> {
    Ok(ProjectionEvidenceRealization {
        plan: c.text()?,
        host: c.text()?,
        boot: c.text()?,
        back: c.text()?,
        artifact: c.text()?,
    })
}
fn mechanism(c: &mut Cursor<'_>) -> Result<ProjectionMechanism, E> {
    match c.byte()? {
        0 => Ok(ProjectionMechanism::Completed),
        1 => Ok(ProjectionMechanism::Refused(c.terminal()?)),
        2 => Ok(ProjectionMechanism::Failed(c.terminal()?)),
        3 => Ok(ProjectionMechanism::Cancelled(c.terminal()?)),
        _ => Err(E::Malformed),
    }
}
fn attempt<'a>(c: &mut Cursor<'a>) -> Result<ProjectionEvidenceAttempt<'a>, E> {
    Ok(ProjectionEvidenceAttempt {
        realization: realization(c)?,
        outcome: mechanism(c)?,
        fallback_reason: c.optional(Cursor::text)?,
    })
}
