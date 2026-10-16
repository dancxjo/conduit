use conduit_audio::*;
use conduit_core::projection::*;
#[path = "common/pitch_signal.rs"]
mod pitch_signal;
use pitch_signal::*;
#[test]
fn generic_fidelity_report_requires_declared_reduction_and_exact_source_target_pair() {
    let profile = PcmPitchProfile::new(55, 16000, 1024, 0, 1024, 8, 256).unwrap();
    let source = frame(&signal(1024, 40.0, 0.5, 0.0), 16000, 100);
    let observation = profile
        .observe(&mut PcmPitchWorkspace::default(), &source)
        .unwrap()
        .prepare_native()
        .unwrap();
    let domain = PcmPitchEvidence::bind(profile, &source, &observation).unwrap();
    let facts = domain.facts();
    let native = domain.native();
    let policy = PcmPitchObservationPolicy;
    let text = |s| ProjectionText::new(s).unwrap();
    let build = |facts| {
        ProjectionReport::new(
            &domain,
            &policy,
            ProjectionInput {
                basis: ProjectionBasis {
                    report: text("pitch-report"),
                    source: text("retained-pcm"),
                    target: Some(text("emitted-observation")),
                    projector: text(PCM_PITCH_PROFILE),
                    requested_route: text("signal-observation"),
                    boundary: None,
                },
                source: domain.source(),
                target: Some(domain.observation()),
                facts,
                native: &native,
                scores: &[],
                admitted: &[],
                attempts: &[],
                selected_attempt: None,
                mechanism: ProjectionMechanism::Completed,
                diagnostics: &[],
            },
        )
    };
    let report = build(&facts).unwrap();
    assert!(report.require_exact().is_err());
    assert!(build(&facts[..2]).is_err());
    let mut forged = domain.facts();
    forged[2] = ProjectionFact::Preserved {
        obligation: text("waveform"),
    };
    assert!(build(&forged).is_err());
    let changed = frame(&signal(1024, 80.0, 0.5, 0.0), 16000, 100);
    assert!(matches!(
        PcmPitchEvidence::bind(profile, &changed, &observation),
        Err(PcmPitchEvidenceRefusal::ObservationMismatch)
    ));
}
