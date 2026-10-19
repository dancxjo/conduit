//! Exact owner-route validation and bounded Mask preparation before Show.
use super::*;

struct MaskHostContext<'a> {
    basis: HostBasis,
    host: &'a conduit_core::HostAdvertisement,
}

impl OwnerBrowserMask {
    pub(super) fn prepare(
        basis: HostBasis,
        face: Face,
        route: RemoteOwnerMaskRouteSeal,
        play_sequence: u64,
        interactions_admitted: bool,
    ) -> Result<Self, String> {
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
        let host = crate::membership_abi::owner_component::current_advertisement(
            basis.host_id.clone(),
            basis.boot_id.clone(),
        )?;
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
        Self::prepare_with_host(
            MaskHostContext { basis, host: &host },
            face,
            route.planned_mask,
            Some(inspection),
            Some(selected_lines),
            play_sequence,
            interactions_admitted,
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

    /// Explicit local mechanism fixture; never an exported production profile.
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
        let host = crate::installed_browser::membership_advertisement(
            basis.host_id.clone(),
            basis.boot_id.clone(),
        );
        Self::prepare_with_host(
            MaskHostContext { basis, host: &host },
            face,
            planned,
            route,
            selected_lines,
            play_sequence,
            interactions_admitted,
        )
    }

    fn prepare_with_host(
        context: MaskHostContext<'_>,
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
        let MaskHostContext { basis, host } = context;
        let component = component::prepare(&basis, &planned, &face, host)?;
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
        let mut scheduler = execution::mask_scheduler(fragment, &lowered)?;
        let bytes = serde_json::to_vec(&face).map_err(|error| error.to_string())?;
        if bytes.len() > face_boundary.byte_capacity as usize {
            return Err("owner Face exceeds the browser Mask Fore bound".into());
        }
        match scheduler
            .admit_remote_input(face_boundary.endpoint, face_boundary.cord, 0, &bytes)
            .map_err(|error| format!("admit owner Face to Mask: {error:?}"))?
        {
            RemoteIngressOutcome::Accepted { sequence: 0 } => {}
            _ => return Err("owner Face Mask did not admit the exact Face".into()),
        }
        scheduler
            .close_remote_input(face_boundary.endpoint, face_boundary.cord)
            .map_err(|error| format!("close owner Face input: {error:?}"))?;
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
