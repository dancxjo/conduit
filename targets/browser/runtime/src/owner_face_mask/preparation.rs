//! Checked Mask preparation before kernel execution and pending Show.
use super::*;

impl OwnerBrowserMask {
    pub(super) fn prepare(
        basis: HostBasis,
        face: Face,
        route: RemoteOwnerMaskRouteSeal,
        play_sequence: u64,
        interactions_admitted: bool,
    ) -> Result<Self, String> {
        Self::prepare_with_history_look(
            basis,
            face,
            route,
            play_sequence,
            interactions_admitted,
            None,
        )
    }

    pub(super) fn prepare_with_history_look(
        basis: HostBasis,
        face: Face,
        route: RemoteOwnerMaskRouteSeal,
        play_sequence: u64,
        interactions_admitted: bool,
        history_look: Option<&conduit_face::LookInfo>,
    ) -> Result<Self, String> {
        if history_look.is_some()
            && route.planned_mask.mask.plot_name != conduit_face::HISTORY_COMPONENT_MASK_ROOT
        {
            return Err("history startup Look requires reviewed history Root".into());
        }
        face.validate()
            .map_err(|error| format!("owner Face invalid: {error:?}"))?;
        if face.basis.body_id.as_ref() != Some(&basis.body_id) {
            return Err("owner Face belongs to another Body".into());
        }
        if route.body_id != basis.body_id
            || route.face_id != face.identity
            || route.face_revision != face.revision
        {
            return Err("owner route differs from the current Face".into());
        }
        let host = crate::installed_browser::membership_advertisement(
            basis.host_id.clone(),
            basis.boot_id.clone(),
        );
        route
            .validate_mask_host_offer(&host)
            .map_err(|error| format!("owner Mask route differs from this browser: {error:?}"))?;
        let inspection = RouteView {
            plan_id: route.route_plan_id.as_str().into(),
            owner_host_id: route.owner_host.host_id.as_str().into(),
            owner_boot_id: route.owner_host.boot_id.as_str().into(),
            mask_host_id: route.mask_host.host_id.as_str().into(),
            mask_boot_id: route.mask_host.boot_id.as_str().into(),
        };
        let selected_lines = (
            route.face_line.clone(),
            route.return_line.clone(),
            route.interaction_line.clone(),
        );
        Self::prepare_planned_with_look(
            basis,
            face,
            route.planned_mask,
            Some(inspection),
            Some(selected_lines),
            play_sequence,
            interactions_admitted,
            history_look,
        )
    }

    /// Component-only proof of the local Mask kernel and ABI, without an
    /// owner-issued route or a live browser carrier.
    #[cfg(test)]
    pub(super) fn prepare_component_fixture(
        basis: HostBasis,
        face: Face,
        play_sequence: u64,
        interactions_admitted: bool,
    ) -> Result<Self, String> {
        let host = crate::installed_browser::membership_advertisement(
            basis.host_id.clone(),
            basis.boot_id.clone(),
        );
        let planned = crate::workspace_mask::plan::planned_mask(
            &host,
            crate::workspace_mask::plan::MASK_SOURCE,
            "browser-graphical",
        )?;
        Self::prepare_planned(
            basis,
            face,
            planned,
            None,
            None,
            play_sequence,
            interactions_admitted,
        )
    }

    #[cfg(test)]
    pub(super) fn prepare_planned(
        basis: HostBasis,
        face: Face,
        planned: conduit_face::PlannedMaskPlot,
        route: Option<RouteView>,
        selected_lines: Option<(
            conduit_core::AdmittedLine,
            conduit_core::AdmittedLine,
            Option<conduit_core::AdmittedLine>,
        )>,
        play_sequence: u64,
        interactions_admitted: bool,
    ) -> Result<Self, String> {
        Self::prepare_planned_with_look(
            basis,
            face,
            planned,
            route,
            selected_lines,
            play_sequence,
            interactions_admitted,
            None,
        )
    }

