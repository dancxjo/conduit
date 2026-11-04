//! USB carriage for ordinary invited admission, through the live owner Core.
//! No private provision is sent, no membership row is fabricated, no Play starts.
use super::{bounded_read, current_time_millis, membership_receipt_custody};
use conduit_std_host::pico_spawn::{PicoSpawnAdmissionBasis, PicoSpawnSocket};
use std::{path::Path, time::Duration};

pub(crate) fn admit_usb(
    port: &Path,
    basis: &Path,
    receipt_output: &Path,
    state: &Path,
    authorized: bool,
) -> Result<(), String> {
    if !authorized {
        return Err("USB admission requires --authorize-admission".into());
    }
    let basis: PicoSpawnAdmissionBasis = serde_json::from_slice(&bounded_read(basis, 8192)?)
        .map_err(|_| "invalid public USB admission basis")?;
    basis
        .validate(current_time_millis()?)
        .map_err(|e| format!("USB admission basis: {e:?}"))?;
    // Select the actual current owner before touching the candidate device.
    if crate::durable_host_control::carrier_receiver(state)? != basis.owner {
        return Err("live admission owner differs from the Seed's independent pin".into());
    }
    // Empty reserved output after failure is not an admission receipt.
    let mut receipt_file = membership_receipt_custody::reserve(receipt_output)?;
    let timeout = Duration::from_secs(5);
    let mut socket = PicoSpawnSocket::open(port.to_str().ok_or("USB path is not UTF-8")?)
        .map_err(|e| format!("open USB admission carrier: {e:?}"))?;
    let observed = socket
        .request_join(&basis.spore_id, &basis.image_id, &basis.invitation, timeout)
        .map_err(|e| format!("observe actual Seed proof: {e:?}"))?;
    let request = basis
        .request(&observed, current_time_millis()?)
        .map_err(|e| format!("validate actual Seed proof: {e:?}"))?;
    let receipt = crate::durable_host_control::admit_owned_request(state, request)
        .map_err(|e| format!("owner admission (a control timeout leaves outcome unknown): {e}"))?;
    let signed =
        crate::durable_host_control::attest_admission_receipt(state, receipt, basis.owner.clone())
            .map_err(|e| format!("membership committed; owner receipt attestation failed: {e}"))?;
    signed
        .verify(
            &basis.owner,
            &basis.member_verifying_key,
            &basis.invitation.body_id,
            &observed.advertisement,
        )
        .map_err(|e| format!("membership committed; signed receipt refused: {e:?}"))?;
    membership_receipt_custody::retain(&mut receipt_file, &signed)?;
    socket
        .deliver_admission_receipt(
            &signed,
            &basis.owner,
            &basis.member_verifying_key,
            &basis.invitation.body_id,
            &observed.advertisement,
            timeout,
        )
        .map_err(|e| {
            format!("membership committed and receipt saved; USB delivery uncertain: {e:?}")
        })?;
    println!("Owner committed membership; signed receipt saved to {} and sent over USB. Device acceptance and Play are not confirmed.", receipt_output.display());
    Ok(())
}
