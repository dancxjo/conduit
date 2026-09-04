use conduit_body::{
    BodyBiographyEvidence, BodyId, BodyMembership, HostPresenceTable, MembershipCredential,
    MembershipCredentialId, PartId,
};
use conduit_core::{BootId, HostId, LinkBindingId, SignId};
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress, BrowserAdmissionSocket, BROWSER_ADMISSION_PROTOCOL,
};
use std::time::Instant;

#[allow(clippy::too_many_arguments)]
pub(super) fn record_explicit_leave(
    socket: &mut BrowserAdmissionSocket,
    presence: &mut HostPresenceTable,
    membership: &mut BodyMembership,
    biography: &mut BodyBiographyEvidence,
    credential: &MembershipCredential,
    session: &LinkBindingId,
    credential_id: MembershipCredentialId,
    body_id: BodyId,
    part_id: PartId,
    host_id: HostId,
    boot_id: BootId,
    sequence: u64,
    clock: Instant,
    await_fresh_return: bool,
) -> Result<bool, String> {
    if credential_id != credential.credential_id
        || body_id != credential.body_id
        || part_id != credential.part_id
        || host_id != credential.host_id
        || boot_id != credential.boot_id
    {
        socket
            .send(&BrowserAdmissionEgress::Refused {
                protocol: BROWSER_ADMISSION_PROTOCOL,
                code: "stale-membership-credential".into(),
            })
            .map_err(|error| format!("send leave refusal: {error:?}"))?;
        return Ok(false);
    }
    let prior_events = membership.events.len();
    presence
        .lose_session(
            membership,
            &credential.part_id,
            session,
            monotonic_millis(clock)?,
            SignId::from("sign/browser-admission-probe/explicit-leave"),
        )
        .map_err(|error| format!("record explicit leave: {error:?}"))?;
    let event = membership
        .events
        .get(prior_events)
        .ok_or("explicit leave did not append membership evidence")?;
    let biography_sequence = biography
        .records
        .last()
        .and_then(|record| record.sequence.checked_add(1))
        .ok_or("Body biography sequence exhausted")?;
    biography
        .append_membership_events(
            membership.clone(),
            &[(event.change_id.clone(), biography_sequence)],
        )
        .map_err(|error| format!("append leave biography: {error:?}"))?;
    socket
        .send(&BrowserAdmissionEgress::BiographyEvidence {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            evidence: Box::new(biography.clone()),
        })
        .map_err(|error| format!("send leave biography evidence: {error:?}"))?;
    println!(
        "left sequence={sequence} part={}",
        credential.part_id.as_str()
    );
    if await_fresh_return {
        socket
            .close()
            .map_err(|error| format!("close explicitly left browser: {error:?}"))?;
    }
    Ok(await_fresh_return)
}

fn monotonic_millis(clock: Instant) -> Result<u64, String> {
    u64::try_from(clock.elapsed().as_millis()).map_err(|_| "leave presence clock overflowed".into())
}
