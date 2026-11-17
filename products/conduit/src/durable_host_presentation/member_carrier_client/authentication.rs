//! Prepare local continuity custody before entering the original timed exchange.
use super::*;
pub(super) struct Authenticated {
    pub(super) line: SecureWebSocketClientLine,
    pub(super) anchor: Instant,
    pub(super) deadline: Instant,
}
pub(super) fn connect(
    state: &Path,
    credential: &MembershipCredential,
    actual: &conduit_core::HostAdvertisement,
    encounter: &crate::durable_host::routed_join::MemberEncounter,
) -> Result<Authenticated, String> {
    use crate::durable_host::routed_join::{current_member, MEMBER_ROUTE_SCHEMA};
    let started = Instant::now();
    let candidate = &encounter.candidate;
    if encounter.schema != MEMBER_ROUTE_SCHEMA
        || encounter.body_id != credential.body_id
        || encounter.part_id != credential.part_id
        || encounter.host_id != credential.host_id
        || actual.host_id != credential.host_id
        || actual.boot_id != credential.boot_id
        || candidate.line_family != conduit_body::RendezvousLineFamily::AuthenticatedTlsStream
        || candidate.expires_at_millis <= crate::durable_host::current_time_millis()?
        || !(1..=30_000).contains(&candidate.attempt_timeout_millis)
        || !(1..=2).contains(&candidate.maximum_attempts)
    {
        return Err("current-member encounter differs from credential and actual Host/Boot".into());
    }
    let deadline = started + Duration::from_secs(60);
    // Full local equipment/credential validation precedes any network lease.
    let signer =
        crate::durable_host::membership::prepare_member_carrier_signer(state, credential, actual)?;
    let authentication_deadline = Instant::now()
        + Duration::from_millis(u64::from(candidate.attempt_timeout_millis.min(3_000)));
    let mut line = super::connect(
        candidate,
        Duration::from_millis(u64::from(candidate.attempt_timeout_millis.min(3_000))),
    )?;
    line.replace_io_deadline(authentication_deadline.min(deadline))
        .map_err(|e| format!("member authentication deadline:{e:?}"))?;
    line.set_read_timeout(Some(
        deadline
            .saturating_duration_since(Instant::now())
            .min(Duration::from_secs(2)),
    ))
    .map_err(|e| format!("member carrier timeout:{e:?}"))?;
    let anchor = Instant::now();
    current_member::resume_prepared(credential, actual, encounter, &mut line, signer)?;
    Ok(Authenticated {
        line,
        anchor,
        deadline,
    })
}
