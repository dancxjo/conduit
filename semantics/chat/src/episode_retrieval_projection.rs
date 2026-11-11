//! Bounded display of actual retained episode data, without selecting or
//! rewriting its evidence. This projection grants no instruction authority.
use alloc::vec::Vec;
use conduit_core::ValidatedCanonicalStructuredValue;
#[path = "episode_retrieval_projection/writer.rs"]
mod writer;
use writer::*;
pub const MAXIMUM_EPISODE_RETRIEVAL_PROJECTION_BYTES: usize = 4096;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpisodeRetrievalProjectionRefusal {
    Snapshot(crate::EpisodeRetrievalSnapshotRefusal),
    MalformedRecord,
    InvalidEvidenceJson,
    InvalidSelection,
    ByteBound,
}
use EpisodeRetrievalProjectionRefusal as R;
type View<'a> = ValidatedCanonicalStructuredValue<'a>;
pub struct PreparedEpisodeRetrievalProjection {
    output: Vec<u8>,
    parser: crate::PreparedEpisodeRetrievalEvidence,
}
impl PreparedEpisodeRetrievalProjection {
    pub fn prepare() -> Self {
        Self {
            output: Vec::with_capacity(MAXIMUM_EPISODE_RETRIEVAL_PROJECTION_BYTES),
            parser: crate::PreparedEpisodeRetrievalEvidence::new(),
        }
    }
    pub fn output(&self) -> &[u8] {
        &self.output
    }
    /// Publish only a complete projection; every refusal discards prior output.
    pub fn render(
        &mut self,
        snapshot: &crate::BorrowedEpisodeRetrievalSnapshot<'_>,
        episode: u64,
    ) -> Result<&[u8], R> {
        self.output.clear();
        if let Err(error) = self.render_inner(snapshot, episode, None) {
            self.output.clear();
            return Err(error);
        }
        Ok(&self.output)
    }
    /// Project a complete archive with explicit original evidence membership.
    /// Unselected archive items retain custody only, not temporal eligibility.
    pub fn render_selected(
        &mut self,
        snapshot: &crate::BorrowedEpisodeRetrievalSnapshot<'_>,
        episode: u64,
        items: &[u16],
    ) -> Result<&[u8], R> {
        self.output.clear();
        if let Err(error) = self.render_inner(snapshot, episode, Some(items)) {
            self.output.clear();
            return Err(error);
        }
        Ok(&self.output)
    }
    fn render_inner(
        &mut self,
        snapshot: &crate::BorrowedEpisodeRetrievalSnapshot<'_>,
        episode: u64,
        selection: Option<&[u16]>,
    ) -> Result<(), R> {
        let record = snapshot.original_record(episode).map_err(R::Snapshot)?;
        let out = &mut self.output;
        put(out, br#"{"origin":"remembered","episode":"#)?;
        integer(out, episode)?;
        put(
            out,
            br#", "evidence_scope":"historical_snapshot","archive_scope":"raw_episode_for_custody","temporal_selection":"#,
        )?;
        if let Some(items) = selection {
            let evidence =
                variant(field(record, "evidence")?, "some")?.ok_or(R::InvalidSelection)?;
            let parsed = self
                .parser
                .parse(leaf(evidence)?)
                .map_err(|_| R::InvalidEvidenceJson)?;
            if items.is_empty()
                || items.len() > parsed.items().len()
                || items.iter().enumerate().any(|(i, item)| {
                    *item as usize >= parsed.items().len() || items[i + 1..].contains(item)
                })
            {
                return Err(R::InvalidSelection);
            }
            put(out, br#"{"scope":"only_listed_original_items","representative_time_scope":"earliest_selected_observed_at_only","item_indices":["#)?;
            for (index, item) in items.iter().enumerate() {
                if index != 0 {
                    put(out, b",")?;
                }
                integer(out, *item as u64)?;
            }
            put(out, b"]}")?;
        } else {
            put(out, b"null")?;
        }
        put(out, br#", "original_observation":{"identity":"#)?;
        let observation = field(record, "observation")?;
        string(out, leaf(field(observation, "identity")?)?)?;
        put(out, br#", "content":"#)?;
        string(out, leaf(field(observation, "content")?)?)?;
        put(out, br#", "retain":"#)?;
        match leaf(field(observation, "retain")?)? {
            [0] => put(out, b"false")?,
            [1] => put(out, b"true")?,
            _ => return Err(R::MalformedRecord),
        }
        put(out, br#"},"actual_outcome":"#)?;
        let outcome = field(record, "outcome")?;
        if let Some(produced) = variant(outcome, "produced")? {
            let response = field(produced, "response")?;
            put(
                out,
                br#"{"disposition":"produced","provenance":"model_derived","content":"#,
            )?;
            string(out, leaf(field(response, "content")?)?)?;
            put(out, br#", "implementation":"#)?;
            string(out, leaf(field(response, "implementation")?)?)?;
            put(out, br#", "run":"#)?;
            string(out, leaf(field(response, "run")?)?)?;
            put(out, br#", "truncated":"#)?;
            match leaf(field(response, "truncated")?)? {
                [0] => put(out, b"false")?,
                [1] => put(out, b"true")?,
                _ => return Err(R::MalformedRecord),
            }
            put(out, b"}")?;
        } else if let Some(lost) = variant(outcome, "provider_lost")? {
            put(out, br#"{"disposition":"provider_lost","attempt":"#)?;
            string(out, leaf(field(lost, "attempt")?)?)?;
            put(out, br#", "implementation":"#)?;
            string(out, leaf(field(lost, "implementation")?)?)?;
            put(out, b"}")?;
        } else {
            return Err(R::MalformedRecord);
        }
        put(out, br#", "historical_evidence":"#)?;
        let evidence = field(record, "evidence")?;
        if let Some(value) = variant(evidence, "some")? {
            let raw = leaf(value)?;
            conduit_web::validate_json_text(raw).map_err(|_| R::InvalidEvidenceJson)?;
            put(out, raw)?;
        } else if variant(evidence, "none")?.is_some() {
            put(out, b"null")?
        } else {
            return Err(R::MalformedRecord);
        }
        put(out, b"}")
    }
}
impl Default for PreparedEpisodeRetrievalProjection {
    fn default() -> Self {
        Self::prepare()
    }
}
fn field<'a>(view: View<'a>, name: &str) -> Result<View<'a>, R> {
    view.record_field(name)
        .map_err(|_| R::MalformedRecord)?
        .ok_or(R::MalformedRecord)
}
fn variant<'a>(view: View<'a>, tag: &str) -> Result<Option<View<'a>>, R> {
    view.variant_payload(tag).map_err(|_| R::MalformedRecord)
}
fn leaf(view: View<'_>) -> Result<&[u8], R> {
    let node = view.value_node();
    if node.len() < 5 || node[0] != 0 {
        return Err(R::MalformedRecord);
    }
    let size = u32::from_le_bytes(node[1..5].try_into().map_err(|_| R::MalformedRecord)?) as usize;
    if size != node.len() - 5 {
        return Err(R::MalformedRecord);
    }
    Ok(&node[5..])
}
