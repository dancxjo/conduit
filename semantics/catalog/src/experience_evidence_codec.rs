//! Allocation-free projection over explicit, exact canonical evidence inputs.
//! Revision checks are relative to the supplied selection, not an ambient clock
//! or current snapshot. Epistemic classifications are preserved, not certified.
use alloc::vec::Vec;
use conduit_core::{validate_canonical_structured_value, ValidatedCanonicalStructuredValue};
use conduit_human::ExperienceEvidenceRefusal as Refusal;

use crate::{
    EXPERIENCE_EVIDENCE_MAX_BYTES as MAX_BYTES, EXPERIENCE_EVIDENCE_MAX_CONTENT_BYTES,
    EXPERIENCE_EVIDENCE_MAX_IDENTITY_BYTES, EXPERIENCE_EVIDENCE_MAX_ITEMS as MAX_ITEMS,
    EXPERIENCE_EVIDENCE_MAX_SOURCES,
};

type View<'a> = ValidatedCanonicalStructuredValue<'a>;

pub struct BoundedExperienceEvidenceProjection {
    input_type: Vec<u8>,
    output_type: Vec<u8>,
    output: Vec<u8>,
}

impl BoundedExperienceEvidenceProjection {
    /// All allocation occurs before Play. Execute never resolves an identity.
    pub fn prepare() -> Self {
        Self {
            input_type: crate::experience_evidence_request_type()
                .canonical_bytes()
                .unwrap(),
            output_type: crate::experience_evidence_context_type()
                .canonical_bytes()
                .unwrap(),
            output: Vec::with_capacity(MAX_BYTES),
        }
    }

    pub fn output(&self) -> &[u8] {
        &self.output
    }

    pub fn execute(&mut self, input: &[u8]) -> Result<&[u8], Refusal> {
        self.output.clear();
        if input.len() > MAX_BYTES {
            return Err(Refusal::ByteBound);
        }
        let request = validate_canonical_structured_value(input).map_err(|_| Refusal::Malformed)?;
        if request.type_bytes() != self.input_type {
            return Err(Refusal::Malformed);
        }
        let selection = field(request, "selection")?;
        let selection_revision = revision(field(selection, "revision")?)?;
        let provenance = identity(field(selection, "provenance_revision")?)?;
        let selected = field(selection, "item_identities")?;
        let evidence = field(request, "evidence")?;
        let selected_count = count(selected, MAX_ITEMS, Refusal::ItemBound)?;
        let evidence_count = count(evidence, MAX_ITEMS, Refusal::ItemBound)?;
        let mut identities = [&[][..]; MAX_ITEMS];
        let mut items = [None; MAX_ITEMS];
        for index in 0..selected_count {
            let id = identity(element(selected, index)?)?;
            if identities[..index].contains(&id) {
                return Err(Refusal::DuplicateSelection);
            }
            identities[index] = id;
        }
        for index in 0..evidence_count {
            let item = element(evidence, index)?;
            let id = identity(field(item, "identity")?)?;
            let position = identities[..selected_count]
                .iter()
                .position(|selected| *selected == id)
                .ok_or(Refusal::UnselectedEvidence)?;
            if items[position].is_some() {
                return Err(Refusal::DuplicateEvidence);
            }
            match revision(field(item, "revision")?)?.cmp(&selection_revision) {
                core::cmp::Ordering::Less => return Err(Refusal::StaleRevision),
                core::cmp::Ordering::Greater => return Err(Refusal::FutureRevision),
                core::cmp::Ordering::Equal => {}
            }
            if identity(field(item, "provenance_revision")?)? != provenance {
                return Err(Refusal::ProvenanceMismatch);
            }
            validate_item(item)?;
            items[position] = Some(item.value_node());
        }
        if items[..selected_count].iter().any(Option::is_none) {
            return Err(Refusal::MissingEvidence);
        }
        // Check the whole output extent before writing to the preallocated buffer.
        let bytes = self.output_type.len()
            + 5
            + 4
            + "items".len()
            + 5
            + items[..selected_count]
                .iter()
                .flatten()
                .map(|item| item.len())
                .sum::<usize>()
            + 4
            + "selection".len()
            + selection.value_node().len();
        if bytes > MAX_BYTES {
            return Err(Refusal::ByteBound);
        }
        self.output.extend_from_slice(&self.output_type);
        self.output.push(2);
        self.output.extend_from_slice(&2_u32.to_le_bytes());
        text(&mut self.output, b"items");
        self.output.push(1);
        self.output
            .extend_from_slice(&(selected_count as u32).to_le_bytes());
        for item in items[..selected_count].iter().flatten() {
            self.output.extend_from_slice(item);
        }
        text(&mut self.output, b"selection");
        self.output.extend_from_slice(selection.value_node());
        debug_assert_eq!(self.output.len(), bytes);
        Ok(&self.output)
    }
}

