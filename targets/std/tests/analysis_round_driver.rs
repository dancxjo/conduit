//! Actual ordinary Std LocalExpression kernels drive the retained round cursor.
//! Actual execution/ACK custody is retained through zero-copy round publication.
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "../../../semantics/language/tests/common/parser_revision_fixture.rs"]
mod fixture;
#[cfg(feature = "analysis-round-model")]
#[path = "../../../semantics/language/tests/common/parser_round_model_channel.rs"]
mod numeric;
use conduit_core::revision::*;
use conduit_language::{
    analysis_epoch_initialization::*,
    analysis_epoch_owner::{
        fallback_round::{cursor::*, *},
        *,
    },
    lexical::prepare_lexical_tape,
    revision::*,
    *,
};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_std_host::{analysis_round_driver::*, StdHost};
use installed_completion::*;
use retained_evidence::SpentCursorResources;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn text(value: &str) -> RevisionText<'_> {
    RevisionText::new(value).unwrap()
}
fn no_growth<T>(operation: impl FnOnce() -> T) -> T {
    let (value, count) = allocation_probe::observe(operation);
    assert_eq!(count.allocations, 0);
    assert_eq!(count.reallocations, 0);
    value
}
fn advertisement(programs: &PreparedFallbackRoundPrograms) -> conduit_core::HostAdvertisement {
    let mut host = StdHost::new().advertisement().clone();
    host.capabilities
        .extend(required_round_source_offers(programs).unwrap());
    host.capabilities.extend(
        required_auxiliary_source_offers(programs.checked_document(), AuxiliarySource::Initialize)
            .unwrap(),
    );
    host.capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    host.capabilities.dedup();
    StdHost::from_advertisement(host)
        .unwrap()
        .advertisement()
        .clone()
}
#[test]
fn ordinary_std_source_kernels_drive_full_fallback_and_authenticated_learned_cursor() {
    let source = fixture::revision("answer ", None, 7);
    let domain = DependencyRevisions {
        source_text: source.material(),
        tokens: 1,
    };
    let lexical = prepare_lexical_tape(&source, &fixture::profile(), None).unwrap();
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("analysis/std-cursor".into()).unwrap(),
        source.material().revision().clone(),
        source.material().identity().clone(),
    )
    .unwrap();
    let begin = LanguageParserBegin::new(basis, fixture::dep(), 1).unwrap();
    let programs = PreparedFallbackRoundPrograms::prepare().unwrap();
    let host = advertisement(&programs);
    let mut missing = host.clone();
    missing.capabilities.clear();
    assert!(matches!(
        PreparedAnalysisRoundDriver::prepare(&programs, &missing, DriverBounds::default()),
        Err(DriverRefusal::Preparation)
    ));
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
    let initial = PreparedAnalysisInitializationProgram::prepare()
        .unwrap()
        .replay(lexical, &begin, &actual)
        .unwrap();
    assert!(!initializer.definition().internal_plan.fragments[0]
        .fore_ports
        .is_empty());
    assert!(initializer
        .signs()
        .values()
        .flatten()
        .any(|event| event.kind == conduit_kernel::KernelEventKind::RemoteInputAdmitted));
    let mut owner = AnalysisEpochOwner::initialize(
        initial,
        &domain,
        RevisionContext {
            stream: text("analysis"),
            subject: text("utterance"),
            epoch: text("std-cursor/1"),
            producer: text("actual-std-local-source"),
            policy: text("zero-score-fallback@1"),
        },
        RevisionLimits {
            history_events: 16,
            revisable_units: 4,
        },
    )
    .unwrap();
    let mut completion =
        PreparedInstalledRoundCompletion::prepare(programs.checked_document()).unwrap();
    let before = owner.beam().clone().encode().unwrap();
    let mut driver =
        PreparedAnalysisRoundDriver::prepare(&programs, &host, DriverBounds::default()).unwrap();
    let mut cursor =
        PreparedRoundCursor::prepare_fallback(&owner, programs.checked_document(), &programs)
            .unwrap();
    let mut foreign =
        PreparedRoundCursor::prepare_fallback(&owner, programs.checked_document(), &programs)
            .unwrap();
    no_growth(|| driver.poll_fallback(&mut cursor).unwrap());
    assert_eq!(
        no_growth(|| driver.poll_fallback(&mut foreign)),
        Err(DriverRefusal::ForeignCursor)
    );
    assert!(driver.failure().is_none());
    let mut complete = false;
    for _ in 0..1_000_000 {
        if no_growth(|| driver.poll_fallback(&mut cursor).unwrap()) == DriverProgress::Complete {
            complete = true;
            break;
        }
    }
    assert!(complete, "bounded actual Std cursor did not finish");
    verify(&driver, &cursor);
    assert_eq!(owner.beam().clone().encode().unwrap(), before);
    // Explicit hosted semantic correspondence; this allocates/replays after Play.
    let observations = (0..driver.execution_count())
        .map(|i| {
            let execution = driver.execution(i).unwrap();
            FallbackOperationObservation {
                operation: execution.operation.into(),
                input: execution.input.into(),
                output: execution.output.into(),
            }
        })
        .collect();
    let hosted = owner
        .prepare_fallback_round(&programs, observations)
        .map_err(|error| error.refusal)
        .unwrap();
    assert_eq!(
        hosted.result().clone().encode().unwrap(),
        cursor.result_canonical().unwrap()
    );
    let _fallback_spent = publish_retained(&mut owner, cursor, &mut completion, &driver);
    pressure_and_cancel(&owner, &programs, &host);
    #[cfg(feature = "analysis-round-model")]
    let _learned_custody = learned(&mut owner, &programs, &host);
}
fn verify(driver: &PreparedAnalysisRoundDriver, cursor: &PreparedRoundCursor<'_>) {
    assert!(driver.execution_count() > 16);
    assert_eq!(driver.execution_count(), cursor.observation_count());
    for i in 0..driver.execution_count() {
        let actual = driver.execution(i).unwrap();
        let checked = cursor.observation(i).unwrap();
        assert_eq!(actual.ordinal, i);
        assert_eq!(actual.operation, checked.operation);
        assert_eq!(actual.input, checked.input);
        assert_eq!(actual.output, checked.output);
        assert!(actual.acknowledged_sequence.is_some());
        assert!(actual.submitted_to_cursor);
        assert!(checked.admitted);
        assert!(!actual.active_plays.is_empty());
        assert!(conduit_core::verify_plan(&actual.definition.internal_plan));
        assert_eq!(
            actual.definition.internal_plan.source_document_id,
            driver.checked_document().source_document_id
        );
        let signs = driver.signs(actual.operation).unwrap();
        assert!(signs
            .values()
            .flatten()
            .any(|event| event.kind == conduit_kernel::KernelEventKind::RemoteInputAdmitted));
        assert!(signs
            .values()
            .flatten()
            .any(|event| event.kind == conduit_kernel::KernelEventKind::RemoteValueDelivered));
    }
    assert!(driver.pending_lease().is_none());
    assert!(driver.failure().is_none());
}
fn pressure_and_cancel(
    owner: &AnalysisEpochOwner<'_>,
    programs: &PreparedFallbackRoundPrograms,
    host: &conduit_core::HostAdvertisement,
) {
    let mut cursor =
        PreparedRoundCursor::prepare_fallback(owner, programs.checked_document(), programs)
            .unwrap();
    let mut driver = PreparedAnalysisRoundDriver::prepare(
        programs,
        host,
        DriverBounds {
            observations: 1,
            ..DriverBounds::default()
        },
    )
    .unwrap();
    let reason = loop {
        match no_growth(|| driver.poll_fallback(&mut cursor)) {
            Ok(_) => (),
            Err(reason) => break reason,
        }
    };
    assert_eq!(reason, DriverRefusal::Pressure);
    assert_eq!(driver.execution_count(), 1);
    assert_eq!(cursor.progress(), CursorProgress::ReadySource);
    let before = cursor.observation_count();
    assert_eq!(
        no_growth(|| driver.poll_fallback(&mut cursor)),
        Err(DriverRefusal::Pressure)
    );
    assert_eq!(cursor.observation_count(), before);
    no_growth(|| driver.cancel().unwrap());
    assert_eq!(
        no_growth(|| driver.poll_fallback(&mut cursor)),
        Err(DriverRefusal::Closed)
    );
    let mut cursor =
        PreparedRoundCursor::prepare_fallback(owner, programs.checked_document(), programs)
            .unwrap();
    let mut driver =
        PreparedAnalysisRoundDriver::prepare(programs, host, DriverBounds::default()).unwrap();
    while driver.pending_lease().is_none() {
        no_growth(|| driver.poll_fallback(&mut cursor).unwrap());
    }
    let input = cursor
        .source_request(driver.pending_lease().unwrap())
        .unwrap()
        .input
        .to_vec();
    no_growth(|| driver.cancel().unwrap());
    assert_eq!(
        cursor
            .source_request(driver.pending_lease().unwrap())
            .unwrap()
            .input,
        input
    );
    assert!(driver.execution(0).unwrap().acknowledged_sequence.is_none());
    assert!(cursor.result_canonical().is_err());
}
#[cfg(feature = "analysis-round-model")]
fn learned<'d>(
    owner: &mut AnalysisEpochOwner<'_>,
    programs: &'d PreparedFallbackRoundPrograms,
    host: &conduit_core::HostAdvertisement,
) -> (
    SpentCursorResources<'d>,
    PreparedAnalysisRoundDriver,
    numerical::PreparedCursorLearnedChannel,
    PreparedInstalledRoundCompletion<'d>,
) {
    let mut completion =
        PreparedInstalledRoundCompletion::prepare(programs.checked_document()).unwrap();
    let profile = numeric::profile();
    let (document, catalog, definition) = numeric::definition(&profile);
    let mut channel = numerical::PreparedCursorLearnedChannel::prepare_canonical(
        &document,
        "parser-round-numeric",
        &catalog,
        definition,
        profile,
        conduit_language::analysis_epoch_owner::fallback_round::learned::ParserRoundModel::AcceptedEnglishV2,
        16,
    )
    .unwrap();
    let mut cursor = PreparedRoundCursor::prepare_learned(
        owner,
        programs.checked_document(),
        programs,
        &mut channel,
    )
    .unwrap();
    let mut driver =
        PreparedAnalysisRoundDriver::prepare(programs, host, DriverBounds::default()).unwrap();
    let mut complete = false;
    for _ in 0..1_000_000 {
        if no_growth(|| driver.poll_learned(&mut cursor, &mut channel).unwrap())
            == DriverProgress::Complete
        {
            complete = true;
            break;
        }
    }
    assert!(complete);
    verify(&driver, &cursor);
    assert!(!cursor.numerical_evidence().is_empty());
    assert!(cursor
        .numerical_evidence()
        .iter()
        .all(|evidence| !evidence.numerical().active_plays().is_empty()));
    assert!(!channel.is_stopped());
    let spent = publish_retained(owner, cursor, &mut completion, &driver);
    (spent, driver, channel, completion)
}

