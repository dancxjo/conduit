//! Exact external Fore admission for the shipped ordinary Crèche Plot.
use super::{debug, Owner};
use conduit_body::{BodyPlotPlan, ResidentPlot};
use conduit_core::{ConnectionTrack, PortDirection};
use std::collections::BTreeMap;

impl Owner {
    pub(super) fn shipped_creche_partition(
        &self,
        plot: &conduit_plot::ExpandedAuthoringPlot,
        resident: &ResidentPlot,
    ) -> Result<Option<BodyPlotPlan>, String> {
        if plot.expanded.name != "creche/main" {
            return Ok(None);
        }
        // A matching display name is not authority to apply this Fore policy.
        let shipped =
            crate::plot_source::parse(include_str!("../../../../plots/creche/main.conduit"))?
                .expand_entry_for_authoring()?;
        let exact = ResidentPlot::new(
            shipped.expanded.source_document_id.clone(),
            shipped.expanded.checked_plot_id.clone(),
        );
        if resident != &exact || plot.expanded != shipped.expanded {
            return Err("Crèche requires the exact shipped checked Source".into());
        }
        use conduit_body_make::deployment_runtime::{CRECHE_OUTCOME_BYTES, CRECHE_REQUEST_BYTES};
        let boundaries = [
            (PortDirection::Input, "request", CRECHE_REQUEST_BYTES),
            (PortDirection::Output, "outcome", CRECHE_OUTCOME_BYTES),
        ]
        .into_iter()
        .map(|(direction, name, bytes)| {
            (
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: conduit_core::port_id(name),
                    track: ConnectionTrack::Payload,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: bytes as u32,
                },
            )
        })
        .collect();
        let hosts = [self.host.advertisement().clone()];
        let placements =
            conduit_planner::default_expanded_placements(&plot.expanded, &hosts).map_err(debug)?;
        let plan = conduit_planner::plan_expanded_authoring_with_options(
            plot,
            &hosts,
            &placements,
            &["conduit.base/local@1".into()],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: CRECHE_OUTCOME_BYTES as u32,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &boundaries,
        )
        .map_err(debug)?;
        Ok(Some(BodyPlotPlan { plot: exact, plan }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_std_host::{StdHost, StdHostComposition, StdHostConfig};

    fn source() -> conduit_plot::ExpandedAuthoringPlot {
        crate::plot_source::parse(include_str!("../../../../plots/creche/main.conduit"))
            .unwrap()
            .expand_entry_for_authoring()
            .unwrap()
    }

    fn owner(plot: &conduit_plot::ExpandedAuthoringPlot, installed: bool) -> Owner {
        let config = StdHostConfig {
            host_id: "test/creche-planning".into(),
            boot_id: "test/creche-planning-boot".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        };
        let host = if installed {
            StdHost::new_with_creche_encounter(
                config,
                StdHostComposition::default(),
                conduit_core::resource_offer(
                    "test/creche-slot",
                    conduit_body_make::deployment_runtime_catalog::CRECHE_RESOURCE,
                    1,
                ),
            )
            .unwrap()
        } else {
            StdHost::new_with_config(config)
        };
        Owner::open(
            host,
            ResidentPlot::new(
                plot.expanded.source_document_id.clone(),
                plot.expanded.checked_plot_id.clone(),
            ),
            None,
            "Authoring",
        )
        .unwrap()
    }

    #[test]
    fn creche_planning_requires_explicit_installed_offer() {
        let plot = source();
        assert!(owner(&plot, false).plan(&plot).is_err());
        owner(&plot, true).plan(&plot).unwrap();
    }

    #[test]
    fn creche_planning_rejects_same_name_with_different_checked_source() {
        let mut plot = source();
        plot.expanded.source_document_id = "test/not-the-shipped-source".into();
        let owner = owner(&plot, true);
        let resident = owner.resident.as_ref().unwrap();
        let error = owner.shipped_creche_partition(&plot, resident).unwrap_err();
        assert!(error.contains("exact shipped checked Source"));
    }
}
