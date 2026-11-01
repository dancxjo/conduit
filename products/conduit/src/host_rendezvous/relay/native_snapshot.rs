use super::*;
/// Existing reviewed relay connector, now carrying canonical retained-Part
/// Return instead of the running-host Spawn exchange. No fresh invitation.
#[cfg(unix)]
pub(crate) fn connect_native_snapshot(
    state: &Path,
    descriptor: &Path,
    verified: &crate::durable_host_control::native_snapshot::VerifiedSelection,
    workspace: crate::host_rendezvous::DocumentWorkspace,
) -> Result<
    (
        conduit_body::MembershipCredential,
        conduit_face::OwnerFaceSnapshotResponse,
    ),
    String,
> {
    let current = crate::durable_host_control::current(state)?.advertisement;
    let part = crate::durable_host::RetainedPart::open(state, current)?;
    let client = crate::host_rendezvous::NativeReturn::new(part)?;
    let now = relay_now()?;
    let (_document, candidate, role, binding, policy) = load_endpoint_descriptor(descriptor, now)?;
    validate_snapshot_candidate(&candidate, role, Role::Initiator, &binding, verified)?;
    let (carrier, _journal) =
        establish_relay_line(_document.address, &candidate, role, &binding, policy, now)?;
    let line = crate::host_rendezvous::ProtectedDocumentLine::new(
        carrier,
        policy.idle_timeout_millis,
        Some(workspace),
    )?;
    crate::host_rendezvous::native_snapshot(client, verified, line)
}
#[cfg(unix)]
pub(crate) fn serve_native_snapshot(
    state: &Path,
    descriptor: &Path,
    verified: &crate::durable_host_control::native_snapshot::VerifiedSelection,
    workspace: crate::host_rendezvous::DocumentWorkspace,
) -> Result<crate::durable_host::native_snapshot_encounter::NativeSnapshotEvidence, String> {
    let now = relay_now()?;
    let (document, candidate, role, binding, policy) = load_endpoint_descriptor(descriptor, now)?;
    validate_snapshot_candidate(&candidate, role, Role::Responder, &binding, verified)?;
    let (carrier, _journal) =
        establish_relay_line(document.address, &candidate, role, &binding, policy, now)?;
    let line = crate::host_rendezvous::ProtectedDocumentLine::new(
        carrier,
        policy.idle_timeout_millis,
        Some(workspace),
    )?;
    crate::durable_host::native_snapshot_encounter::serve(state, verified, line)
}
fn relay_now() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "relay clock precedes epoch")?
        .as_millis()
        .try_into()
        .map_err(|_| "relay clock overflow".into())
}
#[cfg(unix)]
fn validate_snapshot_candidate(
    candidate: &RelayCandidateDescriptor,
    role: Role,
    expected_role: Role,
    binding: &SessionBinding,
    verified: &crate::durable_host_control::native_snapshot::VerifiedSelection,
) -> Result<(), String> {
    let route = verified.route();
    let id = &candidate.identity;
    if role != expected_role
        || binding != verified.binding()
        || route.line_family != RendezvousLineFamily::AuthenticatedConduitLine
        || route.candidate_id != id.route_id
        || route.reachability != id.relay_locator
        || route.authentication.server_identity != id.relay_server_identity
        || route.authentication.transport_binding_sha256 != id.certificate_binding_sha256
        || route.expires_at_millis != id.expires_at_millis
        || route.maximum_attempts != candidate.bounds.maximum_attempts
        || route.attempt_timeout_millis != candidate.bounds.attempt_timeout_millis
    {
        return Err("native snapshot relay differs from verified retained route or session".into());
    }
    Ok(())
}
