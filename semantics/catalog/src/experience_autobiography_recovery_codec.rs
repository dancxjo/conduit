//! Prepared, allocation-free provider-loss transformation over explicit pending state.
use crate::{
    BoundedExperienceAutobiographyRetainer, BoundedExperienceEvidenceProjection,
    ExperienceAutobiographyRefusal as R, EXPERIENCE_AUTOBIOGRAPHY_MAX_BYTES as MAX,
};
use alloc::vec::Vec;
use conduit_core::{
    validate_canonical_structured_value, TerminalCategory, TerminalInfo,
    ValidatedCanonicalStructuredValue,
};
type View<'a> = ValidatedCanonicalStructuredValue<'a>;
pub struct BoundedExperienceAutobiographyRecovery {
    success: BoundedExperienceAutobiographyRetainer,
    pending_type: Vec<u8>,
    state_type: Vec<u8>,
    request_type: Vec<u8>,
    validation: Vec<u8>,
    validator: BoundedExperienceEvidenceProjection,
    output: Vec<u8>,
    recovering: bool,
}
impl BoundedExperienceAutobiographyRecovery {
    pub fn prepare() -> Self {
        Self {
            success: BoundedExperienceAutobiographyRetainer::prepare(),
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
            recovering: false,
        }
    }
    pub fn output(&self) -> &[u8] {
        if self.recovering {
            &self.output
        } else {
            self.success.output()
        }
    }
    pub fn execute(&mut self, pending: &[u8], result: &[u8]) -> Result<&[u8], R> {
        self.recovering = false;
        self.success.execute(pending, result)
    }
    pub fn recover(&mut self, pending: &[u8], terminal: &[u8]) -> Result<&[u8], R> {
        self.recovering = true;
        self.output.clear();
        let result = self.stage_recovery(pending, terminal);
        if let Err(error) = result {
            self.output.clear();
            return Err(error);
        }
        Ok(&self.output)
    }
    fn observation(&mut self, view: View<'_>) -> Result<(), R> {
        self.validation.clear();
        put(&mut self.validation, &self.request_type)?;
        put(&mut self.validation, view.value_node())?;
        self.validator
            .execute(&self.validation)
            .map_err(R::Evidence)?;
        Ok(())
    }
    fn stage_recovery(&mut self, pending: &[u8], terminal: &[u8]) -> Result<(), R> {
        let terminal = TerminalInfo::decode(terminal).map_err(|_| R::InvalidResult)?;
        if terminal.category() != TerminalCategory::ProviderOrResourceLoss {
            return Err(R::InvalidResult);
        }
        if pending.len() > MAX {
            return Err(R::ByteBound);
        }
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
        self.observation(field(pending, "observation")?)?;
        let records = field(previous, "records")?;
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
            self.observation(field(record, "observation")?)?;
            let response = field(record, "response")?
                .variant_payload("model_derived")
                .map_err(|_| R::Malformed)?
                .ok_or(R::InvalidHistory)?;
            crate::experience_autobiography_codec::validate_result(leaf(response)?)?;
        }
        put(&mut self.output, &self.state_type)?;
        put(&mut self.output, &[2])?;
        put(&mut self.output, &2_u32.to_le_bytes())?;
        name(&mut self.output, "completed")?;
        put(&mut self.output, &[0])?;
        put(&mut self.output, &8_u32.to_le_bytes())?;
        put(&mut self.output, &next.to_le_bytes())?;
        name(&mut self.output, "records")?;
        put(&mut self.output, records.value_node())?;
        validate_canonical_structured_value(&self.output).map_err(|_| R::Malformed)?;
        Ok(())
    }
}
fn field<'a>(view: View<'a>, name: &str) -> Result<View<'a>, R> {
    view.record_field(name)
        .map_err(|_| R::Malformed)?
        .ok_or(R::Malformed)
}
fn leaf(view: View<'_>) -> Result<&[u8], R> {
    let bytes = view.value_node();
    if bytes.len() < 5
        || bytes[0] != 0
        || u32::from_le_bytes(bytes[1..5].try_into().unwrap()) as usize != bytes.len() - 5
    {
        return Err(R::Malformed);
    }
    Ok(&bytes[5..])
}
fn number(view: View<'_>) -> Result<u64, R> {
    Ok(u64::from_le_bytes(
        leaf(view)?.try_into().map_err(|_| R::Malformed)?,
    ))
}
fn put(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), R> {
    if bytes.len() > MAX - out.len() {
        return Err(R::ByteBound);
    }
    out.extend_from_slice(bytes);
    Ok(())
}
fn name(out: &mut Vec<u8>, name: &str) -> Result<(), R> {
    put(out, &(name.len() as u32).to_le_bytes())?;
    put(out, name.as_bytes())
}
