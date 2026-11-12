//! Original online attribution, exact child containment and installed serial pins.
use super::*;
use sha2::{Digest, Sha256};
pub(super) fn bounds(
    b: ForeCarrierBounds,
    backing: usize,
    lines: usize,
) -> Result<(), ForeCarrierRefusal> {
    if b.maximum_lines == 0
        || b.maximum_lines > 16
        || lines == 0
        || lines > usize::from(b.maximum_lines)
        || b.maximum_frame_bytes == 0
        || b.maximum_frame_bytes > crate::transport::LIMIT as u32
        || b.maximum_payload_bytes == 0
        || b.maximum_payload_bytes > b.maximum_frame_bytes
        || b.maximum_buffered_bytes != backing
        || backing < b.maximum_frame_bytes as usize
        || backing > 16 * crate::transport::LIMIT
        || b.maximum_in_flight_items == 0
        || b.maximum_frames == 0
        || b.maximum_frames > 1_000_000
    {
        return Err(ForeCarrierRefusal::Bounds);
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
pub(super) fn prepare<'c, 'p, C: Carrier, const N: usize, const F: usize, const PORTS: usize>(
    workspace: &b::HostCommandInput<N>,
    context: b::NativePreparationContext<'_>,
    config: &Configuration,
    selection: Option<NativeForeCarrierSelection<'c, 'p, C>>,
    approved: &b::DelegatedBodyFragment<'_>,
    fore: &b::ApprovedBodyForePorts<PORTS>,
    profile: b::AssignedBodyLineForePreparationProfile<'_>,
) -> Result<PreparedForeCarrier<'c, 'p, C>, ForeCarrierRefusal> {
    use ForeCarrierRefusal as E;
    let selection = selection.ok_or(E::MissingSelection)?;
    let installed = &selection.installed;
    bounds(
        installed.bounds,
        selection.backing.len(),
        installed.profile.lines.len(),
    )?;
    if installed.image != config.image_id
        || installed.configuration != config.configuration_id
        || installed.configuration != context.actual.profile.as_str()
        || installed.peer != &context.bootstrap.owner
        || installed.profile.local != profile.line_fore.local
        || installed.profile.lines != profile.line_fore.lines
    {
        return Err(E::Scope);
    }
    check_lines(installed, context.actual)?;
    let command = workspace
        .reborrow(
            context.member,
            context.actual,
            &context.bootstrap.owner,
            b::HOST_PREPARED_WORKSET_CONTRACT,
            context.now_millis,
        )
        .map_err(|_| E::Attribution)?;
    let set = b::receive_delegated_body_workset::<F>(config.preparation, context, command)
        .map_err(|_| E::Workset)?;
    if set.fragment_count() != F {
        return Err(E::Workset);
    }
    let mut found = None;
    for index in 0..F {
        let bytes = set.document(index).ok_or(E::Workset)?;
        let (metadata, projection) =
            b::decode_body_fragment_document(bytes).map_err(|_| E::Fragment)?;
        if metadata == *approved.metadata() && projection == approved.projection() {
            if found.replace(index).is_some() {
                return Err(E::Scope);
            }
        }
    }
    let index = found.ok_or(E::Scope)?;
    let (checked, pins) = set
        .receive_fragment_with_line_fore::<PORTS>(index, config.preparation, context, profile)
        .map_err(|_| E::Fragment)?;
    if checked.view() != approved.view() || pins.as_slice() != fore.as_slice() {
        return Err(E::Scope);
    }
    let child_sha: [u8; 32] = Sha256::digest(set.document(index).ok_or(E::Workset)?).into();
    let scope = fore_carrier_scope_commitment(
        &child_sha,
        installed.image,
        installed.configuration,
        installed.bounds,
    );
    let custody = set
        .into_retained_stamp()
        .map_err(|_| E::Attribution)?
        .custody_identity();
    Ok(PreparedForeCarrier {
        selection,
        scope,
        custody,
    })
}
fn check_lines(
    installed: &Installation<'_>,
    actual: &c::HostAdvertisement,
) -> Result<(), ForeCarrierRefusal> {
    use ForeCarrierRefusal as E;
    for (pin, line) in installed.profile.lines.iter().zip(installed.lines) {
        let (local, peer) = match pin.fore().direction() {
            c::AssignedForeDirection::Input => (&line.binding.sink, &line.binding.source),
            c::AssignedForeDirection::Output => (&line.binding.source, &line.binding.sink),
        };
        let limits = line.binding.limits;
        if local.host_id != actual.host_id
            || local.boot_id != actual.boot_id
            || peer.host_id != installed.peer.host_id
            || peer.boot_id != installed.peer.boot_id
            || local.endpoint_id.as_str().is_empty()
            || peer.endpoint_id.as_str().is_empty()
            || line.line_id.as_str().is_empty()
            || line.binding.binding_id.as_str().is_empty()
            || line.binding.base.as_str().is_empty()
            || line.binding.base_instance_id.as_str().is_empty()
            || line.contract.security != c::LineSecurity::PhysicalPossession
            || line.contract.scope != c::LineScope::PointToPoint
            || line.contract.traffic_shape != c::LineTrafficShape::Message
            || line.contract.ordering != c::LineOrdering::Ordered
            || line.contract.reliability != c::LineReliability::Reliable
            || limits.maximum_payload_bytes == 0
            || limits.maximum_payload_bytes > installed.bounds.maximum_payload_bytes
            || limits.maximum_frame_bytes < limits.maximum_payload_bytes
            || limits.maximum_frame_bytes > installed.bounds.maximum_frame_bytes
            || limits.maximum_buffered_bytes < limits.maximum_payload_bytes
            || limits.maximum_buffered_bytes as usize > installed.bounds.maximum_buffered_bytes
            || limits.maximum_in_flight_items == 0
            || limits.maximum_in_flight_items > installed.bounds.maximum_in_flight_items
        {
            return Err(E::Bounds);
        }
        let binding = c::AssignedRemoteBinding {
            line: c::AssignedIdentity::from_text(line.line_id.as_str()),
            local_host: c::AssignedIdentity::from_text(local.host_id.as_str()),
            local_boot: c::AssignedIdentity::from_text(local.boot_id.as_str()),
            peer_host: c::AssignedIdentity::from_text(peer.host_id.as_str()),
            peer_boot: c::AssignedIdentity::from_text(peer.boot_id.as_str()),
        };
        let expected = c::AssignedLineForeRequirement::new(
            *pin.fore(),
            binding,
            c::AssignedIdentity::from_text(installed.session),
            c::AssignedIdentity::from_text(installed.session_contract),
            c::admitted_line_commitment(line),
        );
        if expected.payload() != pin.payload() {
            return Err(E::Scope);
        }
    }
    Ok(())
}
