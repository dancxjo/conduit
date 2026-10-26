//! Preparation of the true ordinary Mask beside the local sampler. No Step,
//! native possession, Body start, caller permit or GPIO operation is released.
use super::mask::material;
use super::{c, memory_check::mark, NativeRoot};
use alloc::collections::BTreeMap;

// Exclusive full canonical storage is linked in ordinary DRAM. The image linker
// still enforces the unchanged 36 KiB CPU stack floor. This is neither heap
// borrowing nor received authority; it removes large-allocation fragmentation.
fn take_fixed_values() -> Result<&'static mut conduit_kernel::FixedValueStore<2, 8192>, &'static str>
{
    use core::sync::atomic::{AtomicBool, Ordering};
    static TAKEN: AtomicBool = AtomicBool::new(false);
    static mut VALUES: conduit_kernel::FixedValueStore<2, 8192> =
        match conduit_kernel::FixedValueStore::new(16384) {
            Ok(values) => values,
            Err(_) => panic!("fixed Mask storage budget"),
        };
    if TAKEN.swap(true, Ordering::SeqCst) {
        return Err("local-mask-storage-already-owned");
    }
    // SAFETY: this Boot's only entrance publishes one exclusive reference once;
    // the static object remains live, and no other access is exposed.
    Ok(unsafe { &mut *core::ptr::addr_of_mut!(VALUES) })
}

