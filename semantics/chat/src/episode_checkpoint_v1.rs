//! Upgrade the previous checkpoint without inventing historical evidence.
use super::*;
use conduit_core::StructuredInfoType;

#[allow(dead_code)]
mod legacy {
    include!(concat!(env!("OUT_DIR"), "/episode_checkpoint_v1.rs"));
}

pub fn episode_checkpoint_v1_type() -> StructuredInfoType {
    legacy::EpisodeMemoryState::semantic_type().expect("previous episode checkpoint Type")
}

pub fn migrate_episode_checkpoint_v1(bytes: &[u8]) -> Result<Vec<u8>, EpisodeRefusal> {
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
    let mut memories = BoundedSequence::new();
    let mut previous = 0;
    for item in old.memories().get() {
        let episode = *item.episode();
        if episode <= previous || episode > *old.completed() {
            return Err(EpisodeRefusal::MemoryMalformed);
        }
        previous = episode;
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
        memories
            .push(EpisodeMemory::new(episode, None, observation, response).map_err(malformed)?)
            .map_err(|_| EpisodeRefusal::MemoryMalformed)?;
    }
    let encoded = EpisodeMemoryState::new(
        *old.completed(),
        EpisodeRecall::new(memories).map_err(malformed)?,
        owner,
    )
    .and_then(NativeRustBinding::encode)
    .map_err(malformed)?;
    if encoded.len() > MAXIMUM_EPISODE_CHECKPOINT_BYTES {
        return Err(EpisodeRefusal::MemoryMalformed);
    }
    Ok(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn episode_loop_migrates_the_exact_previous_checkpoint_profile() {
        assert_eq!(
            episode_checkpoint_v1_type().profile().unwrap().value_kind().as_str(),
            "structured-info/profile-6ef4de183666f3723fc1021db14c2b39ab7ea236d4ea3cd4bea9f40fbd329556@1"
        );
        let old_memory = legacy::EpisodeMemory::new(
            3,
            legacy::EpisodeObservation::new("COPPER KITE".into(), "utterance/3".into(), true)
                .unwrap(),
            legacy::EpisodeModelResponse::new(
                "Recorded.".into(),
                "model".into(),
                "run/3".into(),
                false,
            )
            .unwrap(),
        )
        .unwrap();
        let old = legacy::EpisodeMemoryState::new(
            5,
            legacy::EpisodeRecall::new(BoundedSequence::try_from_iter([old_memory]).unwrap())
                .unwrap(),
            legacy::EpisodeMemoryOwner::bound("body".into(), "wake".into(), 2).unwrap(),
        )
        .unwrap()
        .encode()
        .unwrap();
        let upgraded = migrate_episode_checkpoint_v1(&old).unwrap();
        let memory = decode_episode_checkpoint(&upgraded).unwrap();
        assert_eq!(*memory.completed(), 5);
        assert_eq!(
            memory.owner(),
            &EpisodeMemoryOwner::bound("body".into(), "wake".into(), 2).unwrap()
        );
        let [item] = memory.memories().get().as_slice() else {
            panic!("expected the retained legacy memory")
        };
        assert_eq!(*item.episode(), 3);
        assert_eq!(item.observation().content(), "COPPER KITE");
        assert_eq!(item.response().run(), "run/3");
        assert!(item.evidence().is_none());
    }
}
