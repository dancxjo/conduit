//! A refused numeric comparison still explains ordinary admission and provenance.
use super::*;

#[test]
fn refused_comparison_retains_hard_rejections_costs_and_sources_without_request_contents() {
    for mode in [CalibrationMode::None, CalibrationMode::Fresh] {
        exercise_with_budget(false, mode, Some(0), |result| {
            let error = result.unwrap_err();
            let text = render_cost_selection_refusal(&error).unwrap();
            assert!(text.contains("no cost-admitted winner"));
            assert!(text.contains("WorkLimit"));
            assert!(text.contains("Rejected(QueueByteBound)"));
            assert!(text.contains("forecast/stale") && text.contains("forecast/fresh"));
            assert!(text.contains("upper_fulfillment_seconds="));
            assert!(!text.contains("REQUEST_CANARY") && !text.contains("hard_admission=Selected"));
            let CostSelectionError::Comparison(mut refusal) = error else {
                panic!("comparison account");
            };
            if matches!(mode, CalibrationMode::Fresh) {
                assert!(text.contains("condition/current") && text.contains("completion/measured"));
            }
            let original_boot = refusal.operands[0].source.boot_id.clone();
            refusal.operands[0].source.boot_id = "forged/boot".into();
            let mut error = CostSelectionError::Comparison(refusal);
            assert_eq!(
                render_cost_selection_refusal(&error),
                Err(CostExplanationRefusal::InconsistentSelection)
            );
            let CostSelectionError::Comparison(ref mut refusal) = error else {
                unreachable!()
            };
            refusal.operands[0].source.boot_id = original_boot;
            refusal.arithmetic_work_remaining = 1;
            assert_eq!(
                render_cost_selection_refusal(&error),
                Err(CostExplanationRefusal::InconsistentSelection)
            );
        });
    }
}
