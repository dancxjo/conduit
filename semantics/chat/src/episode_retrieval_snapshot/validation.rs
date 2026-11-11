use super::*;
pub(super) fn field<'a>(view: View<'a>, name: &str) -> Result<View<'a>, R> {
    view.record_field(name)
        .map_err(|_| R::Malformed)?
        .ok_or(R::Malformed)
}
pub(super) fn number(view: View<'_>) -> Result<u64, R> {
    let bytes = leaf(view)?;
    Ok(u64::from_le_bytes(
        bytes.try_into().map_err(|_| R::Malformed)?,
    ))
}
fn text(view: View<'_>) -> Result<&str, R> {
    core::str::from_utf8(leaf(view)?).map_err(|_| R::Malformed)
}
pub(super) fn validate_owner(owner: EpisodeRetrievalOwner<'_>) -> Result<(), R> {
    if [owner.body_id, owner.wake_id]
        .iter()
        .any(|s| s.is_empty() || s.len() > 256)
    {
        return Err(R::OwnerIdentity);
    }
    Ok(())
}
pub(super) fn validate_memory<'a>(
    memory: View<'a>,
    expected: EpisodeRetrievalOwner<'_>,
) -> Result<(SnapshotRecords<'a>, usize), R> {
    let completed = number(field(memory, "completed")?)?;
    let owner = field(memory, "owner")?;
    let bound = owner.variant_payload("bound").map_err(|_| R::Malformed)?;
    match (completed, bound) {
        (0, None) => {
            if owner
                .variant_payload("unbound")
                .map_err(|_| R::Malformed)?
                .is_none()
            {
                return Err(R::OwnerMismatch);
            }
        }
        (0, Some(_)) | (_, None) => return Err(R::OwnerMismatch),
        (_, Some(owner)) => {
            if text(field(owner, "body_id")?)? != expected.body_id
                || text(field(owner, "wake_id")?)? != expected.wake_id
                || number(field(owner, "wake_sequence")?)? != expected.wake_sequence
            {
                return Err(R::OwnerMismatch);
            }
        }
    }
    let memories = field(memory, "memories")?;
    let mut records = [None; 4];
    let mut count = 0;
    let mut previous = 0;
    for (index, slot) in records.iter_mut().enumerate() {
        let Some(record) = memories
            .collection_index(index as u16)
            .map_err(|_| R::Malformed)?
        else {
            break;
        };
        let episode = number(field(record, "episode")?)?;
        if episode <= previous || episode > completed {
            return Err(R::MemoryOrder);
        }
        previous = episode;
        *slot = Some((episode, record));
        count += 1;
    }
    if memories
        .collection_index(4)
        .map_err(|_| R::Malformed)?
        .is_some()
    {
        return Err(R::MemoryOrder);
    }
    Ok((records, count))
}

fn leaf(view: View<'_>) -> Result<&[u8], R> {
    let node = view.value_node();
    if node.len() < 5 || node[0] != 0 {
        return Err(R::Malformed);
    }
    let length = u32::from_le_bytes(node[1..5].try_into().map_err(|_| R::Malformed)?) as usize;
    if length != node.len() - 5 {
        return Err(R::Malformed);
    }
    Ok(&node[5..])
}
