use conduit_core::{kind_id, SignId, TemporalInstant, TemporalScale};
use conduit_human::*;

fn at(ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Milliseconds,
        clock_basis: "clock/experience".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
}

fn experience() -> CurrentExperience {
    CurrentExperience::new(
        ExperienceLimits {
            maximum_items: 4,
            maximum_current_items: 4,
            maximum_recent_items: 4,
            maximum_stale_items: 4,
            maximum_historical_items: 4,
            maximum_items_per_domain: 4,
            maximum_model_derived_items: 4,
            maximum_selected_memory_items: 4,
            maximum_source_refs: 2,
            maximum_relationships: 3,
            maximum_item_bytes: 16,
            maximum_encoded_bytes: 32,
            maximum_conflict_alternatives: 2,
            maximum_identity_bytes: 64,
        },
        at(100),
        ExperienceTemporalPolicy {
            maximum_current_age_ticks: 5,
            maximum_recent_age_ticks: 20,
        },
    )
    .unwrap()
}

fn item(id: &str) -> ExperienceItem {
    ExperienceItem {
        id: id.into(),
        domain: ExperienceDomain::Visual,
        content_kind: kind_id("experience/fact@1"),
        encoded_content: id.as_bytes().to_vec(),
        origin: ExperienceOrigin::Observation,
        temporal_role: ExperienceTemporalRole::Current,
        availability: ExperienceAvailability::Present,
        certainty: ExperienceCertainty::Certain,
        observed_at: Some(at(100)),
        recorded_at: None,
        sources: vec![ExperienceSourceRef::Sign(SignId::from(format!(
            "sign/{id}"
        )))],
    }
}

fn evolution(
    maximum_pending_updates: usize,
    maximum_retained_revisions: usize,
) -> EvolvingExperience {
    EvolvingExperience::new(
        experience(),
        ExperienceEvolutionLimits {
            maximum_pending_updates,
            maximum_retained_revisions,
        },
    )
    .unwrap()
}

fn admit(expected_revision: u64, id: &str) -> ExperienceUpdate {
    ExperienceUpdate {
        expected_revision: ExperienceRevision(expected_revision),
        operation: ExperienceUpdateOperation::Admit(Box::new(item(id))),
    }
}

#[test]
fn updates_apply_transactionally_at_exact_revisions() {
    let mut evolving = evolution(2, 2);
    evolving.try_enqueue(admit(0, "door")).unwrap();
    evolving.try_enqueue(admit(1, "person")).unwrap();

    assert_eq!(evolving.try_apply_next(), Ok(Some(ExperienceRevision(1))));
    assert_eq!(evolving.current().experience.items()[0].id, "door");
    assert_eq!(evolving.try_apply_next(), Ok(Some(ExperienceRevision(2))));
    assert_eq!(evolving.current().experience.items().len(), 2);
    assert_eq!(evolving.retained()[0].revision, ExperienceRevision(0));
    assert_eq!(evolving.retained()[1].revision, ExperienceRevision(1));
}

#[test]
fn stale_future_and_queue_pressure_refuse_with_update_ownership() {
    let mut evolving = evolution(1, 1);
    let stale = admit(1, "future");
    let error = evolving.try_enqueue(stale.clone()).unwrap_err();
    assert_eq!(error.refusal, ExperienceUpdateRefusal::FutureRevision);
    assert_eq!(*error.update, stale);

    evolving.try_enqueue(admit(0, "first")).unwrap();
    let pressured = admit(1, "second");
    let error = evolving.try_enqueue(pressured.clone()).unwrap_err();
    assert_eq!(
        error.refusal,
        ExperienceUpdateRefusal::PendingUpdateCapacity
    );
    assert_eq!(*error.update, pressured);

    evolving.try_apply_next().unwrap();
    let stale = admit(0, "stale");
    assert_eq!(
        evolving.try_enqueue(stale).unwrap_err().refusal,
        ExperienceUpdateRefusal::StaleRevision
    );
}

#[test]
fn failed_application_leaves_current_output_unchanged() {
    let mut evolving = evolution(1, 1);
    evolving.try_enqueue(admit(0, "door")).unwrap();
    evolving.try_apply_next().unwrap();
    evolving.try_enqueue(admit(1, "door")).unwrap();

    let error = evolving.try_apply_next().unwrap_err();
    assert_eq!(
        error.refusal,
        ExperienceUpdateRefusal::Experience(ExperienceRefusal::DuplicateIdentity)
    );
    assert_eq!(*error.update, admit(1, "door"));
    assert_eq!(evolving.current().revision, ExperienceRevision(1));
    assert_eq!(evolving.current().experience.items().len(), 1);
}

#[test]
fn retained_output_history_evicts_the_oldest_exact_revision() {
    let mut evolving = evolution(1, 2);
    for (revision, id) in [(0, "one"), (1, "two"), (2, "three")] {
        evolving.try_enqueue(admit(revision, id)).unwrap();
        evolving.try_apply_next().unwrap();
    }

    assert_eq!(
        evolving
            .retained()
            .iter()
            .map(|snapshot| snapshot.revision)
            .collect::<Vec<_>>(),
        vec![ExperienceRevision(1), ExperienceRevision(2)]
    );
    assert_eq!(evolving.current().revision, ExperienceRevision(3));
}

#[test]
fn source_removal_is_a_revision_only_when_it_changes_semantic_state() {
    let mut evolving = evolution(1, 1);
    evolving.try_enqueue(admit(0, "door")).unwrap();
    evolving.try_apply_next().unwrap();
    evolving
        .try_enqueue(ExperienceUpdate {
            expected_revision: ExperienceRevision(1),
            operation: ExperienceUpdateOperation::RemoveSource(ExperienceSourceRef::Sign(
                SignId::from("sign/missing"),
            )),
        })
        .unwrap();

    let error = evolving.try_apply_next().unwrap_err();
    assert_eq!(error.refusal, ExperienceUpdateRefusal::NoSemanticChange);
    assert_eq!(
        *error.update,
        ExperienceUpdate {
            expected_revision: ExperienceRevision(1),
            operation: ExperienceUpdateOperation::RemoveSource(ExperienceSourceRef::Sign(
                SignId::from("sign/missing")
            )),
        }
    );
    assert_eq!(evolving.current().revision, ExperienceRevision(1));
}

#[test]
fn empty_update_queue_has_no_terminal_or_failure_disposition() {
    assert_eq!(evolution(1, 1).try_apply_next(), Ok(None));
}

#[test]
fn evolution_requires_finite_nonzero_queue_and_history_bounds() {
    for limits in [
        ExperienceEvolutionLimits {
            maximum_pending_updates: 0,
            maximum_retained_revisions: 1,
        },
        ExperienceEvolutionLimits {
            maximum_pending_updates: 1,
            maximum_retained_revisions: 0,
        },
    ] {
        assert_eq!(
            EvolvingExperience::new(experience(), limits),
            Err(ExperienceUpdateRefusal::InvalidLimits)
        );
    }
}
