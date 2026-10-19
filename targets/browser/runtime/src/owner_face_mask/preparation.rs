//! One browser Mask kernel preparation and acknowledgement path.
use super::*;

pub(super) struct MaskHostBasis {
    pub body_id: Option<BodyId>,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub visual_constraints: Option<component::VisualConstraints>,
}

impl OwnerBrowserMask {
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
        Self::prepare_context(
            MaskHostBasis {
                body_id: Some(basis.body_id),
                host_id: basis.host_id,
                boot_id: basis.boot_id,
                visual_constraints: basis.visual_constraints,
            },
            face,
            planned,
            route,
            selected_lines,
            play_sequence,
            interactions_admitted,
        )
    }
    pub(super) fn prepare_context(
        basis: preparation::MaskHostBasis,
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
        let component = component::prepare(&basis, &planned, &face)?;
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

    pub(super) fn acknowledge(&mut self, ack: Acknowledgement) -> Result<(), String> {
        if ack.show_id != self.show.show_id.as_str()
            || ack.face_id != self.face.identity.as_str()
            || ack.face_revision != self.face.revision.to_string()
            || self.show.show.lifecycle != ManifestationLifecycle::Prepared
        {
            return Err("owner Face Show acknowledgement is stale or mismatched".into());
        }
        let show_bytes = serde_json::to_vec(&self.show.show).map_err(|error| error.to_string())?;
        let output = self
            .scheduler
            .store_host_value(&show_bytes)
            .map_err(|error| format!("store owner Face Show: {error:?}"))?;
        self.scheduler
            .complete_host_call(
                self.pending_node,
                self.pending_request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(output, MASK_BYTES)
                            .map_err(|_| "owner Face Show bound")?,
                    ),
                    failure: None,
                },
            )
            .map_err(|error| format!("complete browser DOM Mask Host Call: {error:?}"))?;
        let offer = loop {
            if let Some(offer) = self
                .scheduler
                .remote_egress_offer(self.show_boundary.endpoint, self.show_boundary.cord)
                .map_err(|error| format!("offer owner Face Show: {error:?}"))?
            {
                break offer;
            }
            match self
                .scheduler
                .step()
                .map_err(|error| format!("advance owner Face Mask: {error:?}"))?
            {
                SchedulerStatus::Progress { .. } => {}
                other => return Err(format!("owner Face Mask ended without Show: {other:?}")),
            }
        };
        let observed = self
            .scheduler
            .host_value(offer.value)
            .map_err(|error| format!("read owner Face Show: {error:?}"))?;
        if observed != show_bytes.as_slice() {
            return Err("browser Mask emitted a different Show".into());
        }
        self.scheduler
            .remote_egress_accept(
                self.show_boundary.endpoint,
                self.show_boundary.cord,
                offer.sequence,
            )
            .map_err(|error| format!("accept owner Face Show: {error:?}"))?;
        self.scheduler
            .remote_egress_delivered(
                self.show_boundary.endpoint,
                self.show_boundary.cord,
                offer.sequence,
            )
            .map_err(|error| format!("deliver owner Face Show: {error:?}"))?;
        let pending = self
            .scheduler
            .next_host_request()
            .ok_or("browser Mask did not suspend for its read-only interaction boundary")?;
        if pending.request != conduit_kernel::RequestId(1) {
            return Err("browser Mask exposed an unexpected request after Show".into());
        }
        self.pending_interaction = Some((pending.node, pending.request));
        self.show = self
            .show
            .transition(
                ManifestationLifecycle::Available,
                SignId::from(format!(
                    "sign/browser-owner-face-shown/{}",
                    self.face.revision
                )),
            )
            .map_err(|error| format!("activate owner Face Show: {error:?}"))?;
        self.show
            .validate(&self.face)
            .map_err(|error| format!("validate owner Face Show: {error:?}"))
    }
}
