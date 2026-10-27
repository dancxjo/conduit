//! Preparation of the true ordinary Mask beside the local sampler. No Step,
//! Body start, caller permit or GPIO operation is released. The separate native
//! diagnostic also checks and retains actual pulse/wait possession.
use super::mask::material;
use super::{c, memory_check::mark, NativeRoot};
use alloc::collections::BTreeMap;

type Bindings = crate::native_authority::NativeBindings<
    conduit_esp32_pulse_capture::Esp32PulseCapture<'static>,
    crate::native_wait::EspWait,
>;
// Checked immutable planning/native custody crosses into software transfer;
// large numeric return temporaries must not surround the native check stack.
struct PreparedSamplerBinding<'a> {
    tables: crate::native_software::DirectTables<'a>,
    image: conduit_plot::flat_image::indexed::IndexedSourceImage<'static>,
    native: Option<Bindings>,
    active: c::ActivePlayIdentity,
    fragment: c::FragmentId,
}
struct PreparedSamplerParts {
    sampler_parts: crate::native_kernel_storage::LocalKernelParts,
    native: Option<Bindings>,
    active: c::ActivePlayIdentity,
    fragment: c::FragmentId,
}
struct PreparedLocalPlots {
    sampler: Option<crate::native_kernel::PreparedKernel>,
    native: Option<Bindings>,
    publisher: crate::native_face::Publisher,
    active: c::ActivePlayIdentity,
    fragment: c::FragmentId,
    mask: crate::native_display::LocalPreparation,
    leases: crate::native_authority::MaskLeaseIssuer,
}

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
    #[cfg(feature = "local-appliance")]
    pub async fn start_local_appliance(&mut self) -> Result<(), &'static str> {
        let prepared = self
            .prepare_local_plots(true)?
            .ok_or("local-plots-not-prepared")?;
        let PreparedLocalPlots {
            mut sampler,
            mut native,
            publisher,
            active,
            fragment,
            mask,
            leases,
        } = *prepared;
        #[cfg(feature = "body-persistence")]
        self.checkpoint_body()?;
        self.display.retain_local(mask)?;
        let mut session = Some(self.display.take_local()?);
        let mut leases = Some(alloc::boxed::Box::new(leases));
        let mut evidence = Some(alloc::boxed::Box::new(
            crate::native_mask_evidence::Consumer::new(),
        ));
        self.local_mask_worker = Some(crate::native_display::task::prepare(
            &mut session,
            &mut leases,
            &mut evidence,
            self.worker_spawner.ok_or("local-worker-executor")?,
        )?);
        self.display.mark_local_task_registered()?;
        let mask_worker = self.local_mask_worker.as_ref().ok_or("local-mask-worker")?;
        embassy_time::with_timeout(
            embassy_time::Duration::from_secs(2),
            mask_worker.wait_ready(),
        )
        .await
        .map_err(|_| "local-mask-ready-timeout")?;
        if mask_worker.status().phase != "ready-held" {
            return Err("local-mask-not-held");
        }
        if let Err(error) = self
            .prepare_local_sampler_worker(&mut sampler, &mut native, publisher, &active, &fragment)
            .await
        {
            self.local_mask_worker
                .as_ref()
                .expect("retained Mask task")
                .abort_preparation();
            return Err(error);
        }
        // Exact canonical transition is accepted only after both actual owners
        // are held in their executor tasks; neither has polled a Step.
        let body = self.body.as_mut().ok_or("local-body-owner")?;
        let current = body.realization().ok_or("local-body-proposal")?;
        if current.play.is_some() {
            return Err("local-body-already-playing");
        }
        let play = conduit_body::BodyPlayIdentity::bind(&current.plan, 1);
        let sign = |sequence| {
            c::bind_sign(&self.host, &self.boot, Some(&play.active_play_id), sequence).sign_id
        };
        let started = current
            .wake
            .body_plan_ready(&current.plan, sign(0))
            .and_then(|wake| wake.body_play_started(&current.plan, &play, sign(1)))
            .map_err(|_| "local-body-start-transition")?;
        body.started(&self.host, &self.boot, play, started)
            .map_err(|_| "local-body-start")?;
        #[cfg(feature = "body-persistence")]
        self.checkpoint_body()?;
        let mask = self.local_mask_worker.as_ref().ok_or("local-mask-worker")?;
        mask.activate(
            self.body.as_ref().ok_or("local-body-owner")?,
            embassy_time::Instant::now().as_millis(),
        )?;
        let sampler = self.worker.as_ref().ok_or("local-sampler-worker")?;
        sampler.accept_start()?;
        mask.accept_start()?;
        // No await between these two releases. Both admitted workers become
        // runnable together at the next executor yield, independently of HTTP.
        sampler.release()?;
        mask.release()?;
        mark("both-local-plots-released", 2);
        Ok(())
    }
    #[cfg(feature = "local-appliance")]
    pub fn report_local_progress(&self) {
        #[derive(serde::Serialize)]
        struct Report {
            event: &'static str,
            millis: u64,
            root_used: usize,
            root_free: usize,
            sampler: Option<crate::native_task::Status>,
            mask: Option<crate::native_display::task::Status>,
            consumed_mask: crate::native_mask_evidence::Status,
            face: Option<crate::native_face::Inspection>,
        }
        let millis = embassy_time::Instant::now().as_millis();
        let memory = esp_alloc::HEAP.stats();
        let report = Report {
            event: "local-appliance-progress",
            millis,
            root_used: memory.current_usage,
            root_free: memory.size.saturating_sub(memory.current_usage),
            sampler: self.worker.as_ref().map(|worker| worker.status()),
            mask: self
                .local_mask_worker
                .as_ref()
                .map(|worker| worker.status()),
            consumed_mask: crate::native_mask_evidence::status(),
            face: self
                .face_reader
                .and_then(|reader| reader.inspect(millis).ok()),
        };
        let mut bytes = [0; 2048];
        if let Ok(length) = serde_json_core::to_slice(&report, &mut bytes) {
            if let Ok(text) = core::str::from_utf8(&bytes[..length]) {
                esp_println::println!("{}", text);
            }
        }
    }
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
        self.prepare_local_plots(false).map(|_| ())
    }
    #[inline(never)]
    fn prepare_local_plots(
        &mut self,
        execute: bool,
    ) -> Result<Option<alloc::boxed::Box<PreparedLocalPlots>>, &'static str> {
        // Keep the sampler planner outside the later assembly frame. Its
        // complete immutable result crosses this synchronous ownership handoff;
        // no plan, authority, or metadata is omitted.
        #[cfg(feature = "local-appliance")]
        let parked = {
            let key = self.restart_verifying_key();
            let parked =
                crate::native_restart::park_box(&mut self.body, &key, &self.host, &self.boot)?;
            mark("cold-body-custody-parked", 1);
            parked
        };
        let sample = self.prepare_local_sampler_plan()?;
        let sampler = self.prepare_local_sampler_parts(&sample, execute)?;
        self.plan_local_mask_from_sampler(
            sample,
            sampler,
            execute,
            #[cfg(feature = "local-appliance")]
            parked,
        )
    }
    // Native possession checks have completed before this frame exists. Keep
    // numeric owners on the ordinary CPU stack only for the remaining cold
    // software/planning phase, releasing their temporary Box before Mask planning.
    #[inline(never)]
    fn plan_local_mask_from_sampler(
        &mut self,
        sample: c::VerifiedPlanMetadata,
        sampler: alloc::boxed::Box<PreparedSamplerParts>,
        execute: bool,
        #[cfg(feature = "local-appliance")] mut parked: crate::native_restart::ParkedBody,
    ) -> Result<Option<alloc::boxed::Box<PreparedLocalPlots>>, &'static str> {
        let sampler = {
            let boxed = sampler;
            *boxed
        };
        mark("ordinary-mask-software-construction-retired", 15);
        let mask_values = take_fixed_values()?;
        mark(
            "ordinary-mask-fixed-canonical-storage",
            core::mem::size_of_val(mask_values),
        );
        let mask = self.prepare_local_mask_plan()?;
        let proposal = self.prepare_local_body_proposal(
            &sample,
            &mask,
            #[cfg(feature = "local-appliance")]
            &mut parked,
        )?;
        drop(sample);
        mark("ordinary-mask-sampler-plan-retired", 15);
        self.assemble_local_plots(
            proposal,
            sampler,
            mask,
            mask_values,
            execute,
            #[cfg(feature = "local-appliance")]
            parked,
        )
    }
    #[inline(never)]
    fn prepare_local_sampler_parts(
        &mut self,
        sample: &c::VerifiedPlanMetadata,
        execute: bool,
    ) -> Result<alloc::boxed::Box<PreparedSamplerParts>, &'static str> {
        let sample_image = crate::native_software::planning_source()?;
        let generation = c::OfferGeneration(self.offer_generation);
        let fragment = &sample.preparation_metadata().fragments[0].fragment_id;
        let tables = crate::native_software::DirectTables::prepare(sample, fragment)
            .map_err(|_| "local-sampler-tables")?;
        crate::native_software::check_local_direct(
            self.software.as_ref().ok_or("software-owner")?,
            &tables,
            &sample_image,
            (&self.host, &self.boot, generation),
        )?;
        mark("ordinary-mask-sampler-tables-live", 15);
        let fragment_id = fragment.clone();
        let active = self
            .issuer
            .reserve_verified_play(sample, fragment)
            .map_err(|_| "local-sampler-play")?;
        let native = if execute || cfg!(feature = "target-mask-native-check") {
            let resident =
                crate::native_software::resident().map_err(|_| "local-sampler-source-facts")?;
            let placements = &sample.preparation_metadata().fragments[0].placements;
            let pulse = &placements[usize::from(resident.roles.pulse.0)].placement_id;
            let wait = &placements[usize::from(resident.roles.tick.0)].placement_id;
            Some(self.issuer.bind_direct_native(
                &mut self.pulse,
                &mut self.wait,
                crate::native_authority::DirectNativeInput {
                    tables: &tables,
                    active: &active,
                    cadence: &resident.cadence,
                    pulse,
                    wait,
                    contract: &self.contract,
                },
            )?)
        } else {
            None
        };
        self.finish_local_sampler_parts(PreparedSamplerBinding {
            tables,
            image: sample_image,
            native,
            active,
            fragment: fragment_id,
        })
    }
    // Keep software/numeric construction slots out of the frame that performs
    // native selection. Boxing the owned result also keeps those slots out of
    // the outer planner frame; this allocation occurs after metadata retirement.
    #[inline(never)]
    fn finish_local_sampler_parts(
        &mut self,
        binding: PreparedSamplerBinding<'_>,
    ) -> Result<alloc::boxed::Box<PreparedSamplerParts>, &'static str> {
        let PreparedSamplerBinding {
            tables,
            image,
            native,
            active,
            fragment,
        } = binding;
        let prepared = crate::native_software::bind_local_direct(
            &mut self.software,
            tables,
            &image,
            (
                &self.host,
                &self.boot,
                c::OfferGeneration(self.offer_generation),
            ),
        )?;
        let sampler_parts = crate::native_kernel_storage::LocalKernelParts::from_checked(prepared);
        Ok(alloc::boxed::Box::new(PreparedSamplerParts {
            sampler_parts,
            native,
            active,
            fragment,
        }))
    }
    #[inline(never)]
    fn prepare_local_body_proposal(
        &self,
        sample: &c::VerifiedPlanMetadata,
        mask: &c::VerifiedPlanMetadata,
        #[cfg(feature = "local-appliance")] parked: &mut crate::native_restart::ParkedBody,
    ) -> Result<conduit_body::PreparedVerifiedBodyProposal, &'static str> {
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
        let prepare = |plots: &[conduit_body::VerifiedBodyPlotPlan<'_>],
                       masks: &[conduit_body::VerifiedBodyMaskTopology<'_>]| {
            #[cfg(feature = "local-appliance")]
            {
                parked.prepare_verified_proposal(plots, masks)
            }
            #[cfg(not(feature = "local-appliance"))]
            {
                self.body
                    .as_ref()
                    .ok_or("body-owner")?
                    .prepare_verified_proposal(plots, masks, None, &self.host, &self.boot)
                    .map_err(|_| "local-mask-body-proposal")
            }
        };
        #[allow(unused_mut)] // Only the parked profile consumes a mutable capsule.
        let mut prepare = prepare;
        let proposal = prepare(
            &[
                conduit_body::VerifiedBodyPlotPlan {
                    plot: resident(sample),
                    plan: sample,
                },
                conduit_body::VerifiedBodyPlotPlan {
                    plot: resident(mask),
                    plan: mask,
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
        )?;
        mark("ordinary-mask-body-proposal-prepared", 2);
        Ok(proposal)
    }
    #[inline(never)]
    fn assemble_local_plots(
        &mut self,
        proposal: conduit_body::PreparedVerifiedBodyProposal,
        sampler: PreparedSamplerParts,
        mask: c::VerifiedPlanMetadata,
        mask_values: &'static mut conduit_kernel::FixedValueStore<2, 8192>,
        execute: bool,
        #[cfg(feature = "local-appliance")] parked: crate::native_restart::ParkedBody,
    ) -> Result<Option<alloc::boxed::Box<PreparedLocalPlots>>, &'static str> {
        let PreparedSamplerParts {
            sampler_parts,
            native,
            active,
            fragment,
        } = sampler;
        let generation = c::OfferGeneration(self.offer_generation);
        let role = material::role().map_err(|_| "local-mask-role")?;
        let context = if execute || cfg!(feature = "target-mask-native-check") {
            // The codec bounds output before allocation and drops dictionaries
            // before final envelope assembly. This independently pinned small
            // Mask needs one placement/configuration at a time (3 KiB scratch).
            let needed = conduit_face::PreparedMaskShowContext::codec_workspace_bytes()
                .saturating_sub(16384)
                + 4096
                + 3072
                + 8192;
            let memory = esp_alloc::HEAP.stats();
            if memory.size.saturating_sub(memory.current_usage) < needed {
                return Err("local-mask-context-capacity");
            }
            // This immutable complete Plan envelope has its own stricter 4 KiB
            // cold extent; complete lifecycle Shows retain the 8 KiB ceiling.
            let source = image()?;
            let context = mask_values
                .with_empty_scratch(|scratch| {
                    conduit_face::PreparedMaskShowContext::prepare_verified_with_workspace(
                        &role,
                        &mask,
                        |gear| {
                            for index in 0..source.gear_count() {
                                let configuration =
                                    source.gear_configuration(index).map_err(|_| ())?;
                                if configuration.gear_id == gear.gear_id {
                                    return Ok(configuration.configuration);
                                }
                            }
                            Err(())
                        },
                        4096,
                        scratch,
                    )
                })
                .map_err(|_| "local-mask-scratch-not-empty")?
                .map_err(|_| "local-mask-show-context")?;
            mark("ordinary-mask-complete-show-context-live", 1);
            Some(context)
        } else {
            None
        };
        #[cfg(feature = "local-appliance")]
        {
            let key = self.restart_verifying_key();
            self.body = Some(crate::native_restart::restore_parked(
                parked, &self.host, &self.boot, &key,
            )?);
            mark("cold-body-custody-restored", 1);
        }
        self.body
            .as_mut()
            .ok_or("body-owner")?
            .commit_verified_proposal(proposal)
            .map_err(|_| "local-mask-body-publication")?;
        mark("ordinary-mask-body-proposal-live", 2);
        let retained = if execute {
            use conduit_body::BodyPlanEvidence;
            use conduit_plan_lowering::selected_operation::{
                SelectedVerifiedForeOperationPlan, SelectedVerifiedOperationPlan,
            };
            let body = self.body.as_ref().ok_or("body-owner")?;
            let current = body.realization().ok_or("local-mask-body-realization")?;
            let routes = conduit_face::AdmittedMaskPlotRoutes::from_sealed_session(
                body,
                &role,
                &mask,
                alloc::vec![conduit_face::SealedMaskPlotRoute {
                    route_id: "route/esp32/local-tm1637".into(),
                    mask_plot: role.plot_identity.clone(),
                    plan_id: current.plan.plan_id().clone(),
                    child_mask_plan_id: None,
                    owner_route_seal_id: None,
                    placement_ids: mask.preparation_metadata().fragments[0]
                        .placements
                        .iter()
                        .map(|p| p.placement_id.clone())
                        .collect(),
                    currently_available: true,
                }],
            )
            .map_err(|_| "local-mask-sealed-route")?;
            let wardrobe = conduit_face::MaskWardrobeControl::new_from_admitted_routes(
                conduit_face::BodyMaskWardrobe::new(
                    current.plan.body_id().clone(),
                    Some(current.plan.wake_id().clone()),
                    conduit_face::MaskWardrobe::new(
                        conduit_face::MaskWardrobeLifetime::Wake,
                        alloc::vec![role.plot_identity.clone()],
                        alloc::vec![role.plot_identity.clone()],
                    )
                    .map_err(|_| "local-mask-wardrobe")?,
                )
                .map_err(|_| "local-mask-wardrobe-scope")?,
                &routes,
                None,
            )
            .map_err(|_| "local-mask-wardrobe-route")?;
            let mask_fragment = &mask.preparation_metadata().fragments[0];
            let mask_active = c::bind_active_play(
                mask.plan_id(),
                &self.host,
                &self.boot,
                self.issuer
                    .next_play_sequence()
                    .ok_or("local-mask-play-exhausted")?,
            );
            let placement = mask_fragment
                .placements
                .iter()
                .find(|p| p.gear_id == role.show_output.gear_id)
                .ok_or("local-mask-renderer-placement")?;
            let source = image()?;
            let configuration = (0..source.gear_count())
                .find_map(|i| {
                    source
                        .gear_configuration(i)
                        .ok()
                        .filter(|gear| gear.gear_id == placement.gear_id)
                })
                .ok_or("local-mask-renderer-configuration")?;
            let lowered = conduit_plan_lowering::local_tables::VerifiedLocalLowering::prepare(
                &mask,
                &mask_fragment.fragment_id,
            )
            .map_err(|_| "local-mask-renderer-lowering")?;
            let (leases, invocation, guard) = self.issuer.prepare_mask_leases(
                self.display.renderer().ok_or("local-mask-renderer-owner")?,
                &configuration.configuration,
                SelectedVerifiedForeOperationPlan {
                    fore: &mask_fragment.fore_ports,
                    operation: SelectedVerifiedOperationPlan {
                        plan: &mask,
                        fragment_id: &mask_fragment.fragment_id,
                        lowered: lowered.lowered(),
                        active: &mask_active,
                        placement_id: &placement.placement_id,
                    },
                },
            )?;
            self.local_mask_scope = Some(guard);
            Some((
                leases,
                invocation,
                routes,
                wardrobe,
                current.plan.wake_id().clone(),
            ))
        } else {
            None
        };
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
        if let Some((leases, invocation, routes, wardrobe, wake)) = retained {
            let mask = crate::native_display::LocalPreparation {
                context: context.ok_or("local-mask-context-unavailable")?,
                kernel: mask_kernel,
                invocation,
                routes,
                wardrobe,
                producer: crate::native_display::FaceScope {
                    producer: crate::native_software::source_identity(),
                    active: active.clone(),
                    wake,
                },
                reader: self.face_reader.ok_or("local-mask-face-reader")?,
            };
            return Ok(Some(alloc::boxed::Box::new(PreparedLocalPlots {
                sampler: Some(sampler_kernel),
                native,
                publisher,
                active,
                fragment,
                mask,
                leases,
            })));
        }
        drop((
            sampler_kernel,
            publisher,
            mask_kernel,
            role,
            context,
            native,
        ));
        self.body
            .as_mut()
            .ok_or("body-owner")?
            .lull(&self.host, &self.boot, None)
            .map_err(|_| "local-mask-probe-lull")?;
        mark("ordinary-mask-probe-lulled", 0);
        Ok(None)
    }
}
