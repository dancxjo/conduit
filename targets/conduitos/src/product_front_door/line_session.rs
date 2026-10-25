//! Finite admitted Line session and its exact Body membership observations.
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    journey: &mut ProductJourney,
    front_door: &mut FrontDoor,
    presenter: &mut FrontDoorPresenter,
    face_workspace: &mut FaceWorkspace,
    display: &mut impl crate::display::PixelTarget,
    platform: &mut platform::RootPlatform<'_>,
    make: &MakeRecord,
) -> Result<(), &'static str> {
    let body_id = journey
        .projection()
        .body_id
        .ok_or("product-usb-line-body-absent")?;
    let peer_host_id = conduit_core::HostId::from(crate::product_usb_line::HARNESS_HOST_ID);
    let peer_boot_id = conduit_core::BootId::from(crate::product_usb_line::HARNESS_BOOT_ID);
    let peer_proof =
        conduit_body::MembershipProofId::bind("conduitos/product/reviewed-qemu-line-peer")
            .map_err(|_| "product-usb-line-membership-proof-invalid")?;
    let mut peer_part = None;
    platform.run_line(body_id.as_str(), |observation| {
        let platform::ConnectivityObservation {
            status,
            line_id,
            value,
        } = observation;
        match status {
            crate::front_door::ConnectivityStatus::PeerAttached => {
                peer_part = Some(
                    journey
                        .admit_line_peer(
                            peer_host_id.clone(),
                            peer_boot_id.clone(),
                            peer_proof.clone(),
                        )
                        .map_err(|error| error.as_str())?,
                );
            }
            crate::front_door::ConnectivityStatus::Lost => {
                journey
                    .observe_line_peer_offline(
                        peer_part
                            .as_ref()
                            .ok_or("product-usb-line-peer-not-admitted")?,
                        &peer_boot_id,
                    )
                    .map_err(|error| error.as_str())?;
            }
            _ => {}
        }
        front_door
            .observe_connectivity(crate::front_door::ConnectivityProjection {
                line_id: line_id.into(),
                status,
                value: value.map(Into::into),
                body_id: body_id.clone(),
            })
            .map_err(|error| error.as_str())?;
        let receipt = refresh(front_door, journey, presenter, face_workspace, display)?;
        emit_journey_sign(&journey.projection(), make, &receipt);
        Ok(())
    })?;
    Ok(())
}
