//! Preparation correspondence tests; these do not claim physical acquisition.
use crate::*;
use conduit_core::{SignId, TemporalInstant, TemporalScale};
use conduit_human::*;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_semantic_catalog::{
    BoundedExperienceEvidenceProjection, BoundedExperiencePrompt, BoundedExperienceSituation,
    EXPERIENCE_PROMPT_MAX_BYTES,
};
fn at(ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Milliseconds,
        clock_basis: "pete/test-clock".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
}
fn observed<T>(value: T, sign: &str, ticks: u64) -> SituationInput<T> {
    SituationInput::Observed(Observed {
        value,
        sign_id: SignId::from(sign),
        observed_at: at(ticks),
    })
}
fn runtime(
    frame: &ExperienceSituationFrame,
) -> (ExperienceEvidenceRequest, ExperienceEvidenceContext) {
    let mut situation = BoundedExperienceSituation::prepare();
    situation.execute(&frame.clone().encode().unwrap()).unwrap();
    let request = ExperienceEvidenceRequest::decode(situation.situation()).unwrap();
    let mut projection = BoundedExperienceEvidenceProjection::prepare();
    let context =
        ExperienceEvidenceContext::decode(projection.execute(situation.situation()).unwrap())
            .unwrap();
    let event = ExperienceAutobiographyEvent::decode(situation.event()).unwrap();
    assert_eq!(event.observation(), &request);
    assert_eq!(*event.retain(), *frame.retain());
    (request, context)
}
fn find<'a>(request: &'a ExperienceEvidenceRequest, id: &str) -> &'a ExperienceEvidenceItem {
    request
        .evidence()
        .get()
        .as_slice()
        .iter()
        .find(|item| item.identity().get() == id)
        .unwrap()
}
#[test]
fn established_battery_human_body_values_preserve_sources_and_freshness() {
    let situation = select_pete_situation(
        at(200),
        20,
        observed(
            conduit_robotics::BatteryObservation::new(750, 14300).unwrap(),
            "battery/198",
            198,
        ),
        observed("Pete, wait".into(), "human/190", 190),
        observed(
            PeteBodyState {
                motion_available: false,
                safety_inhibited: true,
            },
            "body/100",
            100,
        ),
    )
    .unwrap();
    let frame = pete_situation_frame(
        &situation,
        ExperienceTemporalPolicy::new(20, 50).unwrap(),
        7,
        "pete/revision/7",
        true,
    )
    .unwrap();
    assert_eq!(frame.observations().get().as_slice().len(), 3);
    let (request, context) = runtime(&frame);
    assert_eq!(*context.selection().revision(), 7);
    let battery = find(&request, "pete/battery/battery/198");
    assert_eq!(
        battery.content_kind().get(),
        conduit_robotics::ROBOTICS_BATTERY_INFO_ID
    );
    assert_eq!(battery.temporal_role(), &ExperienceTemporalRole::Current);
    let value: serde_json::Value =
        serde_json::from_slice(battery.encoded_content().get().as_slice()).unwrap();
    assert_eq!(value["charge_permille"], 750);
    assert_eq!(value["millivolts"], 14300);
    assert_eq!(
        battery.sources().get().as_slice()[0],
        ExperienceEvidenceSource::sign(
            ExperienceEvidenceIdentity::new("battery/198".into()).unwrap()
        )
        .unwrap()
    );
    let utterance = find(&request, "pete/utterance/human/190");
    assert_eq!(utterance.origin(), &ExperienceOrigin::HumanReported);
    let human: serde_json::Value =
        serde_json::from_slice(utterance.encoded_content().get().as_slice()).unwrap();
    assert_eq!(human["text"], "Pete, wait");
    let body = find(&request, "pete/body/body/100");
    assert_eq!(body.temporal_role(), &ExperienceTemporalRole::Stale);
    let body_value: serde_json::Value =
        serde_json::from_slice(body.encoded_content().get().as_slice()).unwrap();
    assert_eq!(body_value["motion_available"], false);
    assert_eq!(body_value["safety_inhibited"], true);
    // Exact temporal nodes are carried in the ordinary evidence source, rather
    // than inferred from item names or hidden clock reads.
    let encoded = request.encode().unwrap();
    let view = conduit_core::validate_canonical_structured_value(&encoded).unwrap();
    let source = view
        .record_field("evidence")
        .unwrap()
        .unwrap()
        .collection_index(0)
        .unwrap()
        .unwrap()
        .record_field("sources")
        .unwrap()
        .unwrap()
        .collection_index(1)
        .unwrap()
        .unwrap()
        .variant_payload("temporal_context")
        .unwrap()
        .unwrap();
    let observed = conduit_core::BorrowedTemporalInstant::from_canonical(
        source.record_field("observed_at").unwrap().unwrap(),
    )
    .unwrap();
    assert_eq!(observed.clock_basis, "pete/test-clock");
    assert_eq!(observed.ticks, 198);
    let reference = conduit_core::BorrowedTemporalInstant::from_canonical(
        source.record_field("reference_at").unwrap().unwrap(),
    )
    .unwrap();
    assert_eq!(reference.ticks, 200);
}
#[test]
fn contradictory_human_alternatives_remain_separate_uncertain_items() {
    let situation = select_pete_situation(
        at(200),
        20,
        SituationInput::Missing,
        SituationInput::Contradictory {
            first: Observed {
                value: "door open".into(),
                sign_id: "human/1".into(),
                observed_at: at(195),
            },
            second: Observed {
                value: "door closed".into(),
                sign_id: "human/2".into(),
                observed_at: at(196),
            },
        },
        SituationInput::Unavailable {
            source_identity: "body/source".into(),
        },
    )
    .unwrap();
    let frame = pete_situation_frame(
        &situation,
        ExperienceTemporalPolicy::new(20, 50).unwrap(),
        3,
        "pete/revision/3",
        false,
    )
    .unwrap();
    let (request, _) = runtime(&frame);
    let first = find(&request, "pete/utterance/human/1");
    let second = find(&request, "pete/utterance/human/2");
    assert_eq!(first.origin(), &ExperienceOrigin::HumanReported);
    assert_eq!(second.origin(), &ExperienceOrigin::HumanReported);
    assert_eq!(first.certainty(), &ExperienceCertainty::Uncertain);
    assert_eq!(second.certainty(), &ExperienceCertainty::Uncertain);
    assert_ne!(first.sources(), second.sources());
    assert_ne!(first.encoded_content(), second.encoded_content());
    assert_eq!(request.evidence().get().as_slice().len(), 4);
}
#[test]
fn missing_and_unavailable_have_absent_time_and_no_fabricated_observation() {
    let situation = select_pete_situation(
        at(200),
        20,
        SituationInput::Missing,
        SituationInput::Missing,
        SituationInput::Unavailable {
            source_identity: "body/source".into(),
        },
    )
    .unwrap();
    let frame = pete_situation_frame(
        &situation,
        ExperienceTemporalPolicy::new(20, 50).unwrap(),
        0,
        "pete/initial",
        false,
    )
    .unwrap();
    assert!(frame
        .observations()
        .get()
        .as_slice()
        .iter()
        .all(|item| item.time() == &ExperienceSituationTime::Absent));
    let (request, _) = runtime(&frame);
    for id in ["pete/battery/missing", "pete/utterance/missing"] {
        let item = find(&request, id);
        assert_eq!(item.availability(), &ExperienceAvailability::NotObserved);
        assert!(item.sources().get().as_slice().is_empty());
        assert!(item.encoded_content().get().as_slice().is_empty());
    }
    let unavailable = find(&request, "pete/body/unavailable");
    assert_eq!(
        unavailable.availability(),
        &ExperienceAvailability::SourceUnavailable
    );
    assert_eq!(
        unavailable.sources().get().as_slice(),
        &[ExperienceEvidenceSource::source(
            ExperienceEvidenceIdentity::new("body/source".into()).unwrap()
        )
        .unwrap()]
    );
    assert!(unavailable.encoded_content().get().as_slice().is_empty());
    assert_eq!(unavailable.certainty(), &ExperienceCertainty::Uncertain);
}
#[test]
fn uncertainty_survives_preparation_and_context_reaches_bounded_prompt() {
    let mut uncertainty = at(180);
    uncertainty.uncertainty_ticks = 5;
    let situation = select_pete_situation(
        at(200),
        20,
        SituationInput::Missing,
        SituationInput::Observed(Observed {
            value: "uncertain sound".into(),
            sign_id: "human/uncertain".into(),
            observed_at: uncertainty,
        }),
        SituationInput::Missing,
    )
    .unwrap();
    assert_eq!(situation.utterance.state, SituationFactState::Indeterminate);
    let frame = pete_situation_frame(
        &situation,
        ExperienceTemporalPolicy::new(20, 50).unwrap(),
        9,
        "pete/revision/9",
        true,
    )
    .unwrap();
    let (request, context) = runtime(&frame);
    let human = find(&request, "pete/utterance/human/uncertain");
    assert_eq!(
        human.temporal_role(),
        &ExperienceTemporalRole::Indeterminate
    );
    assert_eq!(human.certainty(), &ExperienceCertainty::Uncertain);
    let mut prompt = BoundedExperiencePrompt::prepare();
    let output = prompt.execute(&context.encode().unwrap()).unwrap();
    assert!(output.len() <= EXPERIENCE_PROMPT_MAX_BYTES);
    let text = core::str::from_utf8(output).unwrap();
    assert!(text.contains("uncertain sound"));
    assert!(text.contains("indeterminate"));
    assert!(text.contains("pete/test-clock"));
    assert!(text.contains("human/uncertain"));
}
#[test]
fn incomparable_clock_is_refused_without_fabricating_temporal_role() {
    let mut situation = select_pete_situation(
        at(200),
        20,
        SituationInput::Missing,
        observed("hello".into(), "human/1", 190),
        SituationInput::Missing,
    )
    .unwrap();
    situation.utterance.provenance[0].observed_at.clock_basis = "other/clock".into();
    assert_eq!(
        pete_situation_frame(
            &situation,
            ExperienceTemporalPolicy::new(20, 50).unwrap(),
            0,
            "pete/initial",
            true
        ),
        Err(PeteExperienceRefusal::InvalidFact)
    );
}