fn validate_item(item: View<'_>) -> Result<(), Refusal> {
    identity(field(item, "content_kind")?)?;
    let content = leaf(field(item, "encoded_content")?)?;
    if content.len() > EXPERIENCE_EVIDENCE_MAX_CONTENT_BYTES {
        return Err(Refusal::ByteBound);
    }
    let origin = field(item, "origin")?;
    let role = field(item, "temporal_role")?;
    let availability = field(item, "availability")?;
    if (case(origin, "remembered")? && !case(role, "historical")?)
        || (!case(availability, "present")? && !content.is_empty())
    {
        return Err(Refusal::InvalidEpistemicCombination);
    }
    let sources = field(item, "sources")?;
    let length = count(
        sources,
        EXPERIENCE_EVIDENCE_MAX_SOURCES,
        Refusal::SourceBound,
    )?;
    for index in 0..length {
        let source = element(sources, index)?;
        // Compare with earlier canonical source nodes without retaining ownership.
        for prior in 0..index {
            if element(sources, prior)?.value_node() == source.value_node() {
                return Err(Refusal::SourceBound);
            }
        }
        if let Some(run) = source
            .variant_payload("implementation_run")
            .map_err(|_| Refusal::Malformed)?
        {
            for name in ["implementation", "provider_instance", "artifact", "run"] {
                identity(field(run, name)?)?;
            }
        } else if let Some(context) = source
            .variant_payload("temporal_context")
            .map_err(|_| Refusal::Malformed)?
        {
            for name in ["observed_at", "reference_at"] {
                conduit_core::BorrowedTemporalInstant::from_canonical(field(context, name)?)
                    .map_err(|_| Refusal::Malformed)?
                    .interval()
                    .map_err(|_| Refusal::Malformed)?;
            }
            let policy = field(context, "policy")?;
            if revision(field(policy, "maximum_current_age_ticks")?)?
                >= revision(field(policy, "maximum_recent_age_ticks")?)?
            {
                return Err(Refusal::Malformed);
            }
        } else if let Some(resource) = source
            .variant_payload("resource")
            .map_err(|_| Refusal::Malformed)?
        {
            // ResourceRef is inert provenance: validation grants no possession.
            let bytes = leaf(resource)?;
            conduit_core::BoundedResourceRef::validate_encoded(bytes)
                .map_err(|_| Refusal::Malformed)?;
        } else {
            let mut found = false;
            for tag in ["source", "sign", "human_statement", "memory_record"] {
                if let Some(payload) = source
                    .variant_payload(tag)
                    .map_err(|_| Refusal::Malformed)?
                {
                    identity(payload)?;
                    found = true;
                }
            }
            if !found {
                return Err(Refusal::Malformed);
            }
        }
    }
    Ok(())
}

fn field<'a>(value: View<'a>, name: &str) -> Result<View<'a>, Refusal> {
    value
        .record_field(name)
        .map_err(|_| Refusal::Malformed)?
        .ok_or(Refusal::Malformed)
}

fn element(value: View<'_>, index: usize) -> Result<View<'_>, Refusal> {
    value
        .collection_index(index as u16)
        .map_err(|_| Refusal::Malformed)?
        .ok_or(Refusal::Malformed)
}

fn case(value: View<'_>, tag: &str) -> Result<bool, Refusal> {
    Ok(value
        .variant_payload(tag)
        .map_err(|_| Refusal::Malformed)?
        .is_some())
}

fn count(value: View<'_>, maximum: usize, refusal: Refusal) -> Result<usize, Refusal> {
    let node = value.value_node();
    if node.first() != Some(&1) {
        return Err(Refusal::Malformed);
    }
    let length = u32::from_le_bytes(
        node.get(1..5)
            .ok_or(Refusal::Malformed)?
            .try_into()
            .unwrap(),
    ) as usize;
    if length > maximum {
        return Err(refusal);
    }
    Ok(length)
}

fn identity(value: View<'_>) -> Result<&[u8], Refusal> {
    let bytes = leaf(value)?;
    if bytes.is_empty() || bytes.len() > EXPERIENCE_EVIDENCE_MAX_IDENTITY_BYTES {
        return Err(Refusal::IdentityBound);
    }
    Ok(bytes)
}

fn revision(value: View<'_>) -> Result<u64, Refusal> {
    Ok(u64::from_le_bytes(
        leaf(value)?.try_into().map_err(|_| Refusal::Malformed)?,
    ))
}

fn leaf(value: View<'_>) -> Result<&[u8], Refusal> {
    let node = value.value_node();
    if node.first() != Some(&0) {
        return Err(Refusal::Malformed);
    }
    let length = u32::from_le_bytes(
        node.get(1..5)
            .ok_or(Refusal::Malformed)?
            .try_into()
            .unwrap(),
    ) as usize;
    let bytes = node.get(5..).ok_or(Refusal::Malformed)?;
    (bytes.len() == length)
        .then_some(bytes)
        .ok_or(Refusal::Malformed)
}

fn text(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u32).to_le_bytes());
    output.extend_from_slice(value);
}
