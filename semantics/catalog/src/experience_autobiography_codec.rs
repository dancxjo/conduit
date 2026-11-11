//! A pure bounded retention transformation. Autobiography lives only on Cords.
//! Model envelopes remain exact model-derived bytes, never observed evidence.
use crate::{BoundedExperienceEvidenceProjection, EXPERIENCE_AUTOBIOGRAPHY_MAX_BYTES as MAX};
use alloc::vec::Vec;
use conduit_core::{validate_canonical_structured_value, ValidatedCanonicalStructuredValue};
use conduit_human::ExperienceEvidenceRefusal;
type View<'a> = ValidatedCanonicalStructuredValue<'a>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExperienceAutobiographyRefusal {
    Malformed,
    ByteBound,
    StaleRevision,
    FutureRevision,
    RevisionOverflow,
    InvalidHistory,
    InvalidResult,
    Evidence(ExperienceEvidenceRefusal),
}
use ExperienceAutobiographyRefusal as R;
pub struct BoundedExperienceAutobiographyRetainer {
    pending_type: Vec<u8>,
    state_type: Vec<u8>,
    request_type: Vec<u8>,
    validation: Vec<u8>,
    validator: BoundedExperienceEvidenceProjection,
    output: Vec<u8>,
}
impl BoundedExperienceAutobiographyRetainer {
    pub fn prepare() -> Self {
        Self {
            pending_type: crate::experience_autobiography_pending_type()
                .canonical_bytes()
                .unwrap(),
            state_type: crate::experience_autobiography_state_type()
                .canonical_bytes()
                .unwrap(),
            request_type: crate::experience_evidence_request_type()
                .canonical_bytes()
                .unwrap(),
            validation: Vec::with_capacity(MAX),
            validator: BoundedExperienceEvidenceProjection::prepare(),
            output: Vec::with_capacity(MAX),
        }
    }
    pub fn output(&self) -> &[u8] {
        &self.output
    }
    /// Both inputs are paired by ordinary Cords; this adapter retains no episode
    /// or autobiography between calls. Repeating inputs repeats the pure result.
    pub fn execute(&mut self, pending: &[u8], result: &[u8]) -> Result<&[u8], R> {
        self.output.clear();
        self.validation.clear();
        if let Err(error) = self.retain(pending, result) {
            self.output.clear();
            self.validation.clear();
            return Err(error);
        }
        Ok(&self.output)
    }
    fn validate_observation(&mut self, observation: View<'_>) -> Result<(), R> {
        self.validation.clear();
        put(&mut self.validation, &self.request_type)?;
        put(&mut self.validation, observation.value_node())?;
        self.validator
            .execute(&self.validation)
            .map_err(R::Evidence)?;
        Ok(())
    }
    fn retain(&mut self, pending: &[u8], result: &[u8]) -> Result<(), R> {
        if pending.len() > MAX {
            return Err(R::ByteBound);
        }
        validate_result(result)?;
        let pending = validate_canonical_structured_value(pending).map_err(|_| R::Malformed)?;
        if pending.type_bytes() != self.pending_type {
            return Err(R::Malformed);
        }
        let previous = field(pending, "previous")?;
        let completed = number(field(previous, "completed")?)?;
        let expected = number(field(pending, "expected_completed")?)?;
        if expected < completed {
            return Err(R::StaleRevision);
        }
        if expected > completed {
            return Err(R::FutureRevision);
        }
        let next = completed.checked_add(1).ok_or(R::RevisionOverflow)?;
        let retain = boolean(field(pending, "retain")?)?;
        let observation = field(pending, "observation")?;
        self.validate_observation(observation)?;
        let records = field(previous, "records")?;
        let mut kept = [None; 2];
        let mut count = 0;
        let mut prior = 0;
        for index in 0..2 {
            let Some(record) = records.collection_index(index).map_err(|_| R::Malformed)? else {
                break;
            };
            let episode = number(field(record, "episode")?)?;
            if episode == 0 || episode <= prior || episode > completed {
                return Err(R::InvalidHistory);
            }
            prior = episode;
            self.validate_observation(field(record, "observation")?)?;
            let response = field(record, "response")?
                .variant_payload("model_derived")
                .map_err(|_| R::Malformed)?
                .ok_or(R::InvalidHistory)?;
            validate_result(leaf(response)?)?;
            kept[count] = Some(record);
            count += 1;
        }
        let start = usize::from(retain && count == 2);
        let final_count = count - start + usize::from(retain);
        put(&mut self.output, &self.state_type)?;
        record(&mut self.output, 2)?;
        name(&mut self.output, "completed")?;
        scalar(&mut self.output, &next.to_le_bytes())?;
        name(&mut self.output, "records")?;
        put(&mut self.output, &[1])?;
        put(&mut self.output, &(final_count as u32).to_le_bytes())?;
        for value in kept[start..count].iter().flatten() {
            put(&mut self.output, value.value_node())?;
        }
        if retain {
            record(&mut self.output, 3)?;
            name(&mut self.output, "episode")?;
            scalar(&mut self.output, &next.to_le_bytes())?;
            name(&mut self.output, "observation")?;
            put(&mut self.output, observation.value_node())?;
            name(&mut self.output, "response")?;
            put(&mut self.output, &[3])?;
            name(&mut self.output, "model_derived")?;
            scalar(&mut self.output, result)?;
        }
        validate_canonical_structured_value(&self.output).map_err(|_| R::Malformed)?;
        Ok(())
    }
}
fn field<'a>(view: View<'a>, key: &str) -> Result<View<'a>, R> {
    view.record_field(key)
        .map_err(|_| R::Malformed)?
        .ok_or(R::Malformed)
}
fn leaf(view: View<'_>) -> Result<&[u8], R> {
    let node = view.value_node();
    if node.len() < 5 || node[0] != 0 {
        return Err(R::Malformed);
    }
    if u32::from_le_bytes(node[1..5].try_into().unwrap()) as usize != node.len() - 5 {
        return Err(R::Malformed);
    }
    Ok(&node[5..])
}
fn number(view: View<'_>) -> Result<u64, R> {
    Ok(u64::from_le_bytes(
        leaf(view)?.try_into().map_err(|_| R::Malformed)?,
    ))
}
fn boolean(view: View<'_>) -> Result<bool, R> {
    match leaf(view)? {
        [0] => Ok(false),
        [1] => Ok(true),
        _ => Err(R::Malformed),
    }
}
fn put(out: &mut Vec<u8>, value: &[u8]) -> Result<(), R> {
    if value.len() > MAX - out.len() {
        return Err(R::ByteBound);
    }
    out.extend_from_slice(value);
    Ok(())
}
fn name(out: &mut Vec<u8>, key: &str) -> Result<(), R> {
    put(out, &(key.len() as u32).to_le_bytes())?;
    put(out, key.as_bytes())
}
fn scalar(out: &mut Vec<u8>, value: &[u8]) -> Result<(), R> {
    put(out, &[0])?;
    put(out, &(value.len() as u32).to_le_bytes())?;
    put(out, value)
}
fn record(out: &mut Vec<u8>, fields: u32) -> Result<(), R> {
    put(out, &[2])?;
    put(out, &fields.to_le_bytes())
}

