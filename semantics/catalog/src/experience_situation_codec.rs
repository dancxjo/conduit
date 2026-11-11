//! Prepared Situation selection; all clocks, policies and evidence are explicit.
use crate::{BoundedExperienceEvidenceProjection, EXPERIENCE_EVIDENCE_MAX_BYTES as MAX};
use alloc::vec::Vec;
use conduit_core::{
    validate_canonical_structured_value, BorrowedTemporalInstant, TemporalRelation,
    TemporalRelationError, ValidatedCanonicalStructuredValue,
};
use conduit_human::ExperienceEvidenceRefusal;
type View<'a> = ValidatedCanonicalStructuredValue<'a>;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExperienceSituationRefusal {
    Malformed,
    ByteBound,
    SourceBound,
    RememberedInput,
    InvalidAvailability,
    FutureObservation,
    Temporal(TemporalRelationError),
    Evidence(ExperienceEvidenceRefusal),
}
use ExperienceSituationRefusal as R;
pub struct BoundedExperienceSituation {
    frame_type: Vec<u8>,
    request_type: Vec<u8>,
    event_type: Vec<u8>,
    situation: Vec<u8>,
    event: Vec<u8>,
    validator: BoundedExperienceEvidenceProjection,
}
impl BoundedExperienceSituation {
    pub fn prepare() -> Self {
        Self {
            frame_type: crate::experience_situation_frame_type()
                .canonical_bytes()
                .unwrap(),
            request_type: crate::experience_evidence_request_type()
                .canonical_bytes()
                .unwrap(),
            event_type: crate::experience_autobiography_event_type()
                .canonical_bytes()
                .unwrap(),
            situation: Vec::with_capacity(MAX),
            event: Vec::with_capacity(MAX),
            validator: BoundedExperienceEvidenceProjection::prepare(),
        }
    }
    pub fn situation(&self) -> &[u8] {
        &self.situation
    }
    pub fn event(&self) -> &[u8] {
        &self.event
    }
    pub fn execute(&mut self, frame: &[u8]) -> Result<(), R> {
        self.situation.clear();
        self.event.clear();
        if let Err(error) = self.derive(frame) {
            self.situation.clear();
            self.event.clear();
            return Err(error);
        }
        Ok(())
    }
    fn derive(&mut self, frame: &[u8]) -> Result<(), R> {
        if frame.len() > MAX {
            return Err(R::ByteBound);
        }
        let frame = validate_canonical_structured_value(frame).map_err(|_| R::Malformed)?;
        if frame.type_bytes() != self.frame_type {
            return Err(R::Malformed);
        }
        let reference = field(frame, "reference")?;
        let reference_time =
            BorrowedTemporalInstant::from_canonical(reference).map_err(R::Temporal)?;
        reference_time.interval().map_err(R::Temporal)?;
        let policy = field(frame, "policy")?;
        let current_age = number(field(policy, "maximum_current_age_ticks")?)?;
        let recent_age = number(field(policy, "maximum_recent_age_ticks")?)?;
        if current_age >= recent_age {
            return Err(R::Malformed);
        }
        let observations = field(frame, "observations")?;
        let selection = field(frame, "selection")?;
        let mut count = 0;
        for index in 0..8 {
            if observations
                .collection_index(index)
                .map_err(|_| R::Malformed)?
                .is_some()
            {
                count += 1;
            } else {
                break;
            }
        }
        put(&mut self.situation, &self.request_type)?;
        record(&mut self.situation, 2)?;
        name(&mut self.situation, "evidence")?;
        sequence(&mut self.situation, count)?;
        for index in 0..count {
            let observation = observations
                .collection_index(index as u16)
                .map_err(|_| R::Malformed)?
                .ok_or(R::Malformed)?;
            let item = field(observation, "evidence")?;
            if case(field(item, "origin")?, "remembered")? {
                return Err(R::RememberedInput);
            }
            let sources = field(item, "sources")?;
            let mut source_count = 0;
            for source_index in 0..4 {
                let Some(source) = sources
                    .collection_index(source_index)
                    .map_err(|_| R::Malformed)?
                else {
                    break;
                };
                if case(source, "temporal_context")? {
                    return Err(R::SourceBound);
                }
                source_count += 1;
            }
            let time = field(observation, "time")?;
            let at = time
                .variant_payload("at")
                .map_err(|_| R::Malformed)?
                .map(|wrapper| field(wrapper, "instant"))
                .transpose()?;
            let present = case(field(item, "availability")?, "present")?;
            let (role, uncertain) = if present {
                let at = at.ok_or(R::InvalidAvailability)?;
                let instant = BorrowedTemporalInstant::from_canonical(at).map_err(R::Temporal)?;
                if source_count == 4 {
                    return Err(R::SourceBound);
                }
                (
                    classify(instant, reference_time, current_age, recent_age)?,
                    false,
                )
            } else {
                if at.is_some()
                    || !case(time, "absent")?
                    || !leaf(field(item, "encoded_content")?)?.is_empty()
                {
                    return Err(R::InvalidAvailability);
                }
                ("current", true)
            };
            record(&mut self.situation, 11)?;
            for key in [
                "availability",
                "certainty",
                "content_kind",
                "domain",
                "encoded_content",
                "identity",
                "origin",
                "provenance_revision",
                "revision",
                "sources",
                "temporal_role",
            ] {
                name(&mut self.situation, key)?;
                match key {
                    "certainty" if uncertain || role == "indeterminate" => {
                        unit_variant(&mut self.situation, "uncertain")?
                    }
                    "temporal_role" => unit_variant(&mut self.situation, role)?,
                    "sources" => {
                        sequence(&mut self.situation, source_count + usize::from(present))?;
                        for source_index in 0..source_count {
                            put(
                                &mut self.situation,
                                sources
                                    .collection_index(source_index as u16)
                                    .map_err(|_| R::Malformed)?
                                    .ok_or(R::Malformed)?
                                    .value_node(),
                            )?;
                        }
                        if let Some(at) = at {
                            put(&mut self.situation, &[3])?;
                            name(&mut self.situation, "temporal_context")?;
                            record(&mut self.situation, 3)?;
                            name(&mut self.situation, "observed_at")?;
                            put(&mut self.situation, at.value_node())?;
                            name(&mut self.situation, "policy")?;
                            put(&mut self.situation, policy.value_node())?;
                            name(&mut self.situation, "reference_at")?;
                            put(&mut self.situation, reference.value_node())?;
                        }
                    }
                    _ => put(&mut self.situation, field(item, key)?.value_node())?,
                }
            }
        }
        name(&mut self.situation, "selection")?;
        put(&mut self.situation, selection.value_node())?;
        self.validator
            .execute(&self.situation)
            .map_err(R::Evidence)?;
        let request =
            validate_canonical_structured_value(&self.situation).map_err(|_| R::Malformed)?;
        put(&mut self.event, &self.event_type)?;
        record(&mut self.event, 2)?;
        name(&mut self.event, "observation")?;
        put(&mut self.event, request.value_node())?;
        name(&mut self.event, "retain")?;
        put(&mut self.event, field(frame, "retain")?.value_node())?;
        validate_canonical_structured_value(&self.event).map_err(|_| R::Malformed)?;
        Ok(())
    }
}
fn classify(
    observed: BorrowedTemporalInstant<'_>,
    reference: BorrowedTemporalInstant<'_>,
    current: u64,
    recent: u64,
) -> Result<&'static str, R> {
    Ok(
        match observed.relation_to(&reference).map_err(R::Temporal)? {
            TemporalRelation::Present => "current",
            TemporalRelation::Past { maximum_ticks, .. } if maximum_ticks <= current => "current",
            TemporalRelation::Past {
                minimum_ticks,
                maximum_ticks,
            } if minimum_ticks > current && maximum_ticks <= recent => "recent",
            TemporalRelation::Past { minimum_ticks, .. } if minimum_ticks > recent => "stale",
            TemporalRelation::Indeterminate
                if reference
                    .interval()
                    .map_err(R::Temporal)?
                    .1
                    .saturating_sub(observed.interval().map_err(R::Temporal)?.0)
                    <= current =>
            {
                "current"
            }
            TemporalRelation::Indeterminate => "indeterminate",
            TemporalRelation::Past { .. } => "indeterminate",
            TemporalRelation::Future { .. } => return Err(R::FutureObservation),
        },
    )
}
fn field<'a>(v: View<'a>, key: &str) -> Result<View<'a>, R> {
    v.record_field(key)
        .map_err(|_| R::Malformed)?
        .ok_or(R::Malformed)
}
fn leaf(v: View<'_>) -> Result<&[u8], R> {
    let n = v.value_node();
    if n.len() < 5 || n[0] != 0 {
        return Err(R::Malformed);
    }
    if u32::from_le_bytes(n[1..5].try_into().unwrap()) as usize != n.len() - 5 {
        return Err(R::Malformed);
    }
    Ok(&n[5..])
}
fn number(v: View<'_>) -> Result<u64, R> {
    Ok(u64::from_le_bytes(
        leaf(v)?.try_into().map_err(|_| R::Malformed)?,
    ))
}
fn case(v: View<'_>, tag: &str) -> Result<bool, R> {
    Ok(v.variant_payload(tag).map_err(|_| R::Malformed)?.is_some())
}
fn put(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), R> {
    if bytes.len() > MAX - out.len() {
        return Err(R::ByteBound);
    }
    out.extend_from_slice(bytes);
    Ok(())
}
fn name(out: &mut Vec<u8>, key: &str) -> Result<(), R> {
    put(out, &(key.len() as u32).to_le_bytes())?;
    put(out, key.as_bytes())
}
fn scalar(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), R> {
    put(out, &[0])?;
    put(out, &(bytes.len() as u32).to_le_bytes())?;
    put(out, bytes)
}
fn record(out: &mut Vec<u8>, n: u32) -> Result<(), R> {
    put(out, &[2])?;
    put(out, &n.to_le_bytes())
}
fn sequence(out: &mut Vec<u8>, n: usize) -> Result<(), R> {
    put(out, &[1])?;
    put(out, &(n as u32).to_le_bytes())
}
fn unit_variant(out: &mut Vec<u8>, tag: &str) -> Result<(), R> {
    put(out, &[3])?;
    name(out, tag)?;
    scalar(out, &[])
}
