//! Combined Mask/time policy uses the ordinary private sealing path too.
use super::*;
use crate::{Body, BodyMaskChainPlan, BodyMaskTopology, BodyPlotPlan};
use alloc::vec;
use conduit_core::{BodyTimeTolerance, MonotonicDuration, TemporalScale};
mod fixture {
    use alloc::{vec, vec::Vec};
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../core/tests/common/sealed_state.rs"
    ));
}
#[test]
fn mask_and_body_time_together_preserve_the_ordinary_body_seal() {
    let core = fixture::seal(fixture::fragment());
    let plot = ResidentPlot::new(
        core.source_document_id.clone(),
        core.checked_plot_id.clone(),
    );
    let wake = Body::born_with_plots(BodyWorkset::one(plot.clone()).unwrap(), 1, "born".into())
        .unwrap()
        .wake(2, "wake".into())
        .unwrap()
        .1;
    let requirement = BodyTimeRequirement::new(
        wake.body_id.as_str().into(),
        BodyTimeTolerance::new(2, TemporalScale::Milliseconds),
        MonotonicDuration::new(500, TemporalScale::Milliseconds),
    )
    .unwrap();
    let face = BodyFaceSelector {
        plot: None,
        source_placement_id: None,
    };
    let rich = BodyPlan::seal_with_requirements(
        &wake,
        vec![BodyPlotPlan {
            plot: plot.clone(),
            plan: core.clone(),
        }],
        vec![BodyMaskTopology {
            face: face.clone(),
            chains: vec![BodyMaskChainPlan {
                plan: core.clone(),
                stage_placement_ids: vec!["placement".into()],
            }],
        }],
        Some(requirement.clone()),
    )
    .unwrap();
    let token =
        conduit_core::verify_plan_metadata_with_configurations(core, |_| Ok::<_, ()>(vec![]))
            .unwrap();
    let actual = BodyPlanSeal::capture_verified(
        &wake,
        &[VerifiedBodyPlotPlan { plot, plan: &token }],
        &[VerifiedBodyMaskTopology {
            face,
            chains: vec![VerifiedBodyMaskChain {
                plan: &token,
                stage_placement_ids: vec!["placement".into()],
            }],
        }],
        Some(requirement),
    )
    .unwrap();
    assert_eq!(actual, BodyPlanSeal::capture(&rich).unwrap());
}
