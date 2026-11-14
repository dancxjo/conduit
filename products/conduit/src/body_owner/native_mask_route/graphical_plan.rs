//! Bind an existing graphical Mask's exact external Fore to actual carrier Lines.
use conduit_core::{HostAdvertisement, LineOffer};
pub(super) fn plan(
    native_offer: &HostAdvertisement,
    owner: &HostAdvertisement,
    component_selection: Option<(
        conduit_face::ComponentMaskLook,
        conduit_face::TodoComponentRecipe,
    )>,
    face_line: &LineOffer,
    return_line: &LineOffer,
    interaction_line: Option<&LineOffer>,
) -> Result<conduit_face::PlannedMaskPlot, String> {
    let planned = conduit_conduitos_mask_offer::prepare_advertised_native_stage_from_offer(
        native_offer,
        component_selection.map(|(look, recipe)| (look.into(), recipe)),
    )
    .map_err(|error| format!("native-mask-plan-refused:{error:?}"))?
    .planned_mask;
    let mut choices = vec![
        (
            conduit_core::PortDirection::Input,
            &planned.mask.face_input.front_port_id,
            face_line,
        ),
        (
            conduit_core::PortDirection::Output,
            &planned.mask.show_output.front_port_id,
            return_line,
        ),
    ];
    if let Some(line) = interaction_line {
        choices.push((
            conduit_core::PortDirection::Output,
            &planned.mask.interaction_output.front_port_id,
            line,
        ));
    }
    let choices = choices
        .into_iter()
        .map(
            |(direction, port, line)| conduit_planner::ExternalForeLineChoice {
                host_id: native_offer.host_id.clone(),
                boot_id: native_offer.boot_id.clone(),
                direction,
                front_port_id: port.clone(),
                track: conduit_core::ConnectionTrack::Payload,
                peer_host_id: owner.host_id.clone(),
                peer_boot_id: owner.boot_id.clone(),
                line_id: line.line_id.clone(),
            },
        )
        .collect::<Vec<_>>();
    let mut lines = vec![face_line.clone(), return_line.clone()];
    if let Some(line) = interaction_line {
        lines.push(line.clone());
    }
    let mut bases = vec![
        face_line.binding.base.clone(),
        return_line.binding.base.clone(),
    ];
    if let Some(line) = interaction_line {
        bases.push(line.binding.base.clone());
    }
    bases.sort();
    bases.dedup();
    let plan = conduit_planner::bind_external_fore_lines(&planned.plan, &choices, &lines, &bases)
        .map_err(|error| format!("native-mask-lines-refused:{error:?}"))?;
    conduit_face::PlannedMaskPlot::admit(&planned.mask, &plan)
        .map_err(|error| format!("native-mask-fore-refused:{error:?}"))
}
