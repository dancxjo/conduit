//! Prepare finite Mask execution and its exact presentation evidence.
use super::*;

impl BrowserMaskRuntime {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare_route(
        body_id: BodyId,
        host_id: HostId,
        boot_id: BootId,
        face: Face,
        wake: Wake,
        base_plan: BodyPlan,
        select_alternate: bool,
        play_sequence: u64,
    ) -> Result<(Self, BrowserMaskEffect), String> {
        let host = crate::installed_browser::membership_advertisement(host_id, boot_id);
        let planned = plan::planned_mask(&host, plan::MASK_SOURCE, "browser-graphical")?;
        let alternate = plan::planned_mask(
            &host,
            plan::ALTERNATE_MASK_SOURCE,
            "browser-graphical-alternate",
        )?;
        let chains = [&planned, &alternate]
            .into_iter()
            .map(|p| BodyMaskChainPlan {
                plan: p.plan.clone(),
                stage_placement_ids: p
                    .plan
                    .fragments
                    .iter()
                    .flat_map(|f| &f.placements)
                    .map(|v| v.placement_id.clone())
                    .collect(),
            })
            .collect();
        let body_plan = BodyPlan::seal_with_masks(
            &wake,
            base_plan.plots.clone(),
            vec![BodyMaskTopology {
                face: BodyFaceSelector {
                    plot: base_plan.plots.first().map(|f| f.plot.clone()),
                    source_placement_id: None,
                },
                chains,
            }],
        )
        .map_err(|e| format!("seal browser Mask Body Plan: {e:?}"))?;
        let (planned, alternate) = if select_alternate {
            (alternate, planned)
        } else {
            (planned, alternate)
        };
        Self::prepare_admitted_route(
            body_id,
            face,
            wake,
            body_plan,
            base_plan,
            planned,
            alternate,
            play_sequence,
        )
    }

