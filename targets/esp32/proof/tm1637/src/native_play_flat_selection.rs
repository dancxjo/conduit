//! Exact checked pair selection and its numeric preparation witness.
use super::*;

// Keep the concrete affine issuance/checking waist out of the generic planner
// callback. Its owner brand is preserved; no Source/Plan borrow escapes.
#[inline(never)]
pub(super) fn prepare_linked_selection<'owner, 'root>(
    owners: n::ReadyNetworkPairContractView<'owner, '_, Provider, Source>,
    scope: &conduit_planner::borrowed_local::SourcePlannedLocalScope<'_>,
    issuer: &'root mut RootIssuer,
    pins: (&str, &str, &str),
    stage: &mut u16,
) -> Result<
    (
        n::PreparedNetworkPairSelection<'owner, Provider, Source>,
        n::PreparedNetworkKernelParts,
        crate::native_authority::LinkedNetworkPreparation<'root>,
    ),
    (),
> {
    *stage = 1051;
    let tables = n::NetworkControlFlatDirectTables::prepare(scope.verified()).map_err(|_| ())?;
    *stage = 1052;
    let roles = n::PreparedNetworkKernelParts::check_flat_direct(&tables).map_err(|_| ())?;
    let view = scope.verified().view();
    // The pair checks Credentials and Join only. Keep this
    // independent actual attachment implementation preflight.
    *stage = 1053;
    let sign = view
        .placements
        .iter()
        .nth(usize::from(roles.attachment_sign.0))
        .ok_or(())?;
    if sign.capability_id != CAP
        || sign.implementation_id != IMPLEMENTATION
        || sign.artifact_id != ARTIFACT
        || sign.execution_profile_id != conduit_net::NETWORK_ATTACHMENT_LOCAL_PROFILE
        || sign.host_calls.len() != 1
        || sign.host_calls.iter().next().ok_or(())?.contract_id
            != conduit_net::NETWORK_ATTACHMENT_SIGN_HOST_CALL
    {
        return Err(());
    }
    *stage = 1054;
    let credential = view
        .placements
        .iter()
        .nth(usize::from(roles.credentials.0))
        .ok_or(())?
        .placement_id;
    let join = view
        .placements
        .iter()
        .nth(usize::from(roles.join.0))
        .ok_or(())?
        .placement_id;
    *stage = 1055;
    let root = issuer.prepare_linked_network(scope, pins)?;
    *stage = 1056;
    let credential_scope =
        root.expected_scope(owners.credential_identity(), Operation::Credentials)?;
    *stage = 1057;
    let join_scope = root.expected_scope(owners.join_identity(), Operation::Join)?;
    *stage = 1058;
    let selected = owners
        .prepare_flat_selection(credential_scope, join_scope, &tables, credential, join)
        .map_err(|_| ())?;
    *stage = 1059;
    let parts = n::PreparedNetworkKernelParts::from_flat_direct(tables).map_err(|_| ())?;
    Ok((selected, parts, root))
}
