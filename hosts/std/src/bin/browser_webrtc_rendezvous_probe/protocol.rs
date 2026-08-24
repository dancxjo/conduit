use conduit_body::MembershipCredential;
use conduit_std_host::browser_admission::BrowserAdmissionIngress;

pub(super) fn exact_credential(
    expected: &MembershipCredential,
    credential_id: &conduit_body::MembershipCredentialId,
    body_id: &conduit_body::BodyId,
    part_id: &conduit_body::PartId,
    host_id: &conduit_core::HostId,
    boot_id: &conduit_core::BootId,
) -> Result<(), String> {
    if credential_id == &expected.credential_id
        && body_id == &expected.body_id
        && part_id == &expected.part_id
        && host_id == &expected.host_id
        && boot_id == &expected.boot_id
    {
        Ok(())
    } else {
        Err("stale membership credential".into())
    }
}

pub(super) fn frame_kind(frame: &BrowserAdmissionIngress) -> &'static str {
    match frame {
        BrowserAdmissionIngress::PresenceRenewal { .. } => "presence-renewal",
        BrowserAdmissionIngress::MediaResourceTruth { .. } => "media-resource-truth",
        BrowserAdmissionIngress::WebRtcGrantRequest { .. } => "web-rtc-grant-request",
        BrowserAdmissionIngress::WebRtcSignal { .. } => "web-rtc-signal",
        _ => "unexpected",
    }
}

pub(super) fn debug<T: core::fmt::Debug>(label: &'static str) -> impl FnOnce(T) -> String {
    move |error| format!("{label}: {error:?}")
}
