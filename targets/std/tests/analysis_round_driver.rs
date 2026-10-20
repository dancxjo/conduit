//! Actual ordinary Std LocalExpression kernels drive the retained round cursor.
//! Owner receipt conversion below is hosted comparison, not installed publication.
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
    let owner = AnalysisEpochOwner::initialize(
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
    pressure_and_cancel(&owner, &programs, &host);
    #[cfg(feature = "analysis-round-model")]
    learned(&owner, &programs, &host);
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
fn learned(
    owner: &AnalysisEpochOwner<'_>,
    programs: &PreparedFallbackRoundPrograms,
    host: &conduit_core::HostAdvertisement,
) {
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
}
