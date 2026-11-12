//! Trusted projection of sealed external Fore Lines, never transport authority.
use crate::*;
use conduit_core as c;

/// Explicit session pins selected by trusted preparation for one existing Fore.
/// These identities do not prove authentication, availability or possession.
pub struct SelectedForeSession {
    pub front_port_id: c::PortId,
    pub direction: c::PortDirection,
    pub track: c::ConnectionTrack,
    pub session: c::AssignedIdentity,
    pub session_contract: c::AssignedIdentity,
}

pub struct GeneratedLineForePlan {
    kernel: GeneratedEmbeddedPlan,
    fore: Vec<c::AssignedForeRequirement>,
    local: Vec<c::AssignedForeRequirement>,
    lines: Vec<c::AssignedLineForeRequirement>,
    bindings: Vec<c::AssignedRemoteBinding>,
}
impl GeneratedLineForePlan {
    pub fn kernel(&self) -> &GeneratedEmbeddedPlan {
        &self.kernel
    }
    /// Complete Body metadata inventory in canonical numeric endpoint order.
    pub fn fore(&self) -> &[c::AssignedForeRequirement] {
        &self.fore
    }
    pub fn profile(&self) -> c::AssignedLineForeProfile<'_> {
        c::AssignedLineForeProfile {
            local: &self.local,
            lines: &self.lines,
        }
    }
    pub fn remote_bindings(&self) -> &[c::AssignedRemoteBinding] {
        &self.bindings
    }
    pub fn encode(&self, maxima: c::AssignedPlanMaxima) -> Result<Vec<u8>, GenerationError> {
        crate::assigned_plan::encode_line_fore(&self.kernel, maxima, self.profile(), &[], false)
    }
    pub fn encode_with_resident_computations(
        &self,
        maxima: c::AssignedPlanMaxima,
        resident: &[BoundResidentExpression],
    ) -> Result<Vec<u8>, GenerationError> {
        crate::assigned_plan::encode_line_fore(&self.kernel, maxima, self.profile(), resident, true)
    }
    pub fn layout_requirements(
        &self,
        maxima: c::AssignedPlanMaxima,
    ) -> Result<Vec<c::AssignedRecordRequirement>, GenerationError> {
        self.checked_layout(self.encode(maxima)?, maxima)
    }
    /// Resident references retain the SAME checked structural layout profile.
    pub fn layout_requirements_with_resident_computations(
        &self,
        maxima: c::AssignedPlanMaxima,
        resident: &[BoundResidentExpression],
    ) -> Result<Vec<c::AssignedRecordRequirement>, GenerationError> {
        self.checked_layout(
            self.encode_with_resident_computations(maxima, resident)?,
            maxima,
        )
    }
    fn checked_layout(
        &self,
        bytes: Vec<u8>,
        maxima: c::AssignedPlanMaxima,
    ) -> Result<Vec<c::AssignedRecordRequirement>, GenerationError> {
        let computations = crate::assigned_computation_requirements(&self.kernel)?;
        let host_calls: Vec<_> = self
            .kernel
            .host_calls
            .iter()
            .map(|call| c::AssignedIdentity::from_text(&call.contract_id))
            .collect();
        let resources: Vec<_> = self
            .kernel
            .resources
            .iter()
            .map(|resource| resource.resource)
            .collect();
        let maximum_text_bytes = computations
            .iter()
            .map(|pin| pin.text_bytes)
            .max()
            .unwrap_or(0);
        let maximum_total_text_bytes = computations
            .iter()
            .try_fold(0u32, |sum, pin| sum.checked_add(pin.text_bytes))
            .ok_or(GenerationError::InvalidFragment)?;
        c::decode_assigned_plan_with_line_fore(
            &bytes,
            maxima,
            c::AssignedPlanRequirements {
                host: c::AssignedIdentity::from_text(&self.kernel.host_id),
                boot: c::AssignedIdentity::from_text(&self.kernel.boot_id),
                host_calls: &host_calls,
                resources: &resources,
                remote_bindings: &self.bindings,
            },
            c::AssignedComputationProfile {
                maximum_text_bytes,
                maximum_total_text_bytes,
                requirements: &computations,
            },
            self.profile(),
        )
        .map_err(|_| GenerationError::InvalidFragment)?;
        crate::assigned_layout::layout_records_profile(
            bytes,
            c::ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA,
            Some(self.profile()),
        )
    }
}

