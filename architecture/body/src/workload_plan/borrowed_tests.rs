//! Original normalization and error precedence without deep Plan ownership.
use super::*;
use alloc::{format, vec};
fn fixture() -> (Wake, BodyPlan) {
    let plots = (0..2)
        .map(|index| {
            let source = format!("source/{index}");
            let checked = format!("checked/{index}");
            let plan = conduit_core::seal_plan(
                conduit_core::PlotIdentity {
                    source_document_id: source.clone().into(),
                    checked_plot_id: checked.clone().into(),
                    expanded_plot_id: format!("expanded/{index}").into(),
                },
                vec![],
            );
            BodyPlotPlan {
                plot: ResidentPlot::new(source.into(), checked.into()),
                plan,
            }
        })
        .collect::<Vec<_>>();
    let workset = BodyWorkset::from_plots(plots.iter().map(|part| part.plot.clone())).unwrap();
    let body = crate::Body::born_with_plots(workset, 1, "sign/born".into()).unwrap();
    let wake = body.wake(1, "sign/woke".into()).unwrap().1;
    let plan = BodyPlan::seal(&wake, plots).unwrap();
    (wake, plan)
}
#[test]
fn borrowed_validation_keeps_original_unordered_plot_normalization() {
    let (wake, mut plan) = fixture();
    plan.plots.reverse();
    assert_eq!(plan.validate_for(&wake), Ok(()));
    assert_eq!(plan.validate_for_borrowed(&wake), plan.validate_for(&wake));
}
#[test]
fn borrowed_validation_preserves_duplicate_and_unexpected_plot_refusals() {
    let (wake, plan) = fixture();
    let mut duplicate = plan.clone();
    duplicate.plots.push(duplicate.plots[0].clone());
    assert_eq!(
        duplicate.validate_for_borrowed(&wake),
        duplicate.validate_for(&wake)
    );
    assert_eq!(
        duplicate.validate_for(&wake),
        Err(BodyPlanError::DuplicatePlot)
    );
    let mut unexpected = plan;
    unexpected.plots[0].plot = ResidentPlot::new("source/foreign".into(), "checked/foreign".into());
    assert_eq!(
        unexpected.validate_for_borrowed(&wake),
        unexpected.validate_for(&wake)
    );
    assert_eq!(
        unexpected.validate_for(&wake),
        Err(BodyPlanError::UnexpectedPlot)
    );
}
