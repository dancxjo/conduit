//! Exact planning for retained Source and observed current member equipment.
use super::{debug, Owner};
use conduit_body::{BodyPlotPlan, BodyState, BodyWorkset, ResidentPlot};
use conduit_core::{
    port_id, BaseImplementationId, ConnectionTrack, HostAdvertisement, PortDirection,
};
use std::collections::BTreeMap;

impl Owner {
    pub(in super::super) fn plan(
        &mut self,
        plot: &conduit_plot::ExpandedAuthoringPlot,
    ) -> Result<(), String> {
        let resident = self.resident.as_ref().ok_or("Body has no resident Plot")?;
        let partition = self.plan_partition(plot, resident)?;
        let hosts = [self.host.advertisement().clone()];
        self.session
            .propose(vec![partition], &hosts[0].host_id, &hosts[0].boot_id)
            .map_err(debug)?;
        Ok(())
    }

    pub(in super::super) fn plan_with_source(
        &mut self,
        source: &crate::plot_source::CanonicalSource,
        plot: &conduit_plot::ExpandedAuthoringPlot,
    ) -> Result<(), String> {
        self.plan_workset_with_sources(&[(source, plot)])
    }

    /// Prepare the complete already-retained workset, never adopt a proposal.
    /// Source and checking context are owned locally; expanded inspection values
    /// cannot replace an independent check. All partitions prepare before a Wake
    /// is published.
    pub(super) fn plan_workset_with_sources(
        &mut self,
        sources: &[(
            &crate::plot_source::CanonicalSource,
            &conduit_plot::ExpandedAuthoringPlot,
        )],
    ) -> Result<(), String> {
        self.plan_workset_with_source_boundaries(sources, &BTreeMap::new())
    }

    /// Boundary budgets belong to the selected external adapter. They cannot
    /// adopt Source, add connectivity, or enlarge the selected Back's limits.
    pub(super) fn plan_workset_with_source_boundaries(
        &mut self,
        sources: &[(
            &crate::plot_source::CanonicalSource,
            &conduit_plot::ExpandedAuthoringPlot,
        )],
        boundaries: &BTreeMap<
            ResidentPlot,
            BTreeMap<conduit_planner::ForeBoundaryKey, conduit_planner::ConnectionQueueLimits>,
        >,
    ) -> Result<(), String> {
        if self.session.evidence().body.state != BodyState::Lulled
            || self.session.realization().is_some()
        {
            return Err("workset planning requires an idle Lulled owner".into());
        }
        if sources.len() > conduit_body::MAX_BODY_PLOTS {
            return Err("workset Source count exceeds Body capacity".into());
        }
        let supplied = BodyWorkset::from_plots(sources.iter().map(|(_, plot)| {
            ResidentPlot::new(
                plot.expanded.source_document_id.clone(),
                plot.expanded.checked_plot_id.clone(),
            )
        }))
        .map_err(debug)?;
        if supplied != self.session.evidence().body.workset {
            return Err("checked Sources must cover the exact retained Body workset".into());
        }
        if boundaries.keys().any(|plot| !supplied.contains(plot)) {
            return Err("Fore budget names a Plot outside the retained workset".into());
        }
        let mut partitions = Vec::with_capacity(sources.len());
        for (source, expected) in sources {
            let checked = source.expand_entry_for_authoring()?;
            if &checked != *expected {
                return Err("expanded inspection differs from independently checked Source".into());
            }
            let resident = ResidentPlot::new(
                checked.expanded.source_document_id.clone(),
                checked.expanded.checked_plot_id.clone(),
            );
            let partition = match boundaries.get(&resident) {
                Some(boundary) => {
                    self.plan_partition_with_boundaries(&checked, &resident, boundary)?
                }
                None => self.plan_partition_with_source(
                    source,
                    &checked,
                    &resident,
                    self.host.advertisement(),
                )?,
            };
            partitions.push(partition);
        }
        let advertised = self.host.advertisement();
        self.session
            .propose(partitions, &advertised.host_id, &advertised.boot_id)
            .map_err(debug)?;
        Ok(())
    }

    pub(super) fn plan_partition(
        &self,
        plot: &conduit_plot::ExpandedAuthoringPlot,
        resident: &ResidentPlot,
    ) -> Result<BodyPlotPlan, String> {
        self.plan_partition_with_boundaries(plot, resident, &BTreeMap::new())
    }