    /// The Look input is supplied independently of the planned renderer and
    /// reproduced against the reviewed Source before the Mask kernel starts.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare_planned_with_look(
        basis: HostBasis,
        face: Face,
        planned: conduit_face::PlannedMaskPlot,
        route: Option<RouteView>,
        selected_lines: Option<(
            conduit_core::AdmittedLine,
            conduit_core::AdmittedLine,
            Option<conduit_core::AdmittedLine>,
        )>,
        play_sequence: u64,
        interactions_admitted: bool,
        selected_look: Option<&conduit_face::LookInfo>,
    ) -> Result<Self, String> {
        if selected_look.is_some()
            && planned.mask.plot_name != conduit_face::HISTORY_COMPONENT_MASK_ROOT
        {
            return Err("history startup Look requires reviewed history Root".into());
        }
        if selected_look.is_some() {
            reproduce_history_plan(&basis, &planned, selected_lines.as_ref(), selected_look)?;
        }
        let component = component::prepare_with_look(&basis, &planned, &face, selected_look)?;
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
            .ok_or("owner Face has no semantic subject")?
            .identity
            .clone();
        let show = MaskShow::prepared(
            &planned,
            &face,
            play.clone(),
            face_subject,
            "browser/document".into(),
            SignId::from(format!(
                "sign/browser-owner-face-prepared/{}",
                face.revision
            )),
        )
        .map_err(|error| format!("prepare owner Face Show: {error:?}"))?;
        let fragment = planned
            .plan
            .fragments
            .first()
            .ok_or("browser Mask Plan has no fragment")?;
        let lowered = lower_plan_fragment(fragment)
            .map_err(|error| format!("lower owner Face Mask: {error:?}"))?;
        let face_boundary =
            execution::exact_boundary(&lowered.fore_ports, "face", PortDirection::Input)?;
        let show_boundary =
            execution::exact_boundary(&lowered.fore_ports, "show", PortDirection::Output)?.clone();
        let interaction_boundary =
            execution::exact_boundary(&lowered.fore_ports, "interaction", PortDirection::Output)?
                .clone();
        if let Some((face_line, return_line, interaction_line)) = selected_lines {
            let selected = face_boundary.selected_line.is_some()
                || show_boundary.selected_line.is_some()
                || interaction_boundary.selected_line.is_some();
            if selected
                && (face_boundary.selected_line.as_ref() != Some(&face_line)
                    || show_boundary.selected_line.as_ref() != Some(&return_line)
                    || interaction_boundary.selected_line != interaction_line)
            {
                return Err("browser Mask lowered a different selected carrier Line".into());
            }
        }
        let bytes = serde_json::to_vec(&face).map_err(|error| error.to_string())?;
        let mut scheduler = execution::single_face_scheduler(fragment, &lowered, &bytes)?;
        let pending = loop {
            if let Some(request) = scheduler.next_host_request() {
                break request;
            }
            match scheduler
                .step()
                .map_err(|error| format!("start owner Face Mask: {error:?}"))?
            {
                SchedulerStatus::Progress { .. } => {}
                other => {
                    return Err(format!(
                        "owner Face Mask did not request DOM face: {other:?}"
                    ));
                }
            }
        };
        let presented = scheduler
            .host_value(pending.input.value)
            .map_err(|error| format!("read owner Face Mask input: {error:?}"))?;
        if presented != bytes.as_slice() {
            return Err("browser Mask received a different owner Face".into());
        }
        Ok(Self {
            startup_history_look: selected_look.cloned(),
            component,
            body_id: basis.body_id,
            face,
            planned,
            play,
            route,
            show,
            scheduler,
            show_boundary,
            interaction_boundary,
            pending_node: pending.node,
            pending_request: pending.request,
            pending_interaction: None,
            interactions_admitted,
        })
    }
}

