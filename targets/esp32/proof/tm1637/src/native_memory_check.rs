//! Delivery checkpoint diagnostic, never a claim of a started appliance.
use super::*;
use alloc::collections::BTreeMap;
#[inline(never)]
pub(super) fn mark(phase: &str, item: usize) {
    let root = esp_alloc::HEAP.stats();
    let sdk = crate::sdk_memory::stats();
    let bank_free = core::array::from_fn::<_, 3, _>(|i| {
        root.region_stats[i]
            .as_ref()
            .map_or(0, |region| region.free)
    });
    esp_println::println!(
        "{{\"event\":\"target-memory-check\",\"phase\":\"{}\",\"item\":{},\"root_size\":{},\"root_used\":{},\"root_peak\":{},\"root_free\":{},\"sdk_size\":{},\"sdk_used\":{},\"sdk_peak\":{},\"root_bank_free\":[{},{},{}]}}",
        phase, item, root.size, root.current_usage, root.max_usage,
        root.size.saturating_sub(root.current_usage), sdk.size, sdk.current_usage, sdk.max_usage, bank_free[0], bank_free[1], bank_free[2],
    );
}
impl NativeRoot {
    #[inline(never)]
    pub fn measure_local_sampler(&mut self) -> Result<(), &'static str> {
        mark("initialized-root-body-radio", 0);
        let image = crate::native_software::planning_source()?;
        let mut host = self.advertisement().map_err(|_| "native-offers")?;
        mark("native-offers", host.capabilities.len());
        {
            let software = self.software.as_ref().ok_or("software-owner")?;
            let offers = crate::native_software::offers(software).map_err(|_| "software-offers")?;
            for (index, bytes) in offers.iter().enumerate() {
                mark("before-software-offer", index);
                let offer = serde_json::from_slice(bytes).map_err(|_| "software-offer-codec")?;
                host.capabilities.push(offer);
                mark("after-software-offer", index);
            }
        }
        // Decode one exact Source Gear at a time under the retained production
        // baseline. This is the same decoder ordinary indexed selection uses.
        // A failure here is a lower bound: selection adds candidate state too.
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
        let metadata = image.metadata().map_err(|_| "source-metadata")?;
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
        drop(metadata);
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
        let compact = conduit_planner::indexed_source::plan_compact_with_connection_limits(
            &image,
            core::slice::from_ref(&host),
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
        let plan = compact
            .into_verified_metadata()
            .map_err(|_| "verification")?;
        mark("verified-plan-live", plan.fragment_count());
        // Opaque verification owns the selected metadata now. Retire Source
        // selection descriptions before the installed Body clones its finite
        // biography/proposal state; those descriptions are not execution state.
        let offer_generation = host.offer_generation;
        drop((host, choices, queues, grants, empty, lines));
        mark("planning-scratch-retired", 0);
        let metadata = plan.preparation_metadata();
        let plot = conduit_body::ResidentPlot::new(
            metadata.source_document_id.clone(),
            metadata.checked_plot_id.clone(),
        );
        mark("before-display-planning", 0);
        let display_plan = self.prepare_local_display_plan()?;
        mark("display-verified-plan-live", display_plan.fragment_count());
        let display = display_plan.preparation_metadata();
        let display_plot = conduit_body::ResidentPlot::new(
            display.source_document_id.clone(),
            display.checked_plot_id.clone(),
        );
        self.body
            .as_mut()
            .ok_or("body-owner")?
            .propose_verified(
                &[
                    conduit_body::VerifiedBodyPlotPlan { plot, plan: &plan },
                    conduit_body::VerifiedBodyPlotPlan {
                        plot: display_plot,
                        plan: &display_plan,
                    },
                ],
                &[],
                None,
                &self.host,
                &self.boot,
            )
            .map_err(|error| match error {
                conduit_body::BodyLifecycleSessionError::PlanSeal(
                    conduit_body::BodyPlanSealRefusal::Invalid(
                        conduit_body::BodyPlanError::MissingPlot,
                    ),
                ) => "body-workset-incomplete",
                _ => "body-verified-proposal",
            })?;
        drop(display_plan);
        mark("body-local-proposal-live", 2);
        let preparation = (|| -> Result<(), &'static str> {
            let id = &plan.preparation_metadata().fragments[0].fragment_id;
            let verified_lowering =
                conduit_plan_lowering::local_tables::VerifiedLocalLowering::prepare(&plan, id)
                    .map_err(|_| "lowering")?;
            mark("lowered-live", verified_lowering.lowered().nodes.len());
            crate::native_software::check_local(
                self.software.as_ref().ok_or("software-owner")?,
                &verified_lowering,
                &image,
                &self.host,
                &self.boot,
                offer_generation,
            )?;
            mark("local-software-admitted", 15);
            let active = self
                .issuer
                .reserve_verified_play(&plan, id)
                .map_err(|_| "local-play-identity")?;
            let (publisher, reader) = crate::native_face::prepare_local(
                self.body.as_ref().ok_or("body-owner")?,
                &plan,
                id,
                &active,
            )?;
            self.face_reader = Some(reader);
            mark("local-face-live", 1);
            let tables = conduit_plan_lowering::local_tables::VerifiedLocalKernelTables::<
                15,
                14,
                1,
                15,
                14,
                15,
            >::from_verified_lowering(&verified_lowering)
            .map_err(|_| "tables")?;
            mark("local-tables-live", 15);
            drop(tables);
            let kernel = crate::native_kernel::prepare_local(
                &mut self.software,
                &verified_lowering,
                &image,
                (&self.host, &self.boot, offer_generation),
            )?;
            mark("local-kernel-face-live", 15);
            drop(kernel);
            drop(publisher);
            mark("local-kernel-retired", 0);
            Ok(())
        })();
        drop(plan);
        // This diagnostic prepared no worker or native call. Close its proposed
        // Wake normally; retaining a Plan seal is not a started Body Play.
        self.body
            .as_mut()
            .ok_or("body-owner")?
            .lull(&self.host, &self.boot, None)
            .map_err(|_| "body-probe-lull")?;
        mark("body-probe-lulled", 0);
        preparation
    }
}
