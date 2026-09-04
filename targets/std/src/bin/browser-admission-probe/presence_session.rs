//! Shared responses for the bounded browser presence session.

use conduit_body::HostPresenceRefusal;
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress, BrowserAdmissionSocket, BROWSER_ADMISSION_PROTOCOL,
};

pub(super) fn send_presence_accepted(
    socket: &mut BrowserAdmissionSocket,
    sequence: u64,
    expires_at_millis: u64,
) -> Result<(), String> {
    socket
        .send(&BrowserAdmissionEgress::PresenceAccepted {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            sequence,
            renew_after_millis: super::PRESENCE_RENEW_AFTER_MILLIS,
            expires_at_millis,
        })
        .map_err(|error| format!("send presence acceptance: {error:?}"))
}

pub(super) fn presence_refusal_code(refusal: HostPresenceRefusal) -> &'static str {
    match refusal {
        HostPresenceRefusal::WrongBody => "wrong-body",
        HostPresenceRefusal::UnknownPart => "unknown-part",
        HostPresenceRefusal::RevokedPart => "revoked-part",
        HostPresenceRefusal::HostUnavailable => "host-unavailable",
        HostPresenceRefusal::WrongHost => "wrong-host",
        HostPresenceRefusal::StaleBoot => "stale-boot",
        HostPresenceRefusal::StaleOfferGeneration => "stale-offer-generation",
        HostPresenceRefusal::StaleMembershipProof => "stale-membership-proof",
        HostPresenceRefusal::WrongSession => "wrong-session",
        HostPresenceRefusal::StaleSequence => "stale-sequence",
        HostPresenceRefusal::ClockRegressed => "clock-regressed",
        HostPresenceRefusal::InvalidClock => "invalid-clock",
        HostPresenceRefusal::LeaseDurationZero => "lease-duration-zero",
        HostPresenceRefusal::LeaseDurationTooLong => "lease-duration-too-long",
        HostPresenceRefusal::LeaseDeadlineOverflow => "lease-deadline-overflow",
        HostPresenceRefusal::LeaseStillCurrent => "lease-still-current",
        HostPresenceRefusal::PresenceCapacityExhausted => "presence-capacity-exhausted",
        HostPresenceRefusal::RevisionOverflow => "revision-overflow",
        HostPresenceRefusal::MalformedState => "malformed-presence-state",
        HostPresenceRefusal::EmptyIdentity => "empty-identity",
        HostPresenceRefusal::IdentityTooLong => "identity-too-long",
        HostPresenceRefusal::Membership(_) => "membership-refused",
    }
}
