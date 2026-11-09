//! Opt-in trusted authoring Fore approval under explicit installed delegation.
use super::*;
use crate::{
    check_host_preparation_delegation, AttributedHostCommand, HostPreparationDelegation,
    NativePreparationContext,
};
use conduit_core as c;

/// Fixed preparation storage for checked authoring ABI pins; not ingress authority.
pub struct ApprovedBodyForePorts<const PORTS: usize> {
    pins: [c::AssignedForeRequirement; PORTS],
    count: usize,
}
impl<const PORTS: usize> ApprovedBodyForePorts<PORTS> {
    pub fn as_slice(&self) -> &[c::AssignedForeRequirement] {
        &self.pins[..self.count]
    }
    pub(super) fn copy_pinned(pins: &[c::AssignedForeRequirement]) -> Option<Self> {
        if pins.len() > PORTS {
            return None;
        }
        let empty = c::AssignedForeRequirement::new(
            0,
            0,
            c::AssignedForeDirection::Input,
            c::AssignedIdentity([0; 16]),
            [0; 32],
        );
        let mut result = Self {
            pins: [empty; PORTS],
            count: pins.len(),
        };
        result.pins[..pins.len()].copy_from_slice(pins);
        Some(result)
    }
}

/// Accept the exact Fore ABI supplied by the delegated full Source/Plan checker.
/// No general parser/type checker is installed on the receiver. Local maxima,
/// computation pins and hardware possession remain independently required.
/// This does not grant a caller ingress, a Line, native IO or permission to start.
/// Existing caller-pinned Fore contracts cannot be replaced through this entrance.
pub fn receive_delegated_body_fragment_with_approved_fore<'a, const PORTS: usize>(
    installed: Option<&HostPreparationDelegation>,
    context: NativePreparationContext<'_>,
    command: AttributedHostCommand<'_, 'a>,
    profile: AssignedBodyPreparationProfile<'_>,
) -> Result<(DelegatedBodyFragment<'a>, ApprovedBodyForePorts<PORTS>), DelegatedFragmentRefusal> {
    use DelegatedFragmentRefusal as E;
    check_host_preparation_delegation(installed, context, &command).map_err(E::Policy)?;
    if profile.fore.is_some() {
        return Err(E::ForeApproval);
    }
    let (metadata, projection) =
        decode_body_fragment_document(command.payload()).map_err(E::Document)?;
    finish_approved_fore(
        metadata,
        projection,
        installed.expect("checked delegation"),
        context,
        profile,
    )
}

pub(super) fn finish_approved_fore<'a, const PORTS: usize>(
    metadata: BodyFragmentApprovalMetadata<'a>,
    projection: &'a [u8],
    policy: &HostPreparationDelegation,
    context: NativePreparationContext<'_>,
    profile: AssignedBodyPreparationProfile<'_>,
) -> Result<(DelegatedBodyFragment<'a>, ApprovedBodyForePorts<PORTS>), DelegatedFragmentRefusal> {
    use DelegatedFragmentRefusal as E;
    if profile.fore.is_some() {
        return Err(E::ForeApproval);
    }
    let (records, tail) = metadata
        .fore_ports
        .as_chunks::<{ c::ASSIGNED_LOCAL_FORE_BYTES }>();
    if !tail.is_empty()
        || records.len() > PORTS
        || records.len() > usize::from(profile.maxima.counts[9])
    {
        return Err(E::ForeApproval);
    }
    let empty = c::AssignedForeRequirement::new(
        0,
        0,
        c::AssignedForeDirection::Input,
        c::AssignedIdentity([0; 16]),
        [0; 32],
    );
    let mut fore = [empty; PORTS];
    for (index, raw) in records.iter().enumerate() {
        let direction = match raw[4] {
            0 => c::AssignedForeDirection::Input,
            1 => c::AssignedForeDirection::Output,
            _ => return Err(E::ForeApproval),
        };
        fore[index] = c::AssignedForeRequirement::new(
            u16::from_le_bytes(raw[..2].try_into().map_err(|_| E::ForeApproval)?),
            u16::from_le_bytes(raw[2..4].try_into().map_err(|_| E::ForeApproval)?),
            direction,
            c::AssignedIdentity(raw[5..21].try_into().map_err(|_| E::ForeApproval)?),
            raw[21..].try_into().map_err(|_| E::ForeApproval)?,
        );
    }
    let fragment = receiving::finish_receive(
        metadata,
        projection,
        context.actual,
        context.member,
        policy,
        AssignedBodyPreparationProfile {
            maxima: profile.maxima,
            requirements: profile.requirements,
            computations: profile.computations,
            fore: Some(&fore[..records.len()]),
        },
    )?;
    Ok((
        fragment,
        ApprovedBodyForePorts {
            pins: fore,
            count: records.len(),
        },
    ))
}

/// Preserve independently installed Fore pins. This entrance cannot replace
/// them with descriptors supplied by the authoring Host.
pub(super) fn finish_pinned_fore<'a, const PORTS: usize>(
    metadata: BodyFragmentApprovalMetadata<'a>,
    projection: &'a [u8],
    policy: &HostPreparationDelegation,
    context: NativePreparationContext<'_>,
    profile: AssignedBodyPreparationProfile<'_>,
) -> Result<(DelegatedBodyFragment<'a>, ApprovedBodyForePorts<PORTS>), DelegatedFragmentRefusal> {
    use DelegatedFragmentRefusal as E;
    let installed = profile.fore.ok_or(E::ForeApproval)?;
    if installed.len() > PORTS || installed.len() > usize::from(profile.maxima.counts[9]) {
        return Err(E::ForeApproval);
    }
    let count = installed.len();
    let empty = c::AssignedForeRequirement::new(
        0,
        0,
        c::AssignedForeDirection::Input,
        c::AssignedIdentity([0; 16]),
        [0; 32],
    );
    let mut pins = [empty; PORTS];
    pins[..count].copy_from_slice(installed);
    // Checks every metadata descriptor against these pins, then independently
    // decodes the assigned projection with the same exact installed Fore ABI.
    let fragment = receiving::finish_receive(
        metadata,
        projection,
        context.actual,
        context.member,
        policy,
        profile,
    )?;
    Ok((fragment, ApprovedBodyForePorts { pins, count }))
}
