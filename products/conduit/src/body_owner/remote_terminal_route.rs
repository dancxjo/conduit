//! Owner-side planning of the ordinary terminal Mask on an admitted peer.
//! The authenticated carrier supplies actual directional Lines before this call.
use conduit_core::{ConnectionTrack, HostAdvertisement, LineOffer, PortDirection};
use conduit_face::PlannedMaskPlot;
use conduit_std_host::terminal_mask_execution::HostedTerminalMaskExecution;

pub(super) fn plan(
    peer: &HostAdvertisement,
    owner: &HostAdvertisement,
    face: &LineOffer,
    show: &LineOffer,
    interaction: &LineOffer,
) -> Result<PlannedMaskPlot, String> {
    let execution = HostedTerminalMaskExecution::new_attached(peer)
        .map_err(|error| format!("remote-terminal-plan-refused:{error:?}"))?;
    let mask = &execution.planned_mask().mask;
    let choices = [
        (PortDirection::Input, &mask.face_input.front_port_id, face),
        (PortDirection::Output, &mask.show_output.front_port_id, show),
        (
            PortDirection::Output,
            &mask.interaction_output.front_port_id,
            interaction,
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
        &execution.planned_mask().plan,
        &choices,
        &[face.clone(), show.clone(), interaction.clone()],
        &["conduit.base/authenticated-tls-websocket@1".into()],
    )
    .map_err(|error| format!("remote-terminal-lines-refused:{error:?}"))?;
    PlannedMaskPlot::admit(mask, &plan)
        .map_err(|error| format!("remote-terminal-mask-refused:{error:?}"))
}
