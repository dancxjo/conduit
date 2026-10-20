//! Two future round slots and their drivers/completions are admitted together.
use super::*;
use conduit_language::analysis_epoch_owner::fallback_round::cursor::reserved_slot::*;
#[test]
fn two_learned_rounds_bind_one_actual_channel_with_storage_reserved_before_play() {
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
                policy: text("authenticated-categorical@1"),
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
    let profile = numeric::profile();
    let (document, catalog, definition) = numeric::definition(&profile);
    let prepare_channel = |capacity| {
        numerical::PreparedCursorLearnedChannel::prepare_canonical(
            &document,
            "parser-round-numeric",
            &catalog,
            definition.clone(),
            profile.clone(),
            learned::ParserRoundModel::AcceptedEnglishV2,
            capacity,
        )
        .unwrap()
    };
    let mut channel = prepare_channel(32);
    let mut foreign_channel = prepare_channel(32);
    let mut pressured_channel = prepare_channel(1);
    let mut pressured_slot = PreparedLearnedRoundSlot::reserve(
        &owner,
        programs.checked_document(),
        &programs,
        &mut pressured_channel,
    )
    .unwrap();
    let mut slots: [_; 2] = core::array::from_fn(|_| {
        PreparedLearnedRoundSlot::reserve(
            &owner,
            programs.checked_document(),
            &programs,
            &mut channel,
        )
        .unwrap()
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
            pressured_slot.bind(&owner, &mut pressured_channel),
            Err(CursorRefusal::Pressure)
        ));
        assert!(!pressured_slot.is_consumed());
        assert!(pressured_channel.pending().is_none());
        assert_eq!(pressured_channel.numerical().available_slots(), 1);
        assert!(matches!(
            slots[0].bind(&owner, &mut foreign_channel),
            Err(CursorRefusal::ForeignLease)
        ));
        assert!(matches!(
            slots[0].bind(&foreign, &mut channel),
            Err(CursorRefusal::Lifecycle)
        ));
        assert!(!slots[0].is_consumed());
        for index in 0..2 {
            let mut cursor = slots[index].bind(&owner, &mut channel).unwrap();
            assert!(slots[index].is_consumed());
            assert_eq!(cursor.generation(), index as u64);
            if index == 1 {
                assert_eq!(
                    cursor.prior_canonical(),
                    owner.fallback_rounds()[0]
                        .retained_evidence()
                        .unwrap()
                        .result_canonical()
                );
            }
            let mut complete = false;
            for _ in 0..1_000_000 {
                let progress = drivers[index].poll_learned(&mut cursor, &mut channel).unwrap_or_else(|reason| {
                    panic!("round={index} reason={reason:?} kernel={:?} pending_invocation={:?} cursor_evidence={} free_slots={}",
                        channel.numerical().last_kernel_error(),
                        channel.pending().map(|lease|lease.invocation()),
                        cursor.numerical_evidence().len(), channel.numerical().available_slots());
                });
                if progress == DriverProgress::Complete {
                    complete = true;
                    break;
                }
            }
            assert!(complete);
            assert!(!cursor.numerical_evidence().is_empty());
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
        assert!(!channel.is_stopped());
        assert!(channel.pending().is_none());
    });
    assert_eq!(growth.allocations, 0);
    assert_eq!(growth.reallocations, 0);
    assert_eq!(growth.deallocations, 0);
    assert_eq!(spent.len(), 2);
    let mut previous_invocation = 0;
    for (index, driver) in drivers.iter().enumerate() {
        let retained = owner.fallback_rounds()[index].retained_evidence().unwrap();
        assert_eq!(retained.observation_count(), driver.execution_count());
        for (ordinal, evidence) in retained.numerical_evidence().iter().enumerate() {
            let invocation = evidence.request().invocation();
            assert_eq!(invocation, previous_invocation + 1);
            previous_invocation = invocation;
            assert_eq!(evidence.request().generation(), index as u64);
            assert!(!evidence.numerical().active_plays().is_empty());
            assert_eq!(
                retained.numerical_source(ordinal).unwrap().operation,
                "language-parser-scorer-model-features"
            );
        }
        for ordinal in 0..driver.execution_count() {
            let actual = driver.execution(ordinal).unwrap();
            let checked = retained.observation(ordinal).unwrap();
            assert!(actual.acknowledged_sequence.is_some() && actual.submitted_to_cursor);
            assert_eq!(checked.input, actual.input);
            assert_eq!(checked.output, actual.output);
            assert!(checked.admitted);
        }
    }
    assert!(previous_invocation > 4 && previous_invocation <= 32);
}