    fn plan_partition_with_boundaries(
        &self,
        plot: &conduit_plot::ExpandedAuthoringPlot,
        resident: &ResidentPlot,
        boundary: &BTreeMap<
            conduit_planner::ForeBoundaryKey,
            conduit_planner::ConnectionQueueLimits,
        >,
    ) -> Result<BodyPlotPlan, String> {
        if let Some(partition) = self.shipped_creche_partition(plot, resident)? {
            if !boundary.is_empty() {
                return Err("Crèche boundary budgets belong to its installed adapter".into());
            }
            return Ok(partition);
        }
        let hosts = self.planning_hosts();
        // Ordinary planning does not attach child Plans for scan/fold/make.
        // Refuse before sealing a deceptively runnable parent-only Body Plan.
        if !plot.expanded.activations.is_empty() {
            return Err("installed Body execution has no activation-aware Plan or Play".into());
        }
        let placements =
            conduit_planner::default_expanded_placements(&plot.expanded, &hosts).map_err(debug)?;
        let limits = conduit_planner::default_expanded_connection_limits(
            &plot.expanded,
            &hosts,
            &placements,
        )
        .map_err(debug)?;
        let plan = conduit_planner::plan_expanded_authoring_with_connection_limits(
            plot,
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: conduit_core::DEFAULT_CONNECTION_ITEM_CAPACITY,
                connection_byte_capacity: conduit_core::DEFAULT_CONNECTION_BYTE_CAPACITY,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &limits,
            boundary,
        )
        .map_err(debug)?;
        Ok(BodyPlotPlan {
            plot: resident.clone(),
            plan,
        })
    }

    /// Plan the exact retained source when its Host actually advertises an
    /// activation coordinator. The selected scan worker supplies its bounded
    /// ingress only after this exact parent and child Plan are sealed.
    pub(super) fn plan_partition_with_source(
        &self,
        source: &crate::plot_source::CanonicalSource,
        plot: &conduit_plot::ExpandedAuthoringPlot,
        resident: &ResidentPlot,
        advertisement: &HostAdvertisement,
    ) -> Result<BodyPlotPlan, String> {
        if plot.expanded.activations.is_empty() {
            return self.plan_partition(plot, resident);
        }
        let (state_bytes, command_bytes) = match plot.expanded.name.as_str() {
            "todo/main" => {
                super::super::scoped_todo_initial(plot)?.ok_or("Todo scan is missing")?;
                (
                    conduit_todo_plot::STATE_MAX_BYTES,
                    conduit_todo_plot::COMMAND_MAX_BYTES,
                )
            }
            "thermostat/main" => {
                super::super::scoped_thermostat_initial(plot)?
                    .ok_or("Thermostat scan is missing")?;
                (
                    conduit_thermostat_plot::STATE_BYTES,
                    conduit_thermostat_plot::COMMAND_BYTES,
                )
            }
            _ => return Err("installed Body supports no other activation source".into()),
        };
        let expected = ResidentPlot::new(
            plot.expanded.source_document_id.clone(),
            plot.expanded.checked_plot_id.clone(),
        );
        if &expected != resident || self.resident.as_ref() != Some(resident) {
            return Err("checked source differs from the Body's resident Plot".into());
        }
        let document = source.check()?;
        let hosts = [advertisement.clone()];
        let placements =
            conduit_planner::default_expanded_placements(&plot.expanded, &hosts).map_err(debug)?;
        let queue_bytes = (2 * state_bytes + 2 * command_bytes) as u32;
        let boundaries = BTreeMap::from([
            (
                conduit_planner::ForeBoundaryKey {
                    direction: PortDirection::Input,
                    front_port_id: port_id("commands"),
                    track: ConnectionTrack::Payload,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: command_bytes as u32,
                },
            ),
            (
                conduit_planner::ForeBoundaryKey {
                    direction: PortDirection::Output,
                    front_port_id: port_id("states"),
                    track: ConnectionTrack::Payload,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: state_bytes as u32,
                },
            ),
        ]);
        let plan = conduit_planner::plan_expanded_authoring_with_activations(
            &document,
            plot,
            source.authoring_catalog(),
            &conduit_plot::CanonicalBackCatalog::new(),
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: queue_bytes,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &boundaries,
        )
        .map_err(debug)?;
        Ok(BodyPlotPlan {
            plot: resident.clone(),
            plan,
        })
    }
}

#[cfg(test)]
#[path = "planning_tests.rs"]
mod tests;
