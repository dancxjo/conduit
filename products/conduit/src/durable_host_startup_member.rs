//! Original joined custody and typed member startup authentication.
use super::*;
/// Read the independently selected owner pin, matched to original joined custody.
pub(crate) fn current_joined_owner_pin(
    state_dir: &Path,
) -> Result<conduit_body::FederationPeer, String> {
    membership::selected_owner_pin(state_dir)
}

/// Sign only the typed, independently pinned original startup encounter.
pub(crate) fn sign_startup_member_challenge(
    state_dir: &Path,
    credential: &conduit_body::MembershipCredential,
    actual: &conduit_core::HostAdvertisement,
    challenge: &conduit_std_host::body_execution::StartupOwnerChallenge,
    now: u64,
) -> Result<[u8; 64], String> {
    let owner = membership::selected_owner_pin(state_dir)?;
    challenge.validate(&owner, now)?;
    if challenge.body_id != credential.body_id
        || challenge.part_id != credential.part_id
        || challenge.credential_id != credential.credential_id
    {
        return Err("startup challenge differs from original member scope".into());
    }
    let federation = &challenge.federation;
    let context = membership::MemberCarrierSigningContext {
        receiver: &owner,
        session: &federation.session_id,
        line: &federation.line_id,
        security: federation.line_security,
        maximum_frames: federation.maximum_frames,
        maximum_millis: 30_000,
    };
    membership::sign_member_carrier_challenge(
        state_dir, credential, actual, &context, federation, now,
    )
}

/// Read the original retained membership without issuing or replacing it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn export_member_contact(
    state: &Path,
    credential: &Path,
    bind: std::net::SocketAddr,
    url: &str,
    certificate: &Path,
    key: &Path,
    authorize: bool,
) -> Result<(), String> {
    membership::export_member_contact(state, credential, bind, url, certificate, key, authorize)
}
pub(crate) fn install_member_contact(
    state: &Path,
    descriptor: &Path,
    authorize: bool,
) -> Result<(), String> {
    membership::install_member_contact(state, descriptor, authorize)
}

pub(crate) fn current_joined_credential(
    state_dir: &Path,
) -> Result<conduit_body::MembershipCredential, String> {
    let installation = read_installation(&state_dir.join("installation.json"))?;
    membership::load_joined_credential(state_dir, &installation)
}
