//! Pure recall transformation. No state, identity resolution or hidden memory.
use crate::{BoundedExperienceEvidenceProjection, EXPERIENCE_AUTOBIOGRAPHY_MAX_BYTES as MAX};
use alloc::vec::Vec;
use conduit_core::{validate_canonical_structured_value, ValidatedCanonicalStructuredValue};
use conduit_human::ExperienceEvidenceRefusal;
mod encoding;
use encoding::*;
type View<'a> = ValidatedCanonicalStructuredValue<'a>;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExperienceAutobiographyRecallRefusal {
    Malformed,
    ByteBound,
    ItemBound,
    SourceBound,
    IdentityBound,
    InvalidHistory,
    RevisionOverflow,
    Evidence(ExperienceEvidenceRefusal),
}
use ExperienceAutobiographyRecallRefusal as R;
pub struct BoundedExperienceAutobiographyRecall {
    event_type: Vec<u8>,
    state_type: Vec<u8>,
    update_type: Vec<u8>,
    pending_type: Vec<u8>,
    request_type: Vec<u8>,
    validator: BoundedExperienceEvidenceProjection,
    validation: Vec<u8>,
    update: Vec<u8>,
    pending: Vec<u8>,
    wrapper: Vec<u8>,
    identity: Vec<u8>,
    memory_identity: Vec<u8>,
}
impl BoundedExperienceAutobiographyRecall {
    pub fn prepare() -> Self {
        Self {
            event_type: crate::experience_autobiography_event_type()
                .canonical_bytes()
                .unwrap(),
            state_type: crate::experience_autobiography_state_type()
                .canonical_bytes()
                .unwrap(),
            update_type: crate::experience_integration_update_type()
                .canonical_bytes()
                .unwrap(),
            pending_type: crate::experience_autobiography_pending_type()
                .canonical_bytes()
                .unwrap(),
            request_type: crate::experience_evidence_request_type()
                .canonical_bytes()
                .unwrap(),
            validator: BoundedExperienceEvidenceProjection::prepare(),
            validation: Vec::with_capacity(MAX),
            update: Vec::with_capacity(MAX),
            pending: Vec::with_capacity(MAX),
            wrapper: Vec::with_capacity(MAX),
            identity: Vec::with_capacity(MAX),
            memory_identity: Vec::with_capacity(MAX),
        }
    }
    pub fn update(&self) -> &[u8] {
        &self.update
    }
    pub fn pending(&self) -> &[u8] {
        &self.pending
    }
    pub fn execute(&mut self, event: &[u8], memory: &[u8]) -> Result<(), R> {
        self.execute_with_selection(event, memory, None)
    }
    /// Resolve an explicit selection against this exact supplied state. No
    /// implicit latest-item policy is applied by this entry point.
    pub fn execute_selected(
        &mut self,
        event: &[u8],
        memory: &[u8],
        selection: Option<(u64, u16)>,
    ) -> Result<(), R> {
        self.execute_with_selection(event, memory, Some(selection))
    }
    fn execute_with_selection(
        &mut self,
        event: &[u8],
        memory: &[u8],
        explicit: Option<Option<(u64, u16)>>,
    ) -> Result<(), R> {
        self.update.clear();
        self.pending.clear();
        self.validation.clear();
        self.wrapper.clear();
        self.identity.clear();
        self.memory_identity.clear();
        if let Err(error) = self.recall(event, memory, explicit) {
            self.update.clear();
            self.pending.clear();
            return Err(error);
        }
        Ok(())
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
    fn recall(
        &mut self,
        event: &[u8],
        memory: &[u8],
        explicit: Option<Option<(u64, u16)>>,
    ) -> Result<(), R> {
        if event.len() > MAX || memory.len() > MAX {
            return Err(R::ByteBound);
        }
        let event = validate_canonical_structured_value(event).map_err(|_| R::Malformed)?;
        let state = validate_canonical_structured_value(memory).map_err(|_| R::Malformed)?;
        if event.type_bytes() != self.event_type || state.type_bytes() != self.state_type {
            return Err(R::Malformed);
        }
        let completed = number(field(state, "completed")?)?;
        completed.checked_add(1).ok_or(R::RevisionOverflow)?;
        let observation = field(event, "observation")?;
        self.validate_observation(observation)?;
        let current = field(observation, "evidence")?;
        let mut current_count = 0;
        for index in 0..8 {
            if element(current, index)?.is_some() {
                current_count += 1;
            } else {
                break;
            }
        }
        let records = field(state, "records")?;
        let mut previous = 0;
        let mut selected = None;
        for index in 0..2 {
            let Some(record) = element(records, index)? else {
                break;
            };
            let episode = number(field(record, "episode")?)?;
            if episode == 0 || episode <= previous || episode > completed {
                return Err(R::InvalidHistory);
            }
            previous = episode;
            let original = field(record, "observation")?;
            self.validate_observation(original)?;
            let response = field(record, "response")?
                .variant_payload("model_derived")
                .map_err(|_| R::Malformed)?
                .ok_or(R::InvalidHistory)?;
            crate::experience_autobiography_codec::validate_result(leaf(response)?)
                .map_err(|_| R::InvalidHistory)?;
            if let Some(choice) = explicit {
                if let Some((wanted_episode, wanted_index)) = choice {
                    if wanted_episode == episode {
                        let item = element(field(original, "evidence")?, wanted_index)?
                            .ok_or(R::InvalidHistory)?;
                        if !explicitly_selected(original, item)?
                            || tag(
                                field(item, "availability")?,
                                &["present", "not_observed", "source_unavailable"],
                            )? != "present"
                        {
                            return Err(R::InvalidHistory);
                        }
                        selected = Some((episode, item));
                    }
                }
                continue;
            }
            // Ascending records: a later matching episode replaces the earlier.
            // Within an episode the first matching original item wins.
            let history = field(original, "evidence")?;
            'candidate: for item_index in 0..8 {
                let Some(item) = element(history, item_index)? else {
                    break;
                };
                if !explicitly_selected(original, item)? {
                    continue;
                }
                for current_index in 0..current_count {
                    let candidate = element(current, current_index)?.ok_or(R::Malformed)?;
                    if !explicitly_selected(observation, candidate)? {
                        continue;
                    }
                    if field(item, "content_kind")?.value_node()
                        == field(candidate, "content_kind")?.value_node()
                        && field(item, "domain")?.value_node()
                            == field(candidate, "domain")?.value_node()
                    {
                        selected = Some((episode, item));
                        break 'candidate;
                    }
                }
            }
        }
        if explicit.is_some_and(|choice| choice.is_some()) && selected.is_none() {
            return Err(R::InvalidHistory);
        }
        if selected.is_some() && current_count == 8 {
            return Err(R::ItemBound);
        }
        if let Some((episode, item)) = selected {
            self.wrap(episode, item)?;
        }
        if selected.is_some() {
            for index in 0..current_count {
                if leaf(field(
                    element(current, index)?.ok_or(R::Malformed)?,
                    "identity",
                )?)? == self.identity
                {
                    return Err(R::Evidence(ExperienceEvidenceRefusal::DuplicateEvidence));
                }
            }
        }
        let selection = field(observation, "selection")?;
        put(&mut self.update, &self.update_type)?;
        record(&mut self.update, 3)?;
        name(&mut self.update, "expected_revision")?;
        scalar(&mut self.update, &completed.to_le_bytes())?;
        name(&mut self.update, "operation")?;
        put(&mut self.update, &[3])?;
        name(&mut self.update, "replace_snapshot")?;
        sequence(
            &mut self.update,
            usize::from(current_count) + usize::from(selected.is_some()),
        )?;
        for index in 0..current_count {
            put(
                &mut self.update,
                element(current, index)?.ok_or(R::Malformed)?.value_node(),
            )?;
        }
        if let Some((_, item)) = selected {
            self.recalled_item(item, selection)?;
        }
        name(&mut self.update, "provenance_revision")?;
        put(
            &mut self.update,
            field(selection, "provenance_revision")?.value_node(),
        )?;
        put(&mut self.pending, &self.pending_type)?;
        record(&mut self.pending, 4)?;
        name(&mut self.pending, "expected_completed")?;
        scalar(&mut self.pending, &completed.to_le_bytes())?;
        name(&mut self.pending, "observation")?;
        put(&mut self.pending, observation.value_node())?;
        name(&mut self.pending, "previous")?;
        put(&mut self.pending, state.value_node())?;
        name(&mut self.pending, "retain")?;
        put(&mut self.pending, field(event, "retain")?.value_node())?;
        validate_canonical_structured_value(&self.update).map_err(|_| R::Malformed)?;
        validate_canonical_structured_value(&self.pending).map_err(|_| R::Malformed)?;
        Ok(())
    }
    fn wrap(&mut self, episode: u64, item: View<'_>) -> Result<(), R> {
        put(&mut self.memory_identity, b"autobiography/episode/")?;
        decimal(&mut self.memory_identity, episode)?;
        put(&mut self.identity, b"recall/")?;
        decimal(&mut self.identity, episode)?;
        put(&mut self.identity, b"/")?;
        put(&mut self.identity, leaf(field(item, "identity")?)?)?;
        if self.identity.len() > 128 {
            return Err(R::IdentityBound);
        }
        put(&mut self.wrapper, b"{\"original_identity\":")?;
        quoted(&mut self.wrapper, leaf(field(item, "identity")?)?)?;
        put(&mut self.wrapper, b",\"original_content_kind\":")?;
        quoted(&mut self.wrapper, leaf(field(item, "content_kind")?)?)?;
        put(&mut self.wrapper, b",\"original_origin\":")?;
        quoted(
            &mut self.wrapper,
            tag(
                field(item, "origin")?,
                &[
                    "observation",
                    "human_reported",
                    "remembered",
                    "model_derived",
                    "imagined",
                ],
            )?
            .as_bytes(),
        )?;
        for (key, tags) in [
            (
                "availability",
                &["present", "not_observed", "source_unavailable"][..],
            ),
            (
                "temporal_role",
                &["current", "recent", "stale", "historical", "indeterminate"][..],
            ),
        ] {
            put(&mut self.wrapper, b",\"original_")?;
            put(&mut self.wrapper, key.as_bytes())?;
            put(&mut self.wrapper, b"\":")?;
            quoted(&mut self.wrapper, tag(field(item, key)?, tags)?.as_bytes())?;
        }
        put(&mut self.wrapper, b",\"original_domain\":")?;
        quoted(
            &mut self.wrapper,
            tag(
                field(item, "domain")?,
                &[
                    "visual",
                    "auditory",
                    "location",
                    "body_state",
                    "human_utterance",
                    "recollection",
                ],
            )?
            .as_bytes(),
        )?;
        let content = leaf(field(item, "encoded_content")?)?;
        if core::str::from_utf8(content).is_ok() {
            put(
                &mut self.wrapper,
                b",\"content_encoding\":\"utf8\",\"content\":",
            )?;
            quoted(&mut self.wrapper, content)?;
        } else {
            put(
                &mut self.wrapper,
                b",\"content_encoding\":\"hex\",\"content\":\"",
            )?;
            hex(&mut self.wrapper, content)?;
            put(&mut self.wrapper, b"\"")?;
        }
        put(&mut self.wrapper, b"}")?;
        if self.wrapper.len() > 2048 {
            return Err(R::ByteBound);
        }
        Ok(())
    }
    fn recalled_item(&mut self, item: View<'_>, selection: View<'_>) -> Result<(), R> {
        let sources = field(item, "sources")?;
        let mut count = 0;
        for index in 0..4 {
            if element(sources, index)?.is_some() {
                count += 1;
            } else {
                break;
            }
        }
        if count == 4 {
            return Err(R::SourceBound);
        }
        for index in 0..count {
            let source = element(sources, index)?.ok_or(R::Malformed)?;
            if let Some(id) = source
                .variant_payload("memory_record")
                .map_err(|_| R::Malformed)?
            {
                if leaf(id)? == self.memory_identity {
                    return Err(R::SourceBound);
                }
            }
        }
        let out = &mut self.update;
        record(out, 11)?;
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
            name(out, key)?;
            match key {
                "availability" => unit_variant(out, "present")?,
                "content_kind" => scalar(out, b"experience/remembered-evidence@1")?,
                "domain" => unit_variant(out, "recollection")?,
                "encoded_content" => scalar(out, &self.wrapper)?,
                "identity" => scalar(out, &self.identity)?,
                "origin" => unit_variant(out, "remembered")?,
                "provenance_revision" | "revision" => {
                    put(out, field(selection, key)?.value_node())?
                }
                "sources" => {
                    sequence(out, usize::from(count) + 1)?;
                    put(out, &[3])?;
                    name(out, "memory_record")?;
                    scalar(out, &self.memory_identity)?;
                    for index in 0..count {
                        put(
                            out,
                            element(sources, index)?.ok_or(R::Malformed)?.value_node(),
                        )?;
                    }
                }
                "temporal_role" => unit_variant(out, "historical")?,
                _ => put(out, field(item, key)?.value_node())?,
            }
        }
        Ok(())
    }
}