    /// Prepare only against Mask chains already present in the owner's exact
    /// admitted Body Plan. No topology is added and no replacement Plan is sealed.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_in_body_plan(
        body_id: BodyId,
        host_id: HostId,
        boot_id: BootId,
        face: Face,
        wake: Wake,
        body_plan: BodyPlan,
        play_sequence: u64,
    ) -> Result<(Self, BrowserMaskEffect), String> {
        body_plan
            .validate_for(&wake)
            .map_err(|error| format!("browser Mask Body Plan: {error:?}"))?;
        if body_plan.body_id != body_id || face.basis.body_id.as_ref() != Some(&body_id) {
            return Err("browser Mask Face and owner Body Plan have different Bodies".into());
        }
        let host = crate::installed_browser::membership_advertisement(host_id, boot_id);
        let planned = plan::planned_mask(&host, plan::MASK_SOURCE, "browser-graphical")?;
        let alternate = plan::planned_mask(
            &host,
            plan::ALTERNATE_MASK_SOURCE,
            "browser-graphical-alternate",
        )?;
        Self::prepare_admitted_route(
            body_id,
            face,
            wake,
            body_plan.clone(),
            body_plan,
            planned,
            alternate,
            play_sequence,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_admitted_route(
        body_id: BodyId,
        face: Face,
        wake: Wake,
        body_plan: BodyPlan,
        base_plan: BodyPlan,
        planned: PlannedMaskPlot,
        alternate: PlannedMaskPlot,
        play_sequence: u64,
    ) -> Result<(Self, BrowserMaskEffect), String> {
        let routes = plan::admitted_routes(&body_plan, &planned, &alternate, true, true)?;
        let mask = planned.mask.clone();
        let wardrobe = MaskWardrobe::new(MaskWardrobeLifetime::Body, vec![], vec![])
            .map_err(|error| format!("{error:?}"))?;
        let scoped =
            BodyMaskWardrobe::new(body_id, None, wardrobe).map_err(|error| format!("{error:?}"))?;
        let scoped_body_id = scoped.body_id.clone();
        let mut control =
            MaskWardrobeControl::new(&scoped_body_id, scoped, &body_plan, &routes, None)
                .map_err(|error| format!("{error:?}"))?;
        let wardrobe_action = control
            .apply(
                0,
                MaskWardrobeAction::Wear(mask.plot_identity.clone()),
                &routes,
            )
            .map_err(|error| format!("{error:?}"))?;
        let terminal = planned.show_placement();
        let play = bind_active_play(
            &planned.plan.plan_id,
            &terminal.host_id,
            &terminal.boot_id,
            play_sequence,
        );
        let face_subject = face
            .subjects
            .first()
            .ok_or_else(|| "browser Mask refuses a Face without a subject".to_string())?
            .identity
            .clone();
        let show = MaskShow::prepared(
            &planned,
            &face,
            play.clone(),
            face_subject,
            "browser/document".into(),
            SignId::from(format!("sign/browser-mask-prepared/{}", face.revision)),
        )
        .map_err(|error| format!("{error:?}"))?;
        let fragment = planned
            .plan
            .fragments
            .first()
            .ok_or("browser Mask Plan has no fragment")?;
        let lowered = lower_plan_fragment(fragment)
            .map_err(|error| format!("lower browser Mask: {error:?}"))?;
        let face_boundary =
            execution::exact_boundary(&lowered.fore_ports, "face", PortDirection::Input)?;
        let show_boundary =
            execution::exact_boundary(&lowered.fore_ports, "show", PortDirection::Output)?.clone();
        let interaction_boundary =
            execution::exact_boundary(&lowered.fore_ports, "interaction", PortDirection::Output)?
                .clone();
        let mut scheduler = execution::mask_scheduler(fragment, &lowered)?;
        let face_bytes = serde_json::to_vec(&face).map_err(|error| error.to_string())?;
        if face_bytes.len() > face_boundary.byte_capacity as usize {
            return Err("browser Mask Face exceeds its sealed Fore bound".into());
        }
        match scheduler
            .admit_remote_input(face_boundary.endpoint, face_boundary.cord, 0, &face_bytes)
            .map_err(|error| format!("inject browser Mask Face: {error:?}"))?
        {
            conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { sequence: 0 } => {}
            outcome => {
                return Err(format!(
                    "browser Mask Face was not admitted exactly: {outcome:?}"
                ))
            }
        }
        scheduler
            .close_remote_input(face_boundary.endpoint, face_boundary.cord)
            .map_err(|error| format!("close browser Mask Face: {error:?}"))?;
        let pending = loop {
            if let Some(request) = scheduler.next_host_request() {
                break request;
            }
            match scheduler
                .step()
                .map_err(|error| format!("start browser Mask Play: {error:?}"))?
            {
                SchedulerStatus::Progress { .. } => {}
                other => {
                    return Err(format!(
                        "browser Mask did not suspend on its DOM effect: {other:?}"
                    ))
                }
            }
        };
        let presented = scheduler
            .host_value(pending.input.value)
            .map_err(|error| format!("read browser Mask effect input: {error:?}"))?;
        if presented != face_bytes.as_slice() {
            return Err("browser Mask Back did not receive the exact Face".into());
        }
        let fore_receipt = lowered
            .fore_ports
            .iter()
            .map(|port| BrowserMaskForeReceipt {
                front_port_id: port.front_port_id.as_str().into(),
                direction: port.direction,
                track: port.track,
                value_kind: port.value_kind.as_str().into(),
                endpoint: port.endpoint.0,
                cord: port.cord.0,
            })
            .collect();
        let effect = BrowserMaskEffect {
            schema: "conduit.browser/mask-effect@1".into(),
            mask_plot: mask.plot_identity,
            mask_plan_id: planned.plan.plan_id.clone(),
            mask_play: play.clone(),
            show_id: show.show_id.as_str().into(),
            manifestation_id: show.show.manifestation_id.as_str().into(),
            placement_id: show.show.placement_id.clone(),
            face_id: face.identity.as_str().into(),
            face_revision: face.revision,
            text: face.text.clone(),
            actions: face.actions.clone(),
        };
        Ok((
            Self {
                routes,
                wardrobe_action,
                planned,
                alternate,
                body_plan,
                basis_plan: base_plan,
                wake,
                play,
                face,
                show,
                scheduler,
                show_boundary,
                interaction_boundary,
                pending_node: pending.node,
                pending_request: pending.request,
                fore_receipt,
                pending_interaction_node: None,
                pending_interaction_request: None,
                interaction_receipt: None,
            },
            effect,
        ))
    }
}
