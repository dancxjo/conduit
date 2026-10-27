//! Delivery checkpoint diagnostic, never a claim of a started appliance.
use super::*;
#[path = "native_sampler_offer.rs"]
mod sampler_offers;
#[inline(never)]
pub(crate) fn mark(phase: &str, item: usize) {
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
    #[cfg(feature = "target-memory-check")]
    #[inline(never)]
    pub fn measure_local_sampler(&mut self) -> Result<(), &'static str> {
        let image = crate::native_software::planning_source()?;
        let plan = self.prepare_local_sampler_plan()?;
        let offer_generation = c::OfferGeneration(self.offer_generation);
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
        mark("body-local-proposal-live", 2);
        let display_kernel = {
            let id = &display_plan.preparation_metadata().fragments[0].fragment_id;
            let lowered = conduit_plan_lowering::local_tables::VerifiedLocalLowering::prepare(
                &display_plan,
                id,
            )
            .map_err(|_| "display-local-lowering")?;
            mark("before-local-display-kernel", 4);
            self.display
                .prepare_local_kernel(&lowered, (&self.host, &self.boot, offer_generation))?
        };
        drop(display_plan);
        mark("local-display-kernel-live", 4);
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
            mark("both-local-kernels-face-live", 19);
            drop(kernel);
            drop(publisher);
            mark("local-kernel-retired", 0);
            Ok(())
        })();
        drop(plan);
        drop(display_kernel);
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
