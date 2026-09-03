use conduit_tongues::{check_research_forms, run_dynamics_analysis, ANALYSIS_FORM_SOURCE};

#[test]
fn analysis_is_exact_bounded_and_tied_to_the_frozen_artifacts() {
    let first = run_dynamics_analysis().expect("bounded analysis");
    let second = run_dynamics_analysis().expect("reproducible bounded analysis");
    assert_eq!(first.identity, second.identity);
    assert!(first.source_checkpoint_identity.starts_with("sha256:"));
    assert_eq!(first.work_bound_frames, 192);
    assert!(first.phase_lag.best_lag_bins.abs() <= 3);
    assert_eq!(
        first.events.discovered_events,
        first.events.event_bins.len()
    );
    assert_eq!(
        first.categories.test_frames,
        first.categories.test_assignments.len()
    );
    assert!(!first.categories.labels_visible_during_clustering);
    assert_eq!(first.sparse_dynamics.library.len(), 6);
    assert!(first.sparse_dynamics.nonzero_terms < 12);
    assert_eq!(
        first
            .sparse_dynamics
            .held_out_observed_delta_millionths
            .len(),
        first
            .sparse_dynamics
            .held_out_predicted_delta_millionths
            .len()
    );
    assert!(first.robustness.cross_speaker.contains("not-identifiable"));
    assert!(first
        .theory_comparisons
        .iter()
        .any(|result| result.disposition.contains("contradicted")
            || result.disposition == "not-identifiable"));
}

#[test]
fn labels_enter_only_the_post_freeze_analysis_surface() {
    let forms = check_research_forms().expect("all research Forms check");
    assert_eq!(forms.len(), 3);
    assert_eq!(forms[2].gears.len(), 4);
    assert!(!ANALYSIS_FORM_SOURCE.contains("phone"));
    assert!(!ANALYSIS_FORM_SOURCE.contains("syllable"));
    assert!(ANALYSIS_FORM_SOURCE.contains("post-freeze"));
}
