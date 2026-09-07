use conduit_core::*;

fn generation(entry: &str, generation: &str, name: &str, at: u64) -> ResourceGeneration {
    ResourceGeneration {
        entry_id: ResourceEntryId(entry.into()),
        generation_id: ResourceGenerationId(generation.into()),
        exact_name: name.into(),
        observed_at: at,
        relation: None,
    }
}

#[test]
fn bounded_collection_selects_latest_without_mutating_generations() {
    let mut collection =
        ResourceCollection::new(ResourceCollectionId("sessions".into()), 3).unwrap();
    collection
        .publish(generation("session", "g1", "today", 1))
        .unwrap();
    collection
        .publish(generation("session", "g2", "today", 2))
        .unwrap();
    let found = collection
        .select(&ResourceSelection {
            exact_name: Some("today"),
            time_window: None,
            relation: None,
            latest_only: true,
            maximum_candidates: 2,
            maximum_results: 1,
        })
        .unwrap();
    assert_eq!(found[0].generation_id, ResourceGenerationId("g2".into()));
    assert_eq!(
        collection.publish(generation("other", "g3", "template", 3)),
        Ok(())
    );
    assert_eq!(
        collection.publish(generation("overflow", "g4", "x", 4)),
        Err(ResourceCollectionRefusal::CollectionFull)
    );
}

#[test]
fn selection_refuses_ambiguity_and_work_beyond_admission() {
    let mut collection = ResourceCollection::new(ResourceCollectionId("forms".into()), 3).unwrap();
    collection
        .publish(generation("a", "ga", "reviewed", 4))
        .unwrap();
    collection
        .publish(generation("b", "gb", "reviewed", 4))
        .unwrap();
    let mut query = ResourceSelection {
        exact_name: Some("reviewed"),
        time_window: Some((0, 5)),
        relation: None,
        latest_only: true,
        maximum_candidates: 2,
        maximum_results: 2,
    };
    assert_eq!(
        collection.select(&query),
        Err(ResourceCollectionRefusal::AmbiguousLatest)
    );
    query.latest_only = false;
    query.maximum_candidates = 1;
    assert_eq!(
        collection.select(&query),
        Err(ResourceCollectionRefusal::CandidateBoundExceeded)
    );
}

#[test]
fn acquisition_distinguishes_all_terminal_paths_and_stale_replacements() {
    let requested = ResourceAcquisitionState::Discoverable
        .request("request-1".into())
        .unwrap();
    let current = requested
        .acquire(AcquiredResource {
            identity: "camera".into(),
            generation: 1,
        })
        .unwrap();
    assert_eq!(current.accepts("camera", 1), Ok(()));
    assert_eq!(
        current.clone().release(),
        Ok(ResourceAcquisitionState::Released { generation: 1 })
    );
    assert_eq!(
        current.clone().revoke(),
        Ok(ResourceAcquisitionState::Revoked { generation: 1 })
    );
    let lost = current.lose().unwrap();
    let replacement = lost
        .request("request-2".into())
        .unwrap()
        .acquire(AcquiredResource {
            identity: "camera".into(),
            generation: 2,
        })
        .unwrap();
    assert_eq!(
        replacement.accepts("camera", 1),
        Err(ResourceAcquisitionRefusal::StaleGeneration)
    );
}