// Diagnostic snapshots are taken outside Play. Original driver, completion and
// returned spent storage remain held after the measured owner transaction.
fn publish_retained<'d>(
    owner: &mut AnalysisEpochOwner<'_>,
    cursor: PreparedRoundCursor<'d>,
    completion: &mut PreparedInstalledRoundCompletion<'d>,
    driver: &PreparedAnalysisRoundDriver,
) -> SpentCursorResources<'d> {
    let expected = cursor.result_canonical().unwrap().to_vec();
    let generation = owner.generation();
    let input_addresses: Vec<_> = (0..cursor.observation_count())
        .map(|index| cursor.observation(index).unwrap().input.as_ptr())
        .collect();
    #[cfg(feature = "analysis-round-model")]
    let numeric_inputs: Vec<_> = cursor
        .numerical_evidence()
        .iter()
        .map(|evidence| {
            let mut bytes = vec![0; evidence.numerical().input_len()];
            evidence.numerical().copy_input_into(&mut bytes).unwrap();
            (evidence.numerical() as *const _ as usize, bytes)
        })
        .collect();
    let (spent, growth) = allocation_probe::observe(|| {
        while completion.progress() != InstalledCompletionProgress::Complete {
            completion.step(&cursor).unwrap();
        }
        let (prepared, spent) = cursor
            .into_retained_prepared_round(owner, completion)
            .map_err(|error| error.refusal)
            .unwrap();
        owner
            .apply_fallback_round(prepared)
            .map_err(|error| error.refusal)
            .unwrap();
        spent
    });
    assert_eq!(growth.allocations, 0);
    assert_eq!(growth.reallocations, 0);
    assert_eq!(growth.deallocations, 0);
    assert_eq!(owner.generation(), generation + 1);
    let receipt = owner.fallback_rounds().last().unwrap();
    assert!(core::ptr::eq(receipt.result(), owner.beam()));
    assert_eq!(owner.beam().clone().encode().unwrap(), expected);
    let retained = receipt.retained_evidence().unwrap();
    assert_eq!(retained.result_canonical(), expected);
    assert_eq!(retained.observation_count(), driver.execution_count());
    for (index, input_address) in input_addresses.iter().enumerate() {
        let actual = driver.execution(index).unwrap();
        let checked = receipt.observations().get(index).unwrap();
        assert_eq!(checked.input.as_ptr(), *input_address);
        assert_eq!(checked.operation, actual.operation);
        assert_eq!(checked.input, actual.input);
        assert_eq!(checked.output, actual.output);
        assert!(checked.native_admitted.is_some());
        assert!(actual.acknowledged_sequence.is_some());
        assert!(actual.submitted_to_cursor);
    }
    #[cfg(feature = "analysis-round-model")]
    {
        assert!(receipt.hosted_learned_evidence().is_none());
        assert_eq!(retained.numerical_evidence().len(), numeric_inputs.len());
        for (index, (evidence, (address, expected_input))) in retained
            .numerical_evidence()
            .iter()
            .zip(&numeric_inputs)
            .enumerate()
        {
            assert_eq!(evidence.numerical() as *const _ as usize, *address);
            let frame = retained.numerical_source(index).unwrap();
            assert_eq!(frame.operation, "language-parser-scorer-model-features");
            let mut input = vec![0; evidence.numerical().input_len()];
            evidence.numerical().copy_input_into(&mut input).unwrap();
            assert_eq!(&input, expected_input);
            assert_eq!(evidence.request().generation(), generation);
            assert!(!evidence.numerical().active_plays().is_empty());
        }
    }
    spent
}

#[path = "analysis_round_driver/reserved_slots.rs"]
mod reserved_slots;

#[cfg(feature = "analysis-round-model")]
#[path = "analysis_round_driver/reserved_learned.rs"]
mod reserved_learned;
