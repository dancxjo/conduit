//! Explicit schema-8 preparation with independently installed Line/Fore pins.
use super::*;
use crate::{
    check_host_preparation_delegation, AttributedHostCommand, HostPreparationDelegation,
    NativePreparationContext,
};
use conduit_core as c;

/// Opt-in representation profile, never ingress authority or carrier custody.
/// `preparation.fore` must be None: there is no legacy/schema-7 fallback.
/// Root supplies the independent Line profile from its original verified pins.
pub struct AssignedBodyLineForePreparationProfile<'a> {
    pub preparation: AssignedBodyPreparationProfile<'a>,
    pub line_fore: c::AssignedLineForeProfile<'a>,
}

/// Authenticate the original command and check schema 8 against independent
/// trusted pins. Returned ABI descriptors grant no Line, input, Start or session.
/// Metadata's existing 53-byte Fore inventory is the union of local pins and
/// `line.fore()` pins, sorted by numeric endpoint. Schema-8 record order remains
/// independent; the Core decoder checks exact records, duplicates and bindings.
pub fn receive_delegated_body_fragment_with_line_fore<'a, const PORTS: usize>(
    installed: Option<&HostPreparationDelegation>,
    context: NativePreparationContext<'_>,
    command: AttributedHostCommand<'_, 'a>,
    profile: AssignedBodyLineForePreparationProfile<'_>,
) -> Result<(DelegatedBodyFragment<'a>, ApprovedBodyForePorts<PORTS>), DelegatedFragmentRefusal> {
    use DelegatedFragmentRefusal as E;
    check_host_preparation_delegation(installed, context, &command).map_err(E::Policy)?;
    let policy = installed.expect("checked installed delegation");
    let (metadata, projection) =
        decode_body_fragment_document(command.payload()).map_err(E::Document)?;
    finish_line_fore(metadata, projection, policy, context, profile)
}

// Shared by the independently authenticated single-fragment and original
// complete-workset entrances. This helper issues no child attribution.
pub(super) fn finish_line_fore<'a, const PORTS: usize>(
    metadata: BodyFragmentApprovalMetadata<'a>,
    projection: &'a [u8],
    policy: &HostPreparationDelegation,
    context: NativePreparationContext<'_>,
    profile: AssignedBodyLineForePreparationProfile<'_>,
) -> Result<(DelegatedBodyFragment<'a>, ApprovedBodyForePorts<PORTS>), DelegatedFragmentRefusal> {
    use DelegatedFragmentRefusal as E;
    if profile.preparation.fore.is_some() {
        return Err(E::ForeApproval);
    }
    let count = profile
        .line_fore
        .local
        .len()
        .checked_add(profile.line_fore.lines.len())
        .ok_or(E::ForeApproval)?;
    if count > PORTS || count > usize::from(profile.preparation.maxima.counts[9]) {
        return Err(E::ForeApproval);
    }
    let empty = c::AssignedForeRequirement::new(
        0,
        0,
        c::AssignedForeDirection::Input,
        c::AssignedIdentity([0; 16]),
        [0; 32],
    );
    let mut pins = [empty; PORTS];
    for (pin, expected) in pins.iter_mut().zip(
        profile
            .line_fore
            .local
            .iter()
            .chain(profile.line_fore.lines.iter().map(|line| line.fore())),
    ) {
        *pin = *expected;
    }
    pins[..count].sort_unstable_by_key(|pin| pin.endpoint());
    let fragment = receiving::finish_checked_with(
        metadata,
        projection,
        context.actual,
        context.member.body_id.as_str(),
        policy.maximum_assignment_bytes,
        policy.maximum_runtime_state_bytes,
        AssignedBodyPreparationProfile {
            fore: Some(&pins[..count]),
            ..profile.preparation
        },
        |prepared| {
            c::decode_assigned_plan_with_line_fore(
                projection,
                prepared.maxima,
                prepared.requirements,
                prepared.computations,
                profile.line_fore,
            )
        },
    )?;
    let approved = ApprovedBodyForePorts::copy_pinned(&pins[..count]).ok_or(E::ForeApproval)?;
    Ok((fragment, approved))
}