/// Lower the original verified Plan, preserving its selected Lines. Refuse
/// missing, duplicate, foreign or surplus session choices before projection.
/// No local-only decoder or writer is used to interpret the resulting schema.
pub fn generate_line_fore_native_plan(
    plan: &c::Plan,
    fragment: &c::FragmentId,
    bounds: EmbeddedImageBounds,
    computations: EmbeddedComputationBounds,
    maximum_fore_ports: usize,
    sessions: &[SelectedForeSession],
) -> Result<GeneratedLineForePlan, GenerationError> {
    if sessions.len() > 16 {
        return Err(GenerationError::InvalidFragment);
    }
    let (kernel, ports) = crate::fore::prepare_fore(
        plan,
        fragment,
        bounds,
        computations,
        maximum_fore_ports,
        true,
    )?;
    let mut result = GeneratedLineForePlan {
        kernel,
        fore: Vec::new(),
        local: Vec::new(),
        lines: Vec::new(),
        bindings: Vec::new(),
    };
    let mut used = [false; 16];
    type SharedSession = (
        c::AssignedRemoteBinding,
        c::AssignedIdentity,
        c::AssignedIdentity,
        [u8; 32],
    );
    let mut shared_sessions: Vec<SharedSession> = Vec::new();
    for port in ports {
        let fore = crate::fore::requirement(&port);
        result.fore.push(fore);
        let matching: Vec<_> = sessions
            .iter()
            .enumerate()
            .filter(|(_, session)| {
                session.front_port_id.as_str() == port.front_port_id.as_str()
                    && session.direction == port.direction
                    && session.track == port.track
            })
            .collect();
        let Some(line) = &port.selected_line else {
            if !matching.is_empty() {
                return Err(GenerationError::InvalidFragment);
            }
            result.local.push(fore);
            continue;
        };
        if matching.len() != 1 {
            return Err(GenerationError::InvalidFragment);
        }
        let (index, session) = matching[0];
        if used[index] {
            return Err(GenerationError::InvalidFragment);
        }
        used[index] = true;
        let (local, peer) = match port.direction {
            c::PortDirection::Input => (&line.binding.sink, &line.binding.source),
            c::PortDirection::Output => (&line.binding.source, &line.binding.sink),
        };
        if local.host_id.as_str() != result.kernel.host_id
            || local.boot_id.as_str() != result.kernel.boot_id
            || line.binding.limits.maximum_in_flight_items < port.item_capacity
            || line.binding.limits.maximum_payload_bytes < port.byte_capacity
            || line.binding.limits.maximum_buffered_bytes < port.byte_capacity
        {
            return Err(GenerationError::InvalidFragment);
        }
        let binding = c::AssignedRemoteBinding {
            line: c::AssignedIdentity::from_text(line.line_id.as_str()),
            local_host: c::AssignedIdentity::from_text(local.host_id.as_str()),
            local_boot: c::AssignedIdentity::from_text(local.boot_id.as_str()),
            peer_host: c::AssignedIdentity::from_text(peer.host_id.as_str()),
            peer_boot: c::AssignedIdentity::from_text(peer.boot_id.as_str()),
        };
        let digest = c::admitted_line_commitment(line);
        let shared = (binding, session.session, session.session_contract, digest);
        if let Some(prior) = shared_sessions
            .iter()
            .find(|prior| prior.0.line == binding.line)
        {
            if *prior != shared {
                return Err(GenerationError::InvalidFragment);
            }
        } else {
            shared_sessions.push(shared);
            result.bindings.push(binding);
        }
        result.lines.push(c::AssignedLineForeRequirement::new(
            fore,
            binding,
            session.session,
            session.session_contract,
            digest,
        ));
    }
    if used[..sessions.len()].contains(&false) {
        return Err(GenerationError::InvalidFragment);
    }
    result.fore.sort_by_key(|fore| fore.endpoint());
    Ok(result)
}
