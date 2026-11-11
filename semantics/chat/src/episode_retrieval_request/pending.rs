//! Native pending packet paired with the exact transmitted retrieval context.
use super::EpisodeRetrievalRequestRefusal as R;
use alloc::vec::Vec;
use conduit_core::{validate_canonical_structured_value, ValidatedCanonicalStructuredValue};
const MAX: usize = crate::MAXIMUM_EPISODE_RECORD_BYTES;
type View<'a> = ValidatedCanonicalStructuredValue<'a>;
pub(super) struct PreparedEpisodeRetrievalPending {
    type_bytes: Vec<u8>,
    output: Vec<u8>,
}
impl PreparedEpisodeRetrievalPending {
    pub(super) fn prepare() -> Self {
        Self {
            type_bytes: crate::episode_pending_type()
                .canonical_bytes()
                .expect("native EpisodePending Type"),
            output: Vec::with_capacity(MAX),
        }
    }
    pub(super) fn output(&self) -> &[u8] {
        &self.output
    }
    pub(super) fn clear(&mut self) {
        self.output.clear();
    }
    pub(super) fn encode(
        &mut self,
        snapshot: &crate::BorrowedEpisodeRetrievalSnapshot<'_>,
        context: &crate::PreparedEpisodeRetrievalContext,
        current_json: &[u8],
    ) -> Result<&[u8], R> {
        self.clear();
        if let Err(error) = self.encode_inner(snapshot, context, current_json) {
            self.clear();
            return Err(error);
        }
        Ok(&self.output)
    }
    fn encode_inner(
        &mut self,
        snapshot: &crate::BorrowedEpisodeRetrievalSnapshot<'_>,
        context: &crate::PreparedEpisodeRetrievalContext,
        current_json: &[u8],
    ) -> Result<(), R> {
        if context.selected_snapshot_identity() != Some(snapshot.identity()) {
            return Err(R::SnapshotMismatch);
        }
        if current_json.len() > 4096 {
            return Err(R::PendingBound);
        }
        conduit_web::validate_json_text(current_json).map_err(|_| R::InvalidCurrentJson)?;
        let input = snapshot.input();
        let memory =
            validate_canonical_structured_value(input.memory).map_err(|_| R::MalformedPending)?;
        let completed = number(field(memory, "completed")?)?;
        let episode = completed.checked_add(1).ok_or(R::MalformedPending)?;
        let experience = validate_canonical_structured_value(input.experience)
            .map_err(|_| R::MalformedPending)?;
        let count = context.selected_episodes().count();
        if count > 4 || context.selected_fragments().count() != count {
            return Err(R::MalformedPending);
        }
        let out = &mut self.output;
        put(out, &self.type_bytes)?;
        record(out, 7)?;
        name(out, "body_id")?;
        scalar(out, input.expected.body_id.as_bytes())?;
        name(out, "episode")?;
        scalar(out, &episode.to_le_bytes())?;
        name(out, "evidence")?;
        some(out, current_json)?;
        name(out, "observation")?;
        put(out, field(experience, "observation")?.value_node())?;
        name(out, "recalled")?;
        put(out, &[1])?;
        put(out, &(count as u32).to_le_bytes())?;
        for (old_episode, fragment) in context
            .selected_episodes()
            .zip(context.selected_fragments())
        {
            if old_episode >= episode {
                return Err(R::MalformedPending);
            }
            if fragment.len() > 4096 {
                return Err(R::PendingBound);
            }
            conduit_web::validate_json_text(fragment).map_err(|_| R::InvalidFragmentJson)?;
            let original = snapshot
                .original_record(old_episode)
                .map_err(|_| R::MalformedPending)?;
            record(out, 4)?;
            name(out, "episode")?;
            put(out, field(original, "episode")?.value_node())?;
            name(out, "evidence")?;
            some(out, fragment)?;
            name(out, "observation")?;
            put(out, field(original, "observation")?.value_node())?;
            name(out, "outcome")?;
            put(out, field(original, "outcome")?.value_node())?;
        }
        name(out, "wake_id")?;
        scalar(out, input.expected.wake_id.as_bytes())?;
        name(out, "wake_sequence")?;
        scalar(out, &input.expected.wake_sequence.to_le_bytes())?;
        let checked = validate_canonical_structured_value(out).map_err(|_| R::MalformedPending)?;
        if checked.type_bytes() != self.type_bytes {
            return Err(R::MalformedPending);
        }
        Ok(())
    }
}
fn field<'a>(value: View<'a>, key: &str) -> Result<View<'a>, R> {
    value
        .record_field(key)
        .map_err(|_| R::MalformedPending)?
        .ok_or(R::MalformedPending)
}
fn number(value: View<'_>) -> Result<u64, R> {
    let node = value.value_node();
    if node.len() != 13 || node[0] != 0 || node[1..5] != 8u32.to_le_bytes() {
        return Err(R::MalformedPending);
    }
    Ok(u64::from_le_bytes(
        node[5..].try_into().map_err(|_| R::MalformedPending)?,
    ))
}
fn put(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), R> {
    if bytes.len() > MAX.saturating_sub(out.len()) {
        return Err(R::PendingBound);
    }
    out.extend_from_slice(bytes);
    Ok(())
}
fn name(out: &mut Vec<u8>, name: &str) -> Result<(), R> {
    put(out, &(name.len() as u32).to_le_bytes())?;
    put(out, name.as_bytes())
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
fn some(out: &mut Vec<u8>, value: &[u8]) -> Result<(), R> {
    put(out, &[3])?;
    name(out, "some")?;
    scalar(out, value)
}
