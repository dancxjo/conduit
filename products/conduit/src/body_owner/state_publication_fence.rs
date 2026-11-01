//! Prevent stale live writers from rolling back recovered committed worksets.
//! This is not a proof of arbitrary higher-sequence archive ancestry.
use super::*;
pub(super) fn require_current(
    root: &Path,
    incoming: &BodyBiographyEvidence,
    archives: &[BodyBiographyArchiveSegment],
) -> Result<(), String> {
    let Some(current) = load_current(root)? else {
        return Ok(());
    };
    if incoming.body_id != current.body_id
        || incoming.last_sequence() < current.last_sequence()
        || incoming.body.workload_revision < current.body.workload_revision
    {
        return Err(
            "stale owner publication would roll back committed biography or workload".into(),
        );
    }
    if incoming.body.workload_revision == current.body.workload_revision
        && incoming.body.workset != current.body.workset
    {
        return Err("same workload revision cannot name a different checked workset".into());
    }
    if incoming.last_sequence() == current.last_sequence() {
        // Archive compaction can change historical representation at this same
        // sequence. It cannot change current authority or canonical event content.
        let current_body = &current.body;
        let next_body = &incoming.body;
        if incoming.schema != current.schema
            || incoming.friendly_name != current.friendly_name
            || incoming.emergency != current.emergency
            || incoming.graduation != current.graduation
            || next_body.identity_derivation != current_body.identity_derivation
            || next_body.workset != current_body.workset
            || next_body.workload_revision != current_body.workload_revision
            || next_body.birth_sequence != current_body.birth_sequence
            || next_body.state != current_body.state
            || incoming.membership.body_id != current.membership.body_id
            || incoming.membership.revision != current.membership.revision
            || incoming.membership.parts != current.membership.parts
            || incoming.membership.fulfilled_sign_id != current.membership.fulfilled_sign_id
            || archive::record_at(root, &current, &[], current.last_sequence())?
                != archive::record_at(root, incoming, archives, incoming.last_sequence())?
        {
            return Err(
                "same biography sequence cannot replace committed authority or event content"
                    .into(),
            );
        }
    }
    Ok(())
}
