use conduit_audio::*;
use conduit_core::projection::*;
use conduit_plot::rust_binding::NativeRustBinding;
#[allow(dead_code)]
#[path = "common/periodic.rs"]
mod fixture;
use fixture::{request, started, Pipeline};
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}

fn admit(candidate: &AudioPeriodicCursorCandidate) -> AudioPeriodicCursor {
    let raw = candidate.profile().projection();
    let projection = AudioPeriodicProjection::new(
        *raw.amplitude_q15(),
        *raw.amplitude_remainder(),
        *raw.period_q8(),
        *raw.period_remainder(),
        raw.request().clone(),
    )
    .unwrap();
    AudioPeriodicCursor::new(
        *candidate.emitted(),
        *candidate.initial_phase_q8(),
        *candidate.phase_q8(),
        *candidate.phase_remainder(),
        AudioPeriodicProfile::new(projection).unwrap(),
        *candidate.remaining(),
    )
    .unwrap()
}

#[test]
fn existing_source_exact_and_fractional_profiles_have_truthful_generic_fidelity() {
    let mut prepare = Pipeline::new(
        "dsp/oscillator-prepare",
        AudioPeriodicRequest::semantic_type().unwrap(),
    );
    for (request, exact, loss_count) in [
        (request(125, 1, 7, 4, 1, 1, 4), true, 0),
        (request(120, 1, -1, 3, 1, 3, 4), false, 3),
    ] {
        let original = request.clone().encode().unwrap();
        let cursor = admit(&started(&prepare.run(original.clone())));
        let domain = AudioPeriodicNumericProjection::new(&cursor).unwrap();
        let facts = domain.facts();
        let policy = AudioPeriodicNumericPolicy;
        let input = || ProjectionInput {
            basis: ProjectionBasis {
                report: text("report"),
                source: text("source"),
                target: Some(text("target")),
                projector: text("audio/periodic"),
                requested_route: text("numeric-profile"),
                boundary: None,
            },
            source: domain.source(),
            target: Some(domain.target()),
            facts: &facts,
            native: &[],
            scores: &[],
            admitted: &[],
            attempts: &[],
            selected_attempt: None,
            mechanism: ProjectionMechanism::Completed,
            diagnostics: &[],
        };
        let report = ProjectionReport::new(&domain, &policy, input()).unwrap();
        assert_eq!(report.require_exact().is_ok(), exact);
        assert_eq!(
            report.summary().losses[ProjectionLoss::Precision as usize],
            loss_count
        );
        assert_eq!(
            domain.target().request().clone().encode().unwrap(),
            original
        );
        assert_eq!(
            domain.target().request().phase().numerator_turns(),
            request.phase().numerator_turns()
        );
        assert!(report
            .require_policy(text("audio/explicit-periodic-integer-quantization@1"))
            .is_ok());
        let mut omitted = input();
        omitted.facts = &facts[..5];
        assert!(ProjectionReport::new(&domain, &policy, omitted).is_err());
    }
}
#[test]
fn phase_remainder_is_independent_and_advanced_cursors_refuse_initial_report() {
    let mut prepare = Pipeline::new(
        "dsp/oscillator-prepare",
        AudioPeriodicRequest::semantic_type().unwrap(),
    );
    let candidate = started(&prepare.run(request(125, 1, -1, 3, 1, 1, 4).encode().unwrap()));
    let cursor = admit(&candidate);
    let report = AudioPeriodicNumericProjection::new(&cursor).unwrap();
    let facts = report.facts();
    assert!(matches!(facts[3], ProjectionFact::Transformed { .. }));
    assert!(matches!(facts[4], ProjectionFact::Transformed { .. }));
    assert!(matches!(
        facts[5],
        ProjectionFact::Lost {
            class: ProjectionLoss::Precision,
            ..
        }
    ));
    let mut step = Pipeline::new(
        "dsp/oscillator-step",
        AudioPeriodicStepInput::semantic_type().unwrap(),
    );
    let bytes = step.run(
        AudioPeriodicStepInput::new(false, candidate)
            .unwrap()
            .encode()
            .unwrap(),
    );
    let AudioPeriodicOutcome::Advanced(next) = AudioPeriodicOutcome::decode(&bytes).unwrap() else {
        panic!("advance")
    };
    assert!(matches!(
        AudioPeriodicNumericProjection::new(&admit(next.cursor())),
        Err(PeriodicNumericReportRefusal::AdvancedCursor)
    ));
}
