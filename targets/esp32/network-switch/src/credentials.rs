//! Root preparation of the SDK-owned representation of one private submission.
//! This code neither touches a controller nor establishes radio authority.
extern crate alloc;
use alloc::string::ToString;
use conduit_net::NetworkJoinRequest;
use esp_radio::wifi::{sta::StationConfig, AuthenticationMethod};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StationConfigurationRefusal {
    Ssid,
    Credential,
}

/// Prepare the attended WPA2-personal profile before Play. The profile accepts
/// one UTF-8 SSID of 1–32 bytes and an 8–63 byte UTF-8 passphrase. Embedded NULs
/// are refused because the SDK's fixed C fields would change their meaning.
/// Open networks, raw hexadecimal PSKs and enterprise authentication require a
/// separately admitted profile; this function does not infer them from input.
///
/// The password String allocates here, never in the selected invocation. Root
/// owns its lifetime and must include SDK configuration memory in admission.
/// The returned SDK type must not be logged or exposed in Face.
pub fn prepare_station_configuration(
    request: NetworkJoinRequest<'_>,
) -> Result<StationConfig, StationConfigurationRefusal> {
    let (ssid, password) = validate(request)?;
    Ok(StationConfig::default()
        .with_ssid(ssid)
        .with_password(password.to_string())
        .with_auth_method(AuthenticationMethod::Wpa2Personal)
        .with_failure_retry_cnt(0))
}

pub(crate) fn validate(
    request: NetworkJoinRequest<'_>,
) -> Result<(&str, &str), StationConfigurationRefusal> {
    use StationConfigurationRefusal as E;
    if !(1..=32).contains(&request.ssid.len()) || request.ssid.contains(&0) {
        return Err(E::Ssid);
    }
    let ssid = core::str::from_utf8(request.ssid).map_err(|_| E::Ssid)?;
    if !(8..=63).contains(&request.credential.len()) || request.credential.contains(&0) {
        return Err(E::Credential);
    }
    let password = core::str::from_utf8(request.credential).map_err(|_| E::Credential)?;
    Ok((ssid, password))
}
