//! Owner-authorized USB carriage of existing canonical Part return.
use super::{bounded_read, membership_receipt_custody};
use conduit_body::{MemberReturnImageAcknowledgement, MemberReturnProofDocument};
use conduit_std_host::pico_spawn::{PicoMemberReturnBasis, ProvisioningCarrier};
use std::{path::Path, time::Duration};
pub(crate) fn return_usb(
    port: &Path,
    basis: &Path,
    receipt_output: &Path,
    state: &Path,
    authorized: bool,
    carrier: ProvisioningCarrier,
) -> Result<(), String> {
    if !authorized {
        return Err("USB member return requires --authorize-return".into());
    }
    let basis: PicoMemberReturnBasis = serde_json::from_slice(&bounded_read(basis, 8192)?)
        .map_err(|_| "invalid public member-return basis")?;
    basis
        .validate()
        .map_err(|e| format!("USB member-return basis: {e:?}"))?;
    if crate::durable_host_control::carrier_receiver(state)? != basis.owner {
        return Err("live return owner differs from the Seed's independent pin".into());
    }
    let mut custody = membership_receipt_custody::reserve(receipt_output)?;
    let timeout = Duration::from_secs(5);
    let mut socket = carrier
        .open(port.to_str().ok_or("USB path is not UTF-8")?)
        .map_err(|e| format!("open member-return carrier: {e:?}"))?;
    let observed = socket
        .request_member_return(&basis, timeout)
        .map_err(|e| format!("observe actual returning member: {e:?}"))?;
    let challenge = crate::durable_host_control::begin_image_member_return(
        state,
        observed.presentation.clone(),
        observed.images.clone(),
        observed.advertisement.clone(),
    )?;
    let proof = socket
        .prove_member_return(&basis, &observed.advertisement, &challenge, timeout)
        .map_err(|e| format!("actual member continuity proof: {e:?}"))?;
    let credential = crate::durable_host_control::complete_member_return(
        state,
        MemberReturnProofDocument::encode(proof),
    )
    .map_err(|e| format!("owner return commit (control timeout leaves outcome unknown): {e}"))?;
    let ack =
        MemberReturnImageAcknowledgement::new(credential, observed.presentation.projection.clone());
    let signed = crate::durable_host_control::attest_image_return_acknowledgement(
        state,
        ack,
        observed.images,
        basis.owner.clone(),
    )
    .map_err(|e| format!("member returned; owner acknowledgement attestation failed: {e}"))?;
    signed
        .verify_image_return(
            &basis.owner,
            &basis.member_verifying_key,
            &basis.credential,
            &observed.presentation.projection,
        )
        .map_err(|e| format!("member returned; signed acknowledgement refused: {e:?}"))?;
    membership_receipt_custody::retain(&mut custody, &signed)?;
    socket
        .deliver_member_return(&signed, &basis, &observed.presentation.projection, timeout)
        .map_err(|e| {
            format!("return committed and receipt saved; USB delivery uncertain: {e:?}")
        })?;
    println!("Owner committed the existing Part on the observed Boot; signed acknowledgement saved to {} and sent over USB. Device acceptance and Play are not confirmed.",receipt_output.display());
    Ok(())
}
