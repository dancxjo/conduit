//! Explicit pinned-Fore receiving of signed installed-table references.
use super::*;
use crate::{
    check_host_preparation_delegation, AttributedHostCommand, HostPreparationDelegation,
    NativePreparationContext,
};
use conduit_core as c;
/// Independently installed data and receiver pins. This cannot authenticate a
/// command, admit a resource or manufacture native possession or Start.
pub struct AssignedBodyInstalledPlainResourcesProfile<'a> {
    pub preparation: AssignedBodyPreparationProfile<'a>,
    pub resources: InstalledPlainResourceTable,
}
pub fn receive_delegated_body_fragment_with_installed_plain_resources<'a, const PORTS: usize>(
    installed: Option<&HostPreparationDelegation>,
    context: NativePreparationContext<'_>,
    command: AttributedHostCommand<'_, 'a>,
    profile: AssignedBodyInstalledPlainResourcesProfile<'_>,
) -> Result<(DelegatedBodyFragment<'a>, ApprovedBodyForePorts<PORTS>), DelegatedFragmentRefusal> {
    check_host_preparation_delegation(installed, context, &command)
        .map_err(DelegatedFragmentRefusal::Policy)?;
    let (metadata, projection) = decode_body_fragment_document(command.payload())
        .map_err(DelegatedFragmentRefusal::Document)?;
    finish(
        metadata,
        projection,
        installed.expect("checked installed delegation"),
        context,
        profile,
    )
}
pub(super) fn finish<'a, const PORTS: usize>(
    metadata: BodyFragmentApprovalMetadata<'a>,
    projection: &'a [u8],
    policy: &HostPreparationDelegation,
    context: NativePreparationContext<'_>,
    profile: AssignedBodyInstalledPlainResourcesProfile<'_>,
) -> Result<(DelegatedBodyFragment<'a>, ApprovedBodyForePorts<PORTS>), DelegatedFragmentRefusal> {
    use DelegatedFragmentRefusal as E;
    let fore = profile.preparation.fore.ok_or(E::ForeApproval)?;
    if fore.len() > usize::from(profile.preparation.maxima.counts[9]) {
        return Err(E::ForeApproval);
    }
    let approved = ApprovedBodyForePorts::copy_pinned(fore).ok_or(E::ForeApproval)?;
    let metadata = profile.resources.resolve(metadata).map_err(E::Document)?;
    let fragment = receiving::finish_resolved_checked_with(
        metadata,
        projection,
        context.actual,
        context.member.body_id.as_str(),
        policy.maximum_assignment_bytes,
        policy.maximum_runtime_state_bytes,
        profile.preparation,
        |prepared| {
            c::decode_assigned_plan_with_fore(
                projection,
                prepared.maxima,
                prepared.requirements,
                prepared.computations,
                fore,
            )
        },
    )?;
    Ok((fragment, approved))
}
