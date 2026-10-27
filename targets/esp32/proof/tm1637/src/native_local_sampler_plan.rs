//! Ordinary sampler planning, shared unchanged with the CPU Root proof.
use super::memory_check::mark;
use super::*;
use alloc::collections::BTreeMap;
impl NativeRoot {
    #[inline(never)]
    pub fn prepare_local_sampler_plan(&mut self) -> Result<c::VerifiedPlanMetadata, &'static str> {
        mark("initialized-root-body-radio", 0);
        let image = crate::native_software::planning_source()?;
        let mut host = self
            .sampler_native_advertisement()
            .map_err(|_| "native-offers")?;
        mark("native-offers", host.capabilities.len());
        {
            let software = self.software.as_ref().ok_or("software-owner")?;
            let offers = crate::native_software::offers(software).map_err(|_| "software-offers")?;
            host.capabilities.reserve_exact(offers.iter().len());
            for index in 0..offers.iter().len() {
                mark("before-software-offer", index);
                let offer = offers
                    .decode_at(index)
                    .map_err(|_| "software-offer-codec")?;
                host.capabilities.push(offer);
                mark("after-software-offer", index);
            }
        }
        // The separate measurement profile probes one Gear at a time to locate
        // decoder allocation peaks. The appliance proceeds directly to ordinary
        // indexed selection, which decodes/checks these same complete Gears;
        // a second diagnostic pass contributes no admission or retained owner.
        #[cfg(not(feature = "local-appliance"))]
        for index in 0..image.gear_count() {
            mark("before-source-gear-probe", index);
            let gear = image.gear(index).map_err(|_| "source-gear-codec")?;
            mark("source-gear-live", index);
            drop(gear);
            mark("after-source-gear-probe", index);
        }
        mark("before-indexed-selection", 0);
        let choices = conduit_planner::indexed_source::default_placements(
            &image,
            core::slice::from_ref(&host),
        )
        .map_err(|_| "selection")?;
        mark("after-indexed-selection", choices.by_gear.len());
        let metadata = image.planning_metadata().map_err(|_| "source-metadata")?;
        let mut queues = BTreeMap::new();
        for cord in metadata.connections() {
            let mut capacity = 1024;
            for gear in [&cord.source_gear_id, &cord.sink_gear_id] {
                let choice = choices.by_gear.get(gear).ok_or("choice")?;
                let offer = host
                    .capabilities
                    .iter()
                    .find(|offer| offer.capability_id == choice.capability_id)
                    .ok_or("offer")?;
                if offer.kind_id.as_str()
                    == conduit_environment_runtime::publication::PublicationChannel::Reading
                        .kind_id()
                {
                    capacity = conduit_environment_runtime::publication::PublicationChannel::Reading
                        .maximum_bytes() as u32;
                }
            }
            for gear in [&cord.source_gear_id, &cord.sink_gear_id] {
                let choice = &choices.by_gear[gear];
                let offer = host
                    .capabilities
                    .iter()
                    .find(|offer| offer.capability_id == choice.capability_id)
                    .ok_or("offer")?;
                capacity = capacity.min(offer.limits.max_queue_bytes);
            }
            // Omitted entries use the exact same one-item/1024-byte defaults.
            // Keep only real overrides, rather than clone every Cord identity.
            if capacity != 1024 {
                queues.insert(
                    (
                        cord.source_gear_id.clone(),
                        cord.source_port_id.clone(),
                        cord.sink_gear_id.clone(),
                        cord.sink_port_id.clone(),
                    ),
                    conduit_planner::ConnectionQueueLimits {
                        item_capacity: 1,
                        byte_capacity: capacity,
                    },
                );
            }
        }
        drop(metadata);
        mark("connection-budget-overrides", queues.len());
        let grants = [
            self.issuer
                .grant_pulse(self.pulse.as_ref().ok_or("pulse")?.native_identity())
                .map_err(|_| "pulse-grant")?,
            self.issuer
                .grant_wait(self.wait.as_ref().ok_or("wait")?.native_identity())
                .map_err(|_| "wait-grant")?,
        ];
        let empty = BTreeMap::new();
        let lines = BTreeMap::new();
        mark("before-compact-planning", 0);
        let compact =
            conduit_planner::indexed_source::plan_compact_consuming_with_connection_limits(
                &image,
                alloc::vec![host],
                &choices,
                &["conduit.base/local@1".into()],
                conduit_planner::PlanningOptions {
                    connection_bases: &empty,
                    line_candidates: &lines,
                    connection_item_capacity: 1,
                    connection_byte_capacity: 1024,
                    authority_grants: &grants,
                    protected_resource_grants: &[],
                    line_offers: &[],
                },
                &queues,
            )
            .map_err(|_| "planning")?;
        mark("compact-plan-live", compact.fragment_count());
        // Ordinary admission and fingerprinting own all selected metadata.
        // Their input ledgers are not needed by the final opaque verification.
        drop((choices, queues, grants, empty, lines));
        mark("planning-scratch-retired", 0);
        let plan = compact
            .into_verified_metadata()
            .map_err(|_| "verification")?;
        mark("verified-plan-live", plan.fragment_count());
        Ok(plan)
    }
}
