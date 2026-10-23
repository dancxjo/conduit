//! Prepare the single Patchbay actor at its exact selected owner placement.
use super::super::prepared_patchbay::PreparedPatchbay;
use conduit_body::BodyPlan;
use conduit_core::{ActivePlayId, PlannedGear};

pub(super) fn prepare(
    placement: &PlannedGear,
    plan: &BodyPlan,
    active_play_id: &ActivePlayId,
    source: &str,
    foreground_checked_plot_id: &str,
) -> Result<PreparedPatchbay, String> {
    #[cfg(test)]
    if placement.kind_id.as_str() != patchbay_application::PATCHBAY_INTERACTION_KIND {
        return prepare_legacy_fixture(
            placement,
            plan,
            active_play_id,
            source,
            foreground_checked_plot_id,
        );
    }
    prepare_canonical(
        placement,
        plan,
        active_play_id,
        source,
        foreground_checked_plot_id,
    )
}

/// Product admission has one exact typed Fore; the old view adapter is a fixture.
pub(super) fn prepare_canonical(
    placement: &PlannedGear,
    plan: &BodyPlan,
    active_play_id: &ActivePlayId,
    source: &str,
    foreground_checked_plot_id: &str,
) -> Result<PreparedPatchbay, String> {
    if placement.kind_id.as_str() != patchbay_application::PATCHBAY_INTERACTION_KIND {
        return Err("patchbay-legacy-application-fore-retired".into());
    }
    if !plan.plots.iter().any(|part| {
        part.plan.fragments.iter().any(|fragment| {
            fragment
                .placements
                .iter()
                .any(|gear| gear.placement_id == placement.placement_id)
        })
    }) {
        return Err("Patchbay owner is absent from its Body Plan".into());
    }
    let port = prepare_port(plan, active_play_id, source, foreground_checked_plot_id)?
        .with_canvas_delivery(patchbay_application::PatchbayCanvasDelivery::TextInspection);
    Ok(PreparedPatchbay::new(port))
}

fn prepare_port(
    plan: &BodyPlan,
    active_play_id: &ActivePlayId,
    source: &str,
    foreground_checked_plot_id: &str,
) -> Result<patchbay_application::PatchbayApplicationPort, String> {
    let residents: Vec<_> = plan.plots.iter().map(|part| part.plot.clone()).collect();
    let subjects = crate::creche::inventory_application_subjects(source, &residents)?;
    let mut active = Vec::with_capacity(plan.plots.len());
    for (part, (expanded, title)) in plan.plots.iter().zip(subjects) {
        active.push(
            patchbay_application::PatchbayActivePlot::project(
                &expanded,
                title,
                part.plot.checked_plot_id.as_str(),
                part.plan.plan_id.clone(),
                patchbay_application::PatchbayActivePlotState::Playing,
                part.plot.checked_plot_id.as_str() == foreground_checked_plot_id,
            )
            .map_err(|error| format!("project resident Patchbay Plot: {error:?}"))?,
        );
    }
    patchbay_application::PatchbayApplicationPort::open_active(
        active,
        plan.plan_id.clone(),
        Some(active_play_id.clone()),
    )
    .map_err(|error| format!("prepare resident Patchbay: {error:?}"))
}

/// Counterfactual old Source is retained only for named CPU migration fixtures.
#[cfg(test)]
fn prepare_legacy_fixture(
    placement: &PlannedGear,
    plan: &BodyPlan,
    active_play_id: &ActivePlayId,
    source: &str,
    foreground_checked_plot_id: &str,
) -> Result<PreparedPatchbay, String> {
    let port = prepare_port(plan, active_play_id, source, foreground_checked_plot_id)?;
    let resident = plan
        .plots
        .iter()
        .find(|part| {
            part.plan.fragments.iter().any(|fragment| {
                fragment
                    .placements
                    .iter()
                    .any(|gear| gear.placement_id == placement.placement_id)
            })
        })
        .ok_or("Patchbay owner is absent from its Body Plan")?;
    let canonical = plan.mask_topologies.iter().any(|topology| {
        topology.face.plot.as_ref() == Some(&resident.plot)
            && topology.face.source_placement_id.is_none()
    });
    let port = if canonical {
        port.with_canvas_delivery(patchbay_application::PatchbayCanvasDelivery::TextInspection)
    } else {
        port
    };
    let mut prepared = PreparedPatchbay::legacy_fixture(port);
    if canonical {
        prepared.require_canonical_input();
    }
    Ok(prepared)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plot_runner::body_start::tests::request_from_sources;
    const CANONICAL: &str = include_str!("../../../../../plots/patchbay/main.conduit");
    const LEGACY: &str =
        include_str!("../../../../../plots/patchbay/fixtures/legacy-retained.conduit");

    #[test]
    fn product_preparation_refuses_retired_fore_before_actor_preparation() {
        let request = request_from_sources(&[LEGACY]);
        let part = &request.plan.plots[0];
        let placement = part
            .plan
            .fragments
            .iter()
            .flat_map(|f| &f.placements)
            .find(|p| p.kind_id.as_str() == conduit_semantic_catalog::RETAINED_APPLICATION_KIND)
            .unwrap();
        let play = conduit_body::BodyPlayIdentity::bind(&request.plan, request.play_sequence);
        // An invalid Source would fail inventory preparation if the retired
        // route were entered; its exact retirement is decided first.
        assert!(
            matches!(prepare_canonical(placement, &request.plan, &play.active_play_id, "not a Plot", part.plot.checked_plot_id.as_str()), Err(error) if error == "patchbay-legacy-application-fore-retired")
        );
    }
    #[test]
    fn canonical_product_has_no_empty_poll_or_legacy_event_view_output() {
        let request = request_from_sources(&[CANONICAL]);
        let part = &request.plan.plots[0];
        let placement = &part.plan.fragments[0].placements[0];
        let play = conduit_body::BodyPlayIdentity::bind(&request.plan, request.play_sequence);
        let mut prepared = prepare_canonical(
            placement,
            &request.plan,
            &play.active_play_id,
            CANONICAL,
            part.plot.checked_plot_id.as_str(),
        )
        .unwrap();
        let view =
            conduit_face::ApplicationView::decode(&prepared.port.apply(&[]).unwrap().view).unwrap();
        assert!(!view
            .nodes
            .iter()
            .any(|node| node.component == conduit_face::ApplicationComponent::PatchbayCanvas));
        let event = conduit_face::ApplicationEvent {
            revision: view.revision,
            action: "patchbay.plot.0".into(),
            kind: conduit_face::ApplicationEventKind::Activate,
            value: vec![],
        }
        .encode(&view)
        .unwrap();
        let before = prepared.port.revision();
        for input in [&[][..], &[0][..], &[255][..], event.as_slice()] {
            assert_eq!(
                prepared.execute(input).unwrap_err(),
                "patchbay-canonical-interaction-required"
            );
            assert_eq!(prepared.port.revision(), before);
            assert!(prepared.pending_observation().is_none());
        }
    }
    #[test]
    fn canonical_product_retains_exact_body_placement_custody() {
        let request = request_from_sources(&[CANONICAL]);
        let part = &request.plan.plots[0];
        let mut placement = part.plan.fragments[0].placements[0].clone();
        placement.placement_id = "placement/not-in-body".into();
        let play = conduit_body::BodyPlayIdentity::bind(&request.plan, request.play_sequence);
        assert!(
            matches!(prepare_canonical(&placement, &request.plan, &play.active_play_id, CANONICAL, part.plot.checked_plot_id.as_str()), Err(error) if error == "Patchbay owner is absent from its Body Plan")
        );
    }
}
