//! Verification of remote Mask route identities, witnesses, and finite Lines.

use super::*;

impl RemoteOwnerMaskRouteSeal {
    /// Apply this bound to every Show frame before emission.
    pub fn validate_return_payload(&self, bytes: usize) -> Result<(), RemoteOwnerMaskRouteError> {
        if bytes == 0 || bytes > self.return_line.binding.limits.maximum_payload_bytes as usize {
            return Err(RemoteOwnerMaskRouteError::ReturnExceedsLine);
        }
        if self
            .selected_fore_byte_capacity(
                &self.planned_mask.mask.show_output.front_port_id,
                conduit_core::PortDirection::Output,
            )
            .is_some_and(|limit| bytes > limit)
        {
            return Err(RemoteOwnerMaskRouteError::ReturnExceedsFore);
        }
        Ok(())
    }

    /// Bound a typed action independently from the Show carried on its own Line.
    pub fn validate_interaction_payload(
        &self,
        bytes: usize,
    ) -> Result<(), RemoteOwnerMaskRouteError> {
        let line = self
            .interaction_line
            .as_ref()
            .ok_or(RemoteOwnerMaskRouteError::MissingOrInvalidLine)?;
        if bytes == 0 || bytes > line.binding.limits.maximum_payload_bytes as usize {
            return Err(RemoteOwnerMaskRouteError::ReturnExceedsLine);
        }
        if self
            .selected_fore_byte_capacity(
                &self.planned_mask.mask.interaction_output.front_port_id,
                conduit_core::PortDirection::Output,
            )
            .is_none_or(|limit| bytes > limit)
        {
            return Err(RemoteOwnerMaskRouteError::ReturnExceedsFore);
        }
        Ok(())
    }

    pub(super) fn selected_fore_byte_capacity(
        &self,
        port: &conduit_core::PortId,
        direction: conduit_core::PortDirection,
    ) -> Option<usize> {
        self.planned_mask
            .plan
            .fragments
            .iter()
            .flat_map(|fragment| fragment.fore_ports.iter())
            .find(|fore| fore.front_port_id == *port && fore.direction == direction)
            .and_then(|fore| {
                fore.selected_line
                    .as_ref()
                    .map(|_| fore.byte_capacity as usize)
            })
    }

    /// Existing unbound routes remain provisional. A selected route must
    /// match every Line recorded in its seal, including a typed interaction
    /// return when that channel has been admitted.
    pub(super) fn verify_selected_fore_lines(&self) -> Result<(), RemoteOwnerMaskRouteError> {
        let fores = || {
            self.planned_mask
                .plan
                .fragments
                .iter()
                .flat_map(|fragment| fragment.fore_ports.iter())
        };
        if fores().all(|fore| fore.selected_line.is_none()) {
            return if self.interaction_line.is_none() {
                Ok(())
            } else {
                Err(RemoteOwnerMaskRouteError::InvalidSeal)
            };
        }
        let exact = |port: &conduit_core::PortId,
                     direction: conduit_core::PortDirection,
                     line: &AdmittedLine| {
            let mut matching = fores().filter(|fore| {
                fore.front_port_id == *port
                    && fore.direction == direction
                    && fore.track == conduit_core::ConnectionTrack::Payload
            });
            matches!(matching.next(), Some(fore) if fore.selected_line.as_ref() == Some(line))
                && matching.next().is_none()
        };
        if !exact(
            &self.planned_mask.mask.face_input.front_port_id,
            conduit_core::PortDirection::Input,
            &self.face_line,
        ) || !exact(
            &self.planned_mask.mask.show_output.front_port_id,
            conduit_core::PortDirection::Output,
            &self.return_line,
        ) || match &self.interaction_line {
            Some(line) => !exact(
                &self.planned_mask.mask.interaction_output.front_port_id,
                conduit_core::PortDirection::Output,
                line,
            ),
            None => fores().any(|fore| {
                fore.front_port_id == self.planned_mask.mask.interaction_output.front_port_id
                    && fore.direction == conduit_core::PortDirection::Output
                    && fore.selected_line.is_some()
            }),
        } {
            return Err(RemoteOwnerMaskRouteError::InvalidSeal);
        }
        Ok(())
    }

    pub(super) fn bind_identity(&self) -> Result<PlanId, RemoteOwnerMaskRouteError> {
        let basis = (
            "conduit.presentation/remote-owner-mask-route@1",
            &self.body_id,
            self.workload_revision,
            &self.face_id,
            self.face_revision,
            &self.face_basis,
            &self.owner_host,
            &self.mask_host,
            &self.planned_mask,
            &self.face_line,
            &self.return_line,
        );
        let bytes = match &self.interaction_line {
            Some(interaction_line) => serde_json::to_vec(&(
                "conduit.presentation/remote-owner-mask-route-with-interaction@1",
                &basis,
                interaction_line,
            )),
            None => serde_json::to_vec(&basis),
        }
        .map_err(|_| RemoteOwnerMaskRouteError::InvalidSeal)?;
        if bytes.len() > MAX_REMOTE_ROUTE_SEAL_BYTES {
            return Err(RemoteOwnerMaskRouteError::SealCapacityExceeded);
        }
        let digest = Sha256::digest(&bytes);
        Ok(PlanId::from(format!(
            "plan/remote-owner-mask/{}",
            hex(&digest)
        )))
    }
}