/// Exact serde ModelDerivedResult envelope for the admitted bounded text profile.
/// This is structural validation, not authentication of the declared provenance.
pub(crate) fn validate_result(bytes: &[u8]) -> Result<(), R> {
    if bytes.len() > 4096 {
        return Err(R::ByteBound);
    }
    conduit_web::validate_json_text(bytes).map_err(|_| R::InvalidResult)?;
    let mut json = ResultCursor(bytes);
    json.take(b"{\"provenance\":\"ModelDerived\",\"payload_kind\":\"llm/generated-result@1\",\"payload\":[")?;
    let mut payload = [0; 512];
    let mut length = 0;
    if !json.0.starts_with(b"]") {
        loop {
            let byte = json.integer()?;
            if byte > 255 || length == payload.len() {
                return Err(R::InvalidResult);
            }
            payload[length] = byte as u8;
            length += 1;
            if json.0.starts_with(b"]") {
                break;
            }
            json.take(b",")?;
        }
    }
    if length == 0 || core::str::from_utf8(&payload[..length]).is_err() {
        return Err(R::InvalidResult);
    }
    json.take(b"],\"implementation_identity\":")?;
    json.identity()?;
    json.take(b",\"request_identity\":")?;
    json.identity()?;
    json.take(b",\"run_identity\":")?;
    json.identity()?;
    json.take(b",\"confidence\":")?;
    if json.0.starts_with(b"null") {
        json.take(b"null")?;
    } else if json.integer()? > 1000 {
        return Err(R::InvalidResult);
    }
    json.take(b",\"disposition\":")?;
    if json.0.starts_with(b"\"Produced\"") {
        json.take(b"\"Produced\"")?;
    } else {
        json.take(b"\"Truncated\"")?;
    }
    json.take(b",\"determinism\":")?;
    let mut found = false;
    for profile in [
        b"\"DeterministicValidationFixture\"".as_slice(),
        b"\"SeededImplementationBestEffort\"",
        b"\"StochasticInference\"",
        b"\"ProviderNondeterministic\"",
    ] {
        if json.0.starts_with(profile) {
            json.take(profile)?;
            found = true;
            break;
        }
    }
    if !found {
        return Err(R::InvalidResult);
    }
    json.take(b",\"accounting\":{\"context_items\":")?;
    if json.integer()? > 1 {
        return Err(R::InvalidResult);
    }
    json.take(b",\"history_items\":0,\"input_bytes\":")?;
    if json.integer()? > 4096 {
        return Err(R::InvalidResult);
    }
    json.take(b",\"output_bytes\":")?;
    if json.integer()? != length as u64 {
        return Err(R::InvalidResult);
    }
    json.take(b",\"work_units\":")?;
    if json.integer()? > 4096 {
        return Err(R::InvalidResult);
    }
    json.take(b"}}")?;
    if !json.0.is_empty() {
        return Err(R::InvalidResult);
    }
    Ok(())
}
struct ResultCursor<'a>(&'a [u8]);
impl ResultCursor<'_> {
    fn take(&mut self, prefix: &[u8]) -> Result<(), R> {
        self.0 = self.0.strip_prefix(prefix).ok_or(R::InvalidResult)?;
        Ok(())
    }
    fn integer(&mut self) -> Result<u64, R> {
        let mut n = 0u64;
        let mut digits = 0;
        while let Some(byte @ b'0'..=b'9') = self.0.get(digits) {
            n = n
                .checked_mul(10)
                .and_then(|n| n.checked_add((byte - b'0') as u64))
                .ok_or(R::InvalidResult)?;
            digits += 1;
        }
        if digits == 0 || (digits > 1 && self.0[0] == b'0') {
            return Err(R::InvalidResult);
        }
        self.0 = &self.0[digits..];
        Ok(n)
    }
    fn identity(&mut self) -> Result<(), R> {
        self.take(b"\"")?;
        let mut index = 0;
        let mut decoded = 0;
        loop {
            let byte = *self.0.get(index).ok_or(R::InvalidResult)?;
            if byte == b'"' {
                break;
            }
            if byte == b'\\' {
                index += 1;
                match self.0.get(index) {
                    Some(b'"' | b'\\' | b'b' | b'f' | b'n' | b'r' | b't') => {}
                    Some(b'u') => {
                        let escape = self.0.get(index + 1..index + 5).ok_or(R::InvalidResult)?;
                        if escape[0..2] != *b"00"
                            || !escape[2..]
                                .iter()
                                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
                        {
                            return Err(R::InvalidResult);
                        }
                        let nibble = |b| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
                        let control = nibble(escape[2]) * 16 + nibble(escape[3]);
                        if control >= 32 || matches!(control, 8 | 9 | 10 | 12 | 13) {
                            return Err(R::InvalidResult);
                        }
                        index += 4;
                    }
                    _ => return Err(R::InvalidResult),
                }
            }
            index += 1;
            decoded += 1;
            if decoded > 256 {
                return Err(R::InvalidResult);
            }
        }
        if decoded == 0 {
            return Err(R::InvalidResult);
        }
        self.0 = &self.0[index + 1..];
        Ok(())
    }
}
