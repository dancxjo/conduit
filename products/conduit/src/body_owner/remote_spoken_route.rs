//! Reproduce a known checked spoken Mask against the admitted peer's actual offer.
//! Disclosed grants are planning inputs; only the peer's private Host can use them.
use conduit_core::{ConnectionTrack, HostAdvertisement, LineOffer, PortDirection};
use conduit_face::{Face, PlannedMaskPlot};
use conduit_std_host::direct_spoken_template_mask::RemoteSpokenTemplateSelection;

pub(super) fn plan(
    peer: &HostAdvertisement,
    owner: &HostAdvertisement,
    face: &Face,
    face_line: &LineOffer,
    show_line: &LineOffer,
    selection: &RemoteSpokenTemplateSelection,
) -> Result<PlannedMaskPlot, String> {
    let canonical = selection.validate_for_offer(face, peer)?;
    let mask = &canonical.mask;
    let choices = [
        (
            PortDirection::Input,
            &mask.face_input.front_port_id,
            face_line,
        ),
        (
            PortDirection::Output,
            &mask.show_output.front_port_id,
            show_line,
        ),
    ]
    .map(
        |(direction, port, line)| conduit_planner::ExternalForeLineChoice {
            host_id: peer.host_id.clone(),
            boot_id: peer.boot_id.clone(),
            direction,
            front_port_id: port.clone(),
            track: ConnectionTrack::Payload,
            peer_host_id: owner.host_id.clone(),
            peer_boot_id: owner.boot_id.clone(),
            line_id: line.line_id.clone(),
        },
    );
    let plan = conduit_planner::bind_external_fore_lines(
        &canonical.plan,
        &choices,
        &[face_line.clone(), show_line.clone()],
        &["conduit.base/authenticated-tls-websocket@1".into()],
    )
    .map_err(|error| format!("remote-spoken-lines-refused:{error:?}"))?;
    PlannedMaskPlot::admit(mask, &plan)
        .map_err(|error| format!("remote-spoken-mask-refused:{error:?}"))
}
