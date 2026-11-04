//! Exact planning for the installed owner's retained Source and live Host.
use super::{debug, Owner};
use conduit_body::{BodyPlotPlan, ResidentPlot};
use conduit_core::{BaseImplementationId, HostAdvertisement, PortDirection};
use std::collections::BTreeMap;

impl Owner {
    pub(in crate::body_owner) fn plan(
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

    pub(in crate::body_owner) fn plan_with_source(
        &mut self,
        source: &crate::plot_source::CanonicalSource,
        plot: &conduit_plot::ExpandedAuthoringPlot,
    ) -> Result<(), String> {
        let resident = self.resident.as_ref().ok_or("Body has no resident Plot")?;
        let partition =
            self.plan_partition_with_source(source, plot, resident, self.host.advertisement())?;
        let advertised = self.host.advertisement();
        self.session
            .propose(vec![partition], &advertised.host_id, &advertised.boot_id)
            .map_err(debug)?;
        Ok(())
    }

    pub(super) fn plan_partition(
        &self,
        plot: &conduit_plot::ExpandedAuthoringPlot,
        resident: &ResidentPlot,
    ) -> Result<BodyPlotPlan, String> {
        if let Some(partition) = self.shipped_creche_partition(plot, resident)? {
            return Ok(partition);
        }
        let hosts = [self.host.advertisement().clone()];
        // Ordinary planning does not attach child Plans for scan/fold/make.
        // Refuse before sealing a deceptively runnable parent-only Body Plan.
        if !plot.expanded.activations.is_empty() {
            return Err("installed Body execution has no activation-aware Plan or Play".into());
        }
        let placements =
            conduit_planner::default_expanded_placements(&plot.expanded, &hosts).map_err(debug)?;
        let plan = conduit_planner::plan_expanded_authoring_with_options(
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
            &BTreeMap::new(),
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