fn image() -> Result<conduit_plot::flat_image::indexed::IndexedSourceImage<'static>, &'static str> {
    let bytes = material::planning_source_image();
    if c::assigned_plan_payload_digest(bytes) != material::planning_source_sha256() {
        return Err("local-mask-source-digest");
    }
    // SAFETY: complete graph, callbacks, role and Fore are linked by the same
    // checked Make; expected digest is independent of any received identity.
    unsafe {
        conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(bytes).map(|view| {
            view.with_make_gear_decoder(material::decode_planning_gear)
                .with_make_configuration_decoder(material::decode_planning_configuration)
        })
    }
    .map_err(|_| "local-mask-source")
}
impl NativeRoot {
    fn prepare_local_mask_plan(&mut self) -> Result<c::VerifiedPlanMetadata, &'static str> {
        let image = image()?;
        let boundary = material::planning_boundary().map_err(|_| "local-mask-boundary")?;
        let mut host = self.advertisement().map_err(|_| "local-mask-host")?;
        // Actual initialized renderer and compiled routing factories only. Other
        // retained Root providers keep custody but are not candidates here.
        host.capabilities = alloc::vec::Vec::new();
        host.resources = alloc::vec::Vec::new();
        host.bases = alloc::vec::Vec::new();
        self.display
            .append_to_advertisement(&mut host)
            .map_err(|_| "local-mask-renderer")?;
        self.display.append_software_offers(&mut host);
        mark("ordinary-mask-current-offers", host.capabilities.len());
        let choices = conduit_planner::indexed_source::default_placements(
            &image,
            core::slice::from_ref(&host),
        )
        .map_err(|_| "local-mask-selection")?;
        let grants = [self
            .issuer
            .grant_mask(
                self.display
                    .renderer()
                    .ok_or("local-mask-owner")?
                    .identity(),
            )
            .map_err(|_| "local-mask-grant")?];
        let fore = boundary
            .bindings()
            .map(|(direction, port, track)| {
                (
                    conduit_planner::ForeBoundaryKey {
                        direction,
                        front_port_id: port.clone(),
                        track,
                    },
                    conduit_planner::ConnectionQueueLimits {
                        item_capacity: 1,
                        byte_capacity: 8192,
                    },
                )
            })
            .collect();
        let empty = BTreeMap::new();
        let lines = BTreeMap::new();
        let queues = BTreeMap::new();
        let bases = ["conduit.base/local@1".into()];
        mark("before-ordinary-mask-planning", 3);
        let compact = conduit_planner::indexed_source::plan_compact_authoring_consuming_with_connection_limits(
            &image, &boundary, alloc::vec![host], &choices, &bases,
            conduit_planner::PlanningOptions {
                connection_bases:&empty, line_candidates:&lines,
                connection_item_capacity:1, connection_byte_capacity:8192,
                authority_grants:&grants, protected_resource_grants:&[], line_offers:&[],
            }, &queues, &fore,
        ).map_err(|_| "local-mask-planning")?;
        drop((boundary, choices, grants, fore, empty, lines, queues, bases));
        let plan = compact
            .into_verified_metadata()
            .map_err(|_| "local-mask-verification")?;
        mark("ordinary-mask-verified-plan-live", plan.fragment_count());
        Ok(plan)
    }
    #[inline(never)]
    pub fn measure_local_mask(&mut self) -> Result<(), &'static str> {
        let sample_image = crate::native_software::planning_source()?;
        let sample = self.prepare_local_sampler_plan()?;
        let generation = c::OfferGeneration(self.offer_generation);
        // Validate and transfer dormant software before retaining the Body
        // proposal. Construction metadata is no longer needed after this point;
        // numeric tables/drivers alone confer no native or Body execution right.
        let sampler_parts = {
            let fragment = &sample.preparation_metadata().fragments[0].fragment_id;
            let tables = crate::native_software::DirectTables::prepare(&sample, fragment)
                .map_err(|_| "local-sampler-tables")?;
            crate::native_software::check_local_direct(
                self.software.as_ref().ok_or("software-owner")?,
                &tables,
                &sample_image,
                (&self.host, &self.boot, generation),
            )?;
            mark("ordinary-mask-sampler-tables-live", 15);
            // Source and actual owner checks precede transfer. The resulting
            // numeric arrays and owned drivers no longer borrow Plan metadata.
            let prepared = crate::native_software::bind_local_direct(
                &mut self.software,
                tables,
                &sample_image,
                (&self.host, &self.boot, generation),
            )?;
            let parts = crate::native_kernel_storage::LocalKernelParts::from_checked(prepared);
            parts
        };
        mark("ordinary-mask-software-construction-retired", 15);
        let mask_values = take_fixed_values()?;
        mark(
            "ordinary-mask-fixed-canonical-storage",
            core::mem::size_of_val(mask_values),
        );
        let mask = self.prepare_local_mask_plan()?;
        let role = material::role().map_err(|_| "local-mask-role")?;
        let resident = |plan: &c::VerifiedPlanMetadata| {
            let metadata = plan.preparation_metadata();
            conduit_body::ResidentPlot::new(
                metadata.source_document_id.clone(),
                metadata.checked_plot_id.clone(),
            )
        };
        let placement = |gear: &c::GearId| {
            mask.preparation_metadata().fragments[0]
                .placements
                .iter()
                .find(|p| &p.gear_id == gear)
                .map(|p| p.placement_id.clone())
                .ok_or("local-mask-stage")
        };
        let stages = alloc::vec![
            placement(&role.face_input.gear_id)?,
            placement(&role.show_output.gear_id)?,
            placement(&role.interaction_output.gear_id)?
        ];
        let proposal = self
            .body
            .as_ref()
            .ok_or("body-owner")?
            .prepare_verified_proposal(
                &[
                    conduit_body::VerifiedBodyPlotPlan {
                        plot: resident(&sample),
                        plan: &sample,
                    },
                    conduit_body::VerifiedBodyPlotPlan {
                        plot: resident(&mask),
                        plan: &mask,
                    },
                ],
                &[conduit_body::VerifiedBodyMaskTopology {
                    face: conduit_body::BodyFaceSelector {
                        plot: None,
                        source_placement_id: None,
                    },
                    chains: alloc::vec![conduit_body::VerifiedBodyMaskChain {
                        plan: &mask,
                        stage_placement_ids: stages
                    }],
                }],
                None,
                &self.host,
                &self.boot,
            )
            .map_err(|_| "local-mask-body-proposal")?;
        mark("ordinary-mask-body-proposal-prepared", 2);
        let fragment = sample.preparation_metadata().fragments[0]
            .fragment_id
            .clone();
        let active = self
            .issuer
            .reserve_verified_play(&sample, &fragment)
            .map_err(|_| "local-sampler-play")?;
        drop(sample);
        mark("ordinary-mask-sampler-plan-retired", 15);
        let mask_parts = {
            let image = image()?;
            let boundary = material::planning_boundary().map_err(|_| "local-mask-boundary")?;
            let fragment = &mask.preparation_metadata().fragments[0].fragment_id;
            let lowered = conduit_plan_lowering::local_tables::VerifiedLocalLowering::prepare(
                &mask, fragment,
            )
            .map_err(|_| "local-mask-lowering")?;
            conduit_environment_runtime::mask_kernel::PreparedMaskKernel::prepare_local_parts(
                conduit_environment_runtime::mask_kernel::LocalMaskInput {
                    verified: &lowered,
                    image: &image,
                    boundary: &boundary,
                    role: &role,
                    scope: (&self.host, &self.boot, generation),
                },
            )
            .map_err(|_| "local-mask-kernel")?
        };
        drop(mask);
        mark("ordinary-mask-plan-lowering-retired", 3);
        let sampler_kernel = crate::native_kernel::prepare_local_direct(sampler_parts)?;
        mark("ordinary-mask-sampler-kernel-live", 15);
        self.body
            .as_mut()
            .ok_or("body-owner")?
            .commit_verified_proposal(proposal)
            .map_err(|_| "local-mask-body-publication")?;
        mark("ordinary-mask-body-proposal-live", 2);
        let (publisher, reader) = crate::native_face::prepare_installed_local(
            self.body.as_ref().ok_or("body-owner")?,
            &fragment,
            &active,
            generation,
        )?;
        self.face_reader = Some(reader);
        mark("ordinary-mask-local-face-live", 1);
        let required = mask_parts.required_fixed_heap_bytes() + 8192;
        let memory = esp_alloc::HEAP.stats();
        if memory.size.saturating_sub(memory.current_usage) < required {
            return Err("local-mask-kernel-capacity");
        }
        let mask_kernel = mask_parts
            .finish_with_fixed_storage(mask_values)
            .map_err(|_| "local-mask-kernel")?;
        mark("both-ordinary-local-kernels-face-live", 18);
        drop((sampler_kernel, publisher, mask_kernel, role));
        self.body
            .as_mut()
            .ok_or("body-owner")?
            .lull(&self.host, &self.boot, None)
            .map_err(|_| "local-mask-probe-lull")?;
        mark("ordinary-mask-probe-lulled", 0);
        Ok(())
    }
}