/// Reproduce the entire checked local Plan, then compare after removing only
/// the independently validated routed carrier selections. This comparison
/// admits no configuration, queue, activation, connection or identity override.
fn reproduce_history_plan(
    basis: &HostBasis,
    planned: &conduit_face::PlannedMaskPlot,
    selected_lines: Option<&(
        conduit_core::AdmittedLine,
        conduit_core::AdmittedLine,
        Option<conduit_core::AdmittedLine>,
    )>,
    look: Option<&conduit_face::LookInfo>,
) -> Result<(), String> {
    use conduit_core::{PlotIdentity, PortDirection, seal_plan_with_activation_entries};
    let host = crate::installed_browser::membership_advertisement(
        basis.host_id.clone(),
        basis.boot_id.clone(),
    );
    let budget = |direction, id: &conduit_core::PortId| -> Result<u32, String> {
        let mut matches = planned
            .plan
            .fragments
            .iter()
            .flat_map(|f| &f.fore_ports)
            .filter(|p| p.direction == direction && &p.front_port_id == id);
        let port = matches.next().ok_or("history Fore absent")?;
        if matches.next().is_some()
            || port.item_capacity != 1
            || port.byte_capacity == 0
            || port.byte_capacity as usize > conduit_face::MAX_OWNER_FACE_RESPONSE_BYTES
        {
            return Err("history Fore ambiguous or unbounded".into());
        }
        if selected_lines.is_some() {
            let line = port
                .selected_line
                .as_ref()
                .ok_or("history Fore carrier missing")?;
            let limits = line.binding.limits;
            if limits.maximum_in_flight_items < port.item_capacity
                || limits.maximum_payload_bytes < port.byte_capacity
                || limits.maximum_frame_bytes < port.byte_capacity
                || limits.maximum_buffered_bytes < port.byte_capacity
            {
                return Err("history Fore exceeds its admitted carrier".into());
            }
        }
        Ok(port.byte_capacity)
    };
    let limits = if let Some((face_line, show_line, interaction_line)) = selected_lines {
        if interaction_line.is_none() {
            return Err("history interaction carrier missing".into());
        }
        budget(PortDirection::Input, &planned.mask.face_input.front_port_id)?;
        budget(
            PortDirection::Output,
            &planned.mask.show_output.front_port_id,
        )?;
        budget(
            PortDirection::Output,
            &planned.mask.interaction_output.front_port_id,
        )?;
        Some(conduit_browser_mask_offer::OwnerCarrierForeLimits {
            face_bytes: face_line
                .binding
                .limits
                .maximum_payload_bytes
                .min(conduit_face::MAX_OWNER_FACE_RESPONSE_BYTES as u32),
            show_bytes: show_line
                .binding
                .limits
                .maximum_payload_bytes
                .min(conduit_face::MAX_OWNER_FACE_RESPONSE_BYTES as u32),
        })
    } else {
        None
    };
    let (expected, _) = conduit_browser_mask_offer::planned_mask_configuration_with_look(
        &host,
        conduit_face::HISTORY_COMPONENT_MASK_SOURCE,
        conduit_face::HISTORY_COMPONENT_MASK_ROOT,
        limits,
        look,
    )?;
    let mut fragments = planned.plan.fragments.clone();
    for fore in fragments.iter_mut().flat_map(|f| &mut f.fore_ports) {
        fore.selected_line = None;
    }
    let plan = &planned.plan;
    let local = seal_plan_with_activation_entries(
        PlotIdentity {
            source_document_id: plan.source_document_id.clone(),
            checked_plot_id: plan.checked_plot_id.clone(),
            expanded_plot_id: plan.expanded_plot_id.clone(),
        },
        plan.completion_policy,
        plan.realization_backs.clone(),
        plan.activations.clone(),
        fragments,
    );
    if expected.mask != planned.mask || expected.plan != local {
        return Err("history routed Plan differs from exact checked startup reproduction".into());
    }
    Ok(())
}
