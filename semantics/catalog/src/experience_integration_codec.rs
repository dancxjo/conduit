//! Preallocated transactional integration of explicit canonical evidence.
//! Item revision/provenance_revision denote the emitted snapshot. Source nodes
//! remain byte-exact original evidence; integration does not certify them.
use crate::{
    BoundedExperienceEvidenceProjection, EXPERIENCE_EVIDENCE_MAX_BYTES as MAX_BYTES,
    EXPERIENCE_EVIDENCE_MAX_ITEMS as MAX_ITEMS,
};
use alloc::vec::Vec;
use conduit_core::{validate_canonical_structured_value, ValidatedCanonicalStructuredValue};
use conduit_human::ExperienceEvidenceRefusal;
type View<'a> = ValidatedCanonicalStructuredValue<'a>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExperienceIntegrationRefusal {
    Malformed,
    ByteBound,
    ItemBound,
    StaleRevision,
    FutureRevision,
    RevisionOverflow,
    UnknownIdentity,
    PendingUpdate,
    Evidence(ExperienceEvidenceRefusal),
}
use ExperienceIntegrationRefusal as R;

pub struct BoundedExperienceIntegration {
    update_type: Vec<u8>,
    request_type: Vec<u8>,
    current: Vec<u8>,
    pending: Vec<u8>,
    validator: BoundedExperienceEvidenceProjection,
    revision: u64,
    staged: bool,
}
impl BoundedExperienceIntegration {
    pub fn prepare() -> Self {
        Self {
            update_type: crate::experience_integration_update_type()
                .canonical_bytes()
                .unwrap(),
            request_type: crate::experience_evidence_request_type()
                .canonical_bytes()
                .unwrap(),
            current: Vec::with_capacity(MAX_BYTES),
            pending: Vec::with_capacity(MAX_BYTES),
            validator: BoundedExperienceEvidenceProjection::prepare(),
            revision: 0,
            staged: false,
        }
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn output(&self) -> &[u8] {
        &self.pending
    }
    /// Stage one update. The committed state is untouched until `commit`.
    pub fn execute(&mut self, encoded: &[u8]) -> Result<&[u8], R> {
        if self.staged {
            return Err(R::PendingUpdate);
        }
        self.pending.clear();
        let result = self.stage(encoded);
        if let Err(error) = result {
            self.pending.clear();
            return Err(error);
        }
        self.staged = true;
        Ok(&self.pending)
    }
    fn stage(&mut self, encoded: &[u8]) -> Result<(), R> {
        if encoded.len() > MAX_BYTES {
            return Err(R::ByteBound);
        }
        let update = validate_canonical_structured_value(encoded).map_err(|_| R::Malformed)?;
        if update.type_bytes() != self.update_type {
            return Err(R::Malformed);
        }
        let expected = number(field(update, "expected_revision")?)?;
        if expected < self.revision {
            return Err(R::StaleRevision);
        }
        if expected > self.revision {
            return Err(R::FutureRevision);
        }
        let next = self.revision.checked_add(1).ok_or(R::RevisionOverflow)?;
        let provenance = field(update, "provenance_revision")?;
        let operation = field(update, "operation")?;
        let upsert = operation
            .variant_payload("upsert")
            .map_err(|_| R::Malformed)?;
        let remove = operation
            .variant_payload("remove")
            .map_err(|_| R::Malformed)?;
        let id = match (upsert, remove) {
            (Some(item), None) => field(item, "identity")?,
            (None, Some(id)) => id,
            _ => return Err(R::Malformed),
        };
        let mut items = [None; MAX_ITEMS];
        let mut count = 0;
        let mut matched = false;
        if !self.current.is_empty() {
            let current =
                validate_canonical_structured_value(&self.current).map_err(|_| R::Malformed)?;
            let evidence = field(current, "evidence")?;
            for index in 0..MAX_ITEMS {
                let Some(item) = evidence
                    .collection_index(index as u16)
                    .map_err(|_| R::Malformed)?
                else {
                    break;
                };
                if field(item, "identity")?.value_node() == id.value_node() {
                    matched = true;
                    if let Some(replacement) = upsert {
                        items[count] = Some(replacement);
                        count += 1;
                    }
                } else {
                    items[count] = Some(item);
                    count += 1;
                }
            }
        }
        if !matched {
            let Some(item) = upsert else {
                return Err(R::UnknownIdentity);
            };
            if count == MAX_ITEMS {
                return Err(R::ItemBound);
            }
            items[count] = Some(item);
            count += 1;
        }
        // All writes are checked against prepared capacity; no growth in Play.
        put(&mut self.pending, &self.request_type)?;
        record(&mut self.pending, 2)?;
        name(&mut self.pending, "evidence")?;
        sequence(&mut self.pending, count)?;
        for item in items[..count].iter().flatten() {
            record(&mut self.pending, 11)?;
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
                name(&mut self.pending, key)?;
                match key {
                    "provenance_revision" => put(&mut self.pending, provenance.value_node())?,
                    "revision" => scalar(&mut self.pending, &next.to_le_bytes())?,
                    _ => put(&mut self.pending, field(*item, key)?.value_node())?,
                }
            }
        }
        name(&mut self.pending, "selection")?;
        record(&mut self.pending, 3)?;
        name(&mut self.pending, "item_identities")?;
        sequence(&mut self.pending, count)?;
        for item in items[..count].iter().flatten() {
            put(&mut self.pending, field(*item, "identity")?.value_node())?;
        }
        name(&mut self.pending, "provenance_revision")?;
        put(&mut self.pending, provenance.value_node())?;
        name(&mut self.pending, "revision")?;
        scalar(&mut self.pending, &next.to_le_bytes())?;
        self.validator.execute(&self.pending).map_err(R::Evidence)?;
        Ok(())
    }
    pub fn commit(&mut self) {
        if self.staged {
            core::mem::swap(&mut self.current, &mut self.pending);
            self.pending.clear();
            self.revision += 1;
            self.staged = false;
        }
    }
    pub fn cancel(&mut self) {
        self.pending.clear();
        self.staged = false;
    }
}
fn field<'a>(view: View<'a>, key: &str) -> Result<View<'a>, R> {
    view.record_field(key)
        .map_err(|_| R::Malformed)?
        .ok_or(R::Malformed)
}
fn number(view: View<'_>) -> Result<u64, R> {
    let node = view.value_node();
    if node.len() != 13 || node[..5] != [0, 8, 0, 0, 0] {
        return Err(R::Malformed);
    }
    Ok(u64::from_le_bytes(node[5..].try_into().unwrap()))
}
fn put(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), R> {
    if bytes.len() > MAX_BYTES - out.len() {
        return Err(R::ByteBound);
    }
    out.extend_from_slice(bytes);
    Ok(())
}
fn name(out: &mut Vec<u8>, key: &str) -> Result<(), R> {
    put(out, &(key.len() as u32).to_le_bytes())?;
    put(out, key.as_bytes())
}
fn record(out: &mut Vec<u8>, count: usize) -> Result<(), R> {
    put(out, &[2])?;
    put(out, &(count as u32).to_le_bytes())
}
fn sequence(out: &mut Vec<u8>, count: usize) -> Result<(), R> {
    put(out, &[1])?;
    put(out, &(count as u32).to_le_bytes())
}
fn scalar(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), R> {
    put(out, &[0])?;
    put(out, &(bytes.len() as u32).to_le_bytes())?;
    put(out, bytes)
}