pub(super) fn validate_hosts(
    session: &BodyLifecycleSession,
    owner: &HostAdvertisement,
    remote: &HostAdvertisement,
) -> Result<(), RemoteOwnerMaskRouteError> {
    validate_host_pair(owner, remote)?;
    let present = session.evidence().membership.parts.iter().any(|part| {
        part.current.as_ref().is_some_and(|current| {
            current.host_id == remote.host_id
                && current.boot_id == remote.boot_id
                && current.offer_generation == remote.offer_generation
        })
    });
    if !present {
        return Err(RemoteOwnerMaskRouteError::WrongOrMissingPart);
    }
    Ok(())
}

pub(super) fn validate_workload_basis(
    session: &BodyLifecycleSession,
    face: &Presentation,
) -> Result<(), RemoteOwnerMaskRouteError> {
    match (
        &session.evidence().body.state,
        session.realization(),
        face.basis.wake_id.as_ref(),
    ) {
        (BodyState::Lulled, None, None) => Ok(()),
        (BodyState::Lulled, Some(_), _) => {
            Err(RemoteOwnerMaskRouteError::WorkloadRealizationPresent)
        }
        (BodyState::Awake { wake_id }, Some(realization), Some(face_wake))
            if wake_id == face_wake && realization.wake.wake_id == *wake_id =>
        {
            Ok(())
        }
        _ => Err(RemoteOwnerMaskRouteError::WrongFaceBasis),
    }
}

fn validate_host_pair(
    owner: &HostAdvertisement,
    remote: &HostAdvertisement,
) -> Result<(), RemoteOwnerMaskRouteError> {
    if owner.protocol_version != PROTOCOL_VERSION
        || remote.protocol_version != PROTOCOL_VERSION
        || owner.host_id == remote.host_id
        || owner.host_id.as_str().is_empty()
        || owner.boot_id.as_str().is_empty()
        || remote.host_id.as_str().is_empty()
        || remote.boot_id.as_str().is_empty()
        || owner.offer_generation.0 == 0
        || remote.offer_generation.0 == 0
    {
        return Err(RemoteOwnerMaskRouteError::InvalidHost);
    }
    Ok(())
}

pub(super) fn validate_mask_basis(
    host: &RemoteMaskHostBasis,
    planned: &PlannedMaskPlot,
) -> Result<(), RemoteOwnerMaskRouteError> {
    if !conduit_core::verify_plan(&planned.plan)
        || PlannedMaskPlot::admit(&planned.mask, &planned.plan).is_err()
        || planned.plan.fragments.is_empty()
        || planned.plan.fragments.iter().any(|fragment| {
            fragment.host_id != host.host_id
                || fragment.boot_id != host.boot_id
                || fragment.offer_generation != host.offer_generation
        })
    {
        return Err(RemoteOwnerMaskRouteError::InvalidSeal);
    }
    Ok(())
}

pub(super) fn validate_line(
    line: &LineOffer,
    source: &RemoteMaskHostBasis,
    sink: &RemoteMaskHostBasis,
) -> Result<(), RemoteOwnerMaskRouteError> {
    if !line.validate_sign_identity() {
        return Err(RemoteOwnerMaskRouteError::MissingOrInvalidLine);
    }
    if line.availability.availability != LineAvailability::Ready {
        return Err(RemoteOwnerMaskRouteError::LineUnavailable);
    }
    validate_admitted_line(&line.admitted_line(), source, sink)
}

pub(super) fn validate_admitted_line(
    line: &AdmittedLine,
    source: &RemoteMaskHostBasis,
    sink: &RemoteMaskHostBasis,
) -> Result<(), RemoteOwnerMaskRouteError> {
    let binding = &line.binding;
    if line.line_id.as_str().is_empty()
        || binding.binding_id.as_str().is_empty()
        || binding.base.as_str().is_empty()
        || binding.base.as_str() == conduit_core::LOCAL_BASE_IMPLEMENTATION_ID
        || binding.base_instance_id.as_str().is_empty()
        || matches!(binding.authority, LinkAuthorityReference::ProcessOwned)
        || matches!(binding.credential, LinkCredentialReference::None)
        || binding.source.endpoint_id.as_str().is_empty()
        || binding.sink.endpoint_id.as_str().is_empty()
        || binding.source.host_id != source.host_id
        || binding.source.boot_id != source.boot_id
        || binding.sink.host_id != sink.host_id
        || binding.sink.boot_id != sink.boot_id
        || binding.limits.maximum_in_flight_items == 0
        || binding.limits.maximum_payload_bytes == 0
        || binding.limits.maximum_frame_bytes == 0
        || binding.limits.maximum_buffered_bytes < binding.limits.maximum_payload_bytes
        || line.contract.traffic_shape != LineTrafficShape::Message
        || line.contract.scope == LineScope::Process
        || line.contract.ordering != LineOrdering::Ordered
        || line.contract.reliability != LineReliability::Reliable
    {
        return Err(RemoteOwnerMaskRouteError::MissingOrInvalidLine);
    }
    Ok(())
}

pub(super) fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}
