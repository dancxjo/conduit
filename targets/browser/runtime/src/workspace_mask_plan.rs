use super::*;
use conduit_core::HostAdvertisement;

pub(crate) const MASK_SOURCE: &str = "plot browser-graphical (\n >> face: Presentation\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n mask: presentation/browser-dom-mask\n face >> mask.presentation\n mask.interaction >> interaction\n mask.show >> show\n}\n";
pub(super) const ALTERNATE_MASK_SOURCE: &str = "plot browser-graphical-alternate (\n >> face: Presentation\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n mask: presentation/browser-dom-mask\n face >> mask.presentation\n mask.interaction >> interaction\n mask.show >> show\n}\n";

pub(crate) fn planned_mask(
    host: &HostAdvertisement,
    source: &str,
    name: &str,
) -> Result<PlannedMaskPlot, String> {
    let definition = crate::installed_browser::dom_mask::kind();
    let mut startup = StartupCatalog::new();
    install_mask_plot_value_aliases(&mut startup).map_err(|error| format!("{error:?}"))?;
    startup
        .insert(KindSignature {
            kind: definition.kind_id.as_str().into(),
            startup_parameters: vec![],
        })
        .map_err(|error| format!("{error:?}"))?;
    let mut profiles = ProfileCatalog::new();
    profiles
        .insert_kind(definition.clone())
        .map_err(|error| format!("{error:?}"))?;
    let (mask, authoring) = mask_plot(source, name, &startup, &profiles)?;
    let syntax = parse_syntax_document(source);
    if !syntax.diagnostics.is_empty() {
        return Err(format!("{:?}", syntax.diagnostics));
    }
    let placements = conduit_planner::default_expanded_placements(
        &authoring.expanded,
        core::slice::from_ref(host),
    )
    .map_err(|error| format!("{error:?}"))?;
    let mut boundary_limits = BTreeMap::new();
    for (direction, bindings) in [
        (PortDirection::Input, authoring.input_bindings.as_slice()),
        (PortDirection::Output, authoring.output_bindings.as_slice()),
    ] {
        for binding in bindings {
            boundary_limits.insert(
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 4,
                    byte_capacity: MASK_BYTES,
                },
            );
        }
    }
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authoring,
        core::slice::from_ref(host),
        &placements,
        &[],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 4,
            connection_byte_capacity: MASK_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundary_limits,
    )
    .map_err(|error| format!("{error:?}"))?;
    let planned = PlannedMaskPlot::admit(&mask, &plan).map_err(|error| format!("{error:?}"))?;
    Ok(planned)
}

fn mask_plot(
    source: &str,
    name: &str,
    startup: &StartupCatalog,
    profiles: &ProfileCatalog,
) -> Result<(MaskPlot, conduit_plot::ExpandedAuthoringPlot), String> {
    let syntax = parse_syntax_document(source);
    if !syntax.diagnostics.is_empty() {
        return Err(format!("{:?}", syntax.diagnostics));
    }
    let checked = check_syntax_document(&syntax, startup).map_err(|error| format!("{error:?}"))?;
    let authoring = expand_canonical_plot_for_authoring(&checked, name, profiles)
        .map_err(|error| format!("{error:?}"))?;
    let mask = MaskPlot::admit(&authoring).map_err(|error| format!("{error:?}"))?;
    Ok((mask, authoring))
}

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
                placement_ids: initial_placements,
                currently_available: initial_available,
            },
            SealedMaskPlotRoute {
                route_id: "route/browser-graphical-fallback".into(),
                mask_plot: alternate.mask.plot_identity.clone(),
                plan_id: body_plan.plan_id.clone(),
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
