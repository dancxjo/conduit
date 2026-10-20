//! Two future round slots and their drivers/completions are admitted together.
use super::*;
use conduit_language::analysis_epoch_owner::fallback_round::cursor::reserved_slot::*;
#[test]
fn two_same_epoch_rounds_bind_actual_prior_with_all_storage_reserved_before_play() {
    let source = fixture::revision("answer ", None, 7);
    let domain = DependencyRevisions {
        source_text: source.material(),
        tokens: 1,
    };
    let begin = LanguageParserBegin::new(
        LanguageParserBasis::new(
            LanguageAnalysisRevisionId::new("analysis/reserved-slots".into()).unwrap(),
            source.material().revision().clone(),
            source.material().identity().clone(),
        )
        .unwrap(),
        fixture::dep(),
        1,
    )
    .unwrap();
    let programs = PreparedFallbackRoundPrograms::prepare().unwrap();
    let host = advertisement(&programs);
    let mut initializer = PreparedAuxiliarySource::prepare(
        programs.checked_document(),
        &host,
        AuxiliarySource::Initialize,
    )
    .unwrap();
    let actual_bytes = initializer
        .execute_for_preparation(&begin.clone().encode().unwrap(), 16384)
        .unwrap();
    let actual = LanguageParserNumericState::decode(&actual_bytes).unwrap();
    let initialization = PreparedAnalysisInitializationProgram::prepare().unwrap();
    let make_owner = || {
        let lexical = prepare_lexical_tape(&source, &fixture::profile(), None).unwrap();
        let initial = initialization.replay(lexical, &begin, &actual).unwrap();
        AnalysisEpochOwner::initialize(
            initial,
            &domain,
            RevisionContext {
                stream: text("analysis"),
                subject: text("utterance"),
                epoch: text("reserved-slots/1"),
                producer: text("actual-std-local-source"),
                policy: text("zero-score-fallback@1"),
            },
            RevisionLimits {
                history_events: 2,
                revisable_units: 4,
            },
        )
        .unwrap()
    };
    let mut owner = make_owner();
    let foreign = make_owner();
    let initial = owner.beam().clone().encode().unwrap();
    let mut slots: [_; 3] = core::array::from_fn(|_| {
        PreparedFallbackRoundSlot::reserve(&owner, programs.checked_document(), &programs).unwrap()
    });
    let mut drivers: [_; 2] = core::array::from_fn(|_| {
        PreparedAnalysisRoundDriver::prepare(&programs, &host, DriverBounds::default()).unwrap()
    });
    let mut completions: [_; 2] = core::array::from_fn(|_| {
        PreparedInstalledRoundCompletion::prepare(programs.checked_document()).unwrap()
    });
    let mut spent = Vec::with_capacity(2);
    let (_, growth) = allocation_probe::observe(|| {
        assert!(matches!(
            slots[0].bind(&foreign),
            Err(CursorRefusal::Lifecycle)
        ));
        assert!(!slots[0].is_consumed());
        for index in 0..2 {
            let mut cursor = slots[index].bind(&owner).unwrap();
            assert!(slots[index].is_consumed());
            assert!(matches!(
                slots[index].bind(&owner),
                Err(CursorRefusal::Lifecycle)
            ));
            assert_eq!(cursor.generation(), index as u64);
            if index == 0 {
                assert_eq!(cursor.prior_canonical(), initial);
            } else {
                let previous = owner.fallback_rounds()[0].retained_evidence().unwrap();
                assert_eq!(cursor.prior_canonical(), previous.result_canonical());
                assert_ne!(cursor.prior_canonical(), previous.prior_canonical());
            }
            let mut complete = false;
            for _ in 0..1_000_000 {
                if drivers[index].poll_fallback(&mut cursor).unwrap() == DriverProgress::Complete {
                    complete = true;
                    break;
                }
            }
            assert!(complete);
            while completions[index].progress() != InstalledCompletionProgress::Complete {
                completions[index].step(&cursor).unwrap();
            }
            let (prepared, resources) = cursor
                .into_retained_prepared_round(&owner, &completions[index])
                .map_err(|error| error.refusal)
                .unwrap();
            owner
                .apply_fallback_round(prepared)
                .map_err(|error| error.refusal)
                .unwrap();
            spent.push(resources);
            assert_eq!(owner.generation(), index as u64 + 1);
        }
        assert!(matches!(
            slots[2].bind(&owner),
            Err(CursorRefusal::Pressure)
        ));
        assert!(!slots[2].is_consumed());
        assert_eq!(owner.generation(), 2);
        assert_eq!(owner.fallback_rounds().len(), 2);
    });
    assert_eq!(growth.allocations, 0);
    assert_eq!(growth.reallocations, 0);
    assert_eq!(growth.deallocations, 0);
    assert_eq!(spent.len(), 2);
    for (index, driver) in drivers.iter().enumerate() {
        let evidence = owner.fallback_rounds()[index].retained_evidence().unwrap();
        assert_eq!(driver.execution_count(), evidence.observation_count());
        assert!(driver.execution_count() > 16);
        for ordinal in 0..driver.execution_count() {
            let actual = driver.execution(ordinal).unwrap();
            let retained = evidence.observation(ordinal).unwrap();
            assert!(actual.acknowledged_sequence.is_some() && actual.submitted_to_cursor);
            assert_eq!(actual.operation, retained.operation);
            assert_eq!(actual.input, retained.input);
            assert_eq!(actual.output, retained.output);
            assert!(retained.admitted);
            assert!(conduit_core::verify_plan(&actual.definition.internal_plan));
        }
    }
    // Slots/drivers/completions/spent evidence are all deliberately held until here.
}
