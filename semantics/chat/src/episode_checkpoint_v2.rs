//! Upgrade successful-only memory to actual-outcome memory with exact provenance.
use super::*;
use conduit_core::StructuredInfoType;

#[allow(dead_code)]
mod legacy {
    include!(concat!(env!("OUT_DIR"), "/episode_checkpoint_v2.rs"));
}

pub fn episode_checkpoint_v2_type() -> StructuredInfoType {
    legacy::EpisodeMemoryState::semantic_type().expect("previous evidence checkpoint Type")
}

pub fn migrate_episode_checkpoint_v2(bytes: &[u8]) -> Result<Vec<u8>, EpisodeRefusal> {
    if bytes.len() > MAXIMUM_EPISODE_CHECKPOINT_BYTES {
        return Err(EpisodeRefusal::MemoryMalformed);
    }
    let malformed = |_| EpisodeRefusal::MemoryMalformed;
    let old = legacy::EpisodeMemoryState::decode(bytes).map_err(malformed)?;
    let owner = match old.owner() {
        legacy::EpisodeMemoryOwner::Unbound => EpisodeMemoryOwner::Unbound,
        legacy::EpisodeMemoryOwner::Bound(owner) => EpisodeMemoryOwner::bound(
            owner.body_id().clone(),
            owner.wake_id().clone(),
            *owner.wake_sequence(),
        )
        .map_err(malformed)?,
    };
    let mut memories = Vec::with_capacity(MAXIMUM_EPISODE_RECALL);
    let mut previous = 0;
    for item in old.memories().get() {
        if *item.episode() <= previous || item.episode() > old.completed() {
            return Err(EpisodeRefusal::MemoryMalformed);
        }
        previous = *item.episode();
        let observation = EpisodeObservation::from_structured(
            item.observation()
                .clone()
                .into_structured()
                .map_err(malformed)?,
        )
        .map_err(malformed)?;
        let response = EpisodeModelResponse::from_structured(
            item.response()
                .clone()
                .into_structured()
                .map_err(malformed)?,
        )
        .map_err(malformed)?;
        memories.push(
            EpisodeMemory::new(
                *item.episode(),
                item.evidence().clone(),
                observation,
                EpisodeOutcome::produced(response).map_err(malformed)?,
            )
            .map_err(malformed)?,
        );
    }
    fit_memory(
        *old.completed(),
        &owner,
        &memories,
        MAXIMUM_EPISODE_CHECKPOINT_BYTES,
    )
    .map(|(_, encoded)| encoded)
    .map_err(|_| EpisodeRefusal::MemoryMalformed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_checkpoint_migration_preserves_selected_evidence_and_actual_response() {
        let memory = legacy::EpisodeMemory::new(
            2,
            Some(r#"{"revision":2}"#.into()),
            legacy::EpisodeObservation::new("COPPER KITE".into(), "seen/2".into(), true).unwrap(),
            legacy::EpisodeModelResponse::new(
                "acknowledged".into(),
                "model/1".into(),
                "run/2".into(),
                false,
            )
            .unwrap(),
        )
        .unwrap();
        let old = legacy::EpisodeMemoryState::new(
            3,
            legacy::EpisodeRecall::new(BoundedSequence::try_from_iter([memory]).unwrap()).unwrap(),
            legacy::EpisodeMemoryOwner::bound("body/1".into(), "wake/1".into(), 4).unwrap(),
        )
        .unwrap()
        .encode()
        .unwrap();
        let current =
            decode_episode_checkpoint(&migrate_episode_checkpoint_v2(&old).unwrap()).unwrap();
        let memory = &current.memories().get()[0];
        assert_eq!(*current.completed(), 3);
        assert_eq!(memory.evidence().as_deref(), Some(r#"{"revision":2}"#));
        assert_eq!(memory.response().unwrap().run(), "run/2");
        assert_eq!(memory.observation().content(), "COPPER KITE");
    }
}
