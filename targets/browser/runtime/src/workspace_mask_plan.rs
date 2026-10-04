use super::*;
pub(crate) use conduit_browser_mask_offer::{planned_mask, ALTERNATE_MASK_SOURCE, MASK_SOURCE};

pub(super) fn admitted_routes(
    body_plan: &conduit_body::BodyPlan,
    mask: &PlannedMaskPlot,
    alternate: &PlannedMaskPlot,
    initial_available: bool,
    alternate_available: bool,
) -> Result<AdmittedMaskPlotRoutes, String> {
    let initial_placements = mask
        .plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .map(|placement| placement.placement_id.clone())
        .collect::<Vec<_>>();
    let alternate_placements = alternate
        .plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .map(|p| p.placement_id.clone())
        .collect();
    AdmittedMaskPlotRoutes::new(
        body_plan,
        &[mask.clone(), alternate.clone()],
        vec![
            SealedMaskPlotRoute {
                route_id: "route/browser-graphical".into(),
                mask_plot: mask.mask.plot_identity.clone(),
                plan_id: body_plan.plan_id.clone(),
                child_mask_plan_id: None,
                owner_route_seal_id: None,
                placement_ids: initial_placements,
                currently_available: initial_available,
            },
            SealedMaskPlotRoute {
                route_id: "route/browser-graphical-fallback".into(),
                mask_plot: alternate.mask.plot_identity.clone(),
                plan_id: body_plan.plan_id.clone(),
                child_mask_plan_id: None,
                owner_route_seal_id: None,
                placement_ids: alternate_placements,
                currently_available: alternate_available,
            },
        ],
    )
    .map_err(|error| format!("admit browser Mask routes: {error:?}"))
}

#[cfg(test)]
mod installed_offer_tests {
    use super::*;
    use conduit_core::{BootId, HostId};

    #[test]
    fn dom_mask_plan_requires_the_current_advertised_back_and_presentation_resource() {
        let host = crate::installed_browser::membership_advertisement(
            HostId::from("host/browser-mask"),
            BootId::from("boot/browser-mask"),
        );
        let planned = planned_mask(&host, MASK_SOURCE, "browser-graphical").unwrap();
        let placement = &planned.plan.fragments[0].placements[0];
        assert_eq!(placement.host_id, host.host_id);
        assert_eq!(placement.boot_id, host.boot_id);
        assert_eq!(placement.offer_generation, host.offer_generation);
        assert_eq!(
            placement.capability_id,
            crate::installed_browser::dom_mask::offer().capability_id
        );
        assert!(placement
            .resources
            .iter()
            .any(|resource| resource.pool_id.as_str() == "browser/presentation"));

        let mut no_back = host.clone();
        no_back.capabilities.retain(|offer| {
            offer.capability_id != crate::installed_browser::dom_mask::offer().capability_id
        });
        assert!(planned_mask(&no_back, MASK_SOURCE, "browser-graphical").is_err());

        let mut no_resource = host;
        no_resource
            .resources
            .retain(|resource| resource.pool_id.as_str() != "browser/presentation");
        assert!(planned_mask(&no_resource, MASK_SOURCE, "browser-graphical").is_err());
    }
}
