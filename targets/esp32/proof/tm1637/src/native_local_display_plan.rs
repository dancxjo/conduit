//! Prepare the second required Body plot from actual retained output owners.
//! No display packet, possession, Body start or Fore ingress is released here.
use super::*;
use alloc::collections::BTreeMap;
impl NativeRoot {
    pub fn prepare_local_display_plan(&mut self) -> Result<c::VerifiedPlanMetadata, &'static str> {
        let image = crate::native_display::planning_source()?;
        let boundary = crate::native_display::planning_boundary()?;
        let mut host = self.advertisement().map_err(|_| "display-native-offers")?;
        // Project this task's native offer from the actual retained display
        // owner. Other initialized Root providers remain owned and available.
        host.capabilities = alloc::vec::Vec::new();
        host.resources = alloc::vec::Vec::new();
        host.bases = alloc::vec::Vec::new();
        self.display
            .append_to_advertisement(&mut host)
            .map_err(|_| "display-native-offer")?;
        super::memory_check::mark("display-native-offers", host.capabilities.len());
        {
            let offers = self
                .display
                .current_software_offers(&self.host, &self.boot, host.offer_generation)
                .map_err(|_| "display-software-unavailable")?;
            for offer in offers.decoded() {
                host.capabilities
                    .push(offer.map_err(|_| "display-offer-codec")?);
            }
        }
        super::memory_check::mark("display-current-offers", host.capabilities.len());
        let choices = conduit_planner::indexed_source::default_placements(
            &image,
            core::slice::from_ref(&host),
        )
        .map_err(|_| "display-selection")?;
        super::memory_check::mark("display-selection-live", choices.by_gear.len());
        let metadata = image
            .planning_metadata()
            .map_err(|_| "display-source-metadata")?;
        let queues = metadata
            .connections()
            .iter()
            .map(|cord| {
                (
                    (
                        cord.source_gear_id.clone(),
                        cord.source_port_id.clone(),
                        cord.sink_gear_id.clone(),
                        cord.sink_port_id.clone(),
                    ),
                    conduit_planner::ConnectionQueueLimits {
                        item_capacity: 1,
                        byte_capacity: conduit_tm1637_wire::MAXIMUM_BYTES,
                    },
                )
            })
            .collect();
        drop(metadata);
        let mut fore = BTreeMap::new();
        for (direction, port, track) in boundary.bindings() {
            fore.insert(
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: port.clone(),
                    track,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: if direction == c::PortDirection::Input {
                        conduit_tm1637_wire::MAXIMUM_BYTES
                    } else {
                        64
                    },
                },
            );
        }
        let native = self
            .display
            .native_identity()
            .ok_or("display-native-unavailable")?;
        let bases = [
            "conduit.base/local@1".into(),
            native.base_implementation_id.clone(),
        ];
        let grants = [self
            .issuer
            .grant_display(native)
            .map_err(|_| "display-grant")?];
        let empty = BTreeMap::new();
        let lines = BTreeMap::new();
        super::memory_check::mark("before-display-compact-planning", 0);
        let compact =
            conduit_planner::indexed_source::plan_compact_authoring_consuming_with_connection_limits(
                &image,
                &boundary,
                alloc::vec![host],
                &choices,
                &bases,
                conduit_planner::PlanningOptions {
                    connection_bases: &empty,
                    line_candidates: &lines,
                    connection_item_capacity: 1,
                    connection_byte_capacity: conduit_tm1637_wire::MAXIMUM_BYTES,
                    authority_grants: &grants,
                    protected_resource_grants: &[],
                    line_offers: &[],
                },
                &queues,
                &fore,
            )
            .map_err(|_| "display-planning")?;
        drop((choices, queues, fore, bases, grants, empty, lines, boundary));
        compact
            .into_verified_metadata()
            .map_err(|_| "display-plan-verification")
    }
}
