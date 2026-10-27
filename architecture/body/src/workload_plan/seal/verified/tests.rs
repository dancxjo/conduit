//! Combined Mask/time policy uses the ordinary private sealing path too.
use super::*;
use crate::{
    Body, BodyMaskChainPlan, BodyMaskTopology, BodyPlotPlan, ResidentPlot, VerifiedBodyMaskChain,
};
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

#[test]
fn prepared_plot_retains_exact_mask_source_placement_time_identity_and_refusals() {
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
    let verified = conduit_core::verify_plan_metadata_with_configurations(core.clone(), |_| {
        Ok::<_, ()>(vec![])
    })
    .unwrap();
    let input = VerifiedBodyPlotPlan {
        plot: plot.clone(),
        plan: &verified,
    };
    let prepared = PreparedVerifiedBodyPlotPlan::capture([7; 32], &input).unwrap();
    let mask =
        conduit_core::verify_plan_metadata_with_configurations(core, |_| Ok::<_, ()>(vec![]))
            .unwrap();
    let requirement = BodyTimeRequirement::new(
        wake.body_id.as_str().into(),
        BodyTimeTolerance::new(2, TemporalScale::Milliseconds),
        MonotonicDuration::new(500, TemporalScale::Milliseconds),
    )
    .unwrap();
    let mut topology = VerifiedBodyMaskTopology {
        face: BodyFaceSelector {
            plot: Some(plot),
            source_placement_id: Some("placement".into()),
        },
        chains: vec![VerifiedBodyMaskChain {
            plan: &mask,
            stage_placement_ids: vec!["placement".into()],
        }],
    };
    let expected = BodyPlanSeal::capture_verified(
        &wake,
        core::slice::from_ref(&input),
        core::slice::from_ref(&topology),
        Some(requirement.clone()),
    )
    .unwrap();
    assert_eq!(
        BodyPlanSeal::capture_prepared_verified(
            &wake,
            core::slice::from_ref(&prepared),
            core::slice::from_ref(&input),
            core::slice::from_ref(&topology),
            Some(requirement.clone())
        ),
        Err(BodyPlanSealRefusal::Invalid(BodyPlanError::DuplicatePlot))
    );
    drop(input);
    drop(verified);
    let actual = BodyPlanSeal::capture_prepared_verified(
        &wake,
        core::slice::from_ref(&prepared),
        &[],
        core::slice::from_ref(&topology),
        Some(requirement.clone()),
    )
    .unwrap();
    assert_eq!(actual, expected);
    topology.face.source_placement_id = Some("missing".into());
    assert_eq!(
        BodyPlanSeal::capture_prepared_verified(
            &wake,
            core::slice::from_ref(&prepared),
            &[],
            core::slice::from_ref(&topology),
            Some(requirement)
        ),
        Err(BodyPlanSealRefusal::Invalid(
            BodyPlanError::InvalidMaskChain
        ))
    );
    assert_eq!(
        BodyPlanSeal::capture_prepared_verified(&wake, &[], &[], &[], None),
        Err(BodyPlanSealRefusal::Invalid(BodyPlanError::MissingPlot))
    );
    assert_eq!(
        preflight_core_scopes(core::iter::once((MAX_BODY_SEAL_FRAGMENTS + 1, false))),
        Err(BodyPlanSealRefusal::Capacity)
    );
    assert_eq!(
        preflight_core_scopes(core::iter::repeat_n((1, false), MAX_BODY_SEAL_PLANS + 1)),
        Err(BodyPlanSealRefusal::Capacity)
    );
    assert_eq!(
        preflight_core_scopes(core::iter::once((1, true))),
        Err(BodyPlanSealRefusal::NestedExecution)
    );
}
