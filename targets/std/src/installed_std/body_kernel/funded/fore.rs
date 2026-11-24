//! Preparation memory for the sealed local Fore routes.
use super::*;

pub(super) fn backing(
    bytes: &mut usize,
    request: &BodyRunRequest<'_>,
    interaction: Option<&BodyLiveForeQueue>,
) -> Result<(), String> {
    use conduit_plan_lowering::lowering::LoweredForePort;
    let mut outputs = 0usize;
    for port in request
        .plan
        .plots
        .iter()
        .flat_map(|p| &p.plan.fragments[0].fore_ports)
    {
        let mut descriptor = port
            .front_port_id
            .0
            .len()
            .checked_add(port.value_kind.0.len())
            .ok_or("Tick Fore identity overflow")?;
        if let Some(abnormal) = &port.abnormal_kind {
            add(&mut descriptor, abnormal.0.len())?;
        }
        if let Some(contract) = &port.value_contract {
            add(&mut descriptor, contract.value_kind.0.len())?;
        }
        if let Some(queue) = interaction.filter(|queue| queue.port() == port) {
            add(bytes, descriptor)?;
            add(bytes, size_of::<LoweredForePort>())?;
            add(bytes, size_of::<(usize, &LoweredForePort)>())?;
            add(bytes, size_of::<&LoweredForePort>())?;
            add(
                bytes,
                size_of::<super::super::super::external_fore_feeder::LiveForeFeeder>(),
            )?;
            add(
                bytes,
                size_of::<(conduit_kernel::RemoteEndpointId, conduit_kernel::CordId)>(),
            )?;
            add(
                bytes,
                queue
                    .face_interaction_backing_bytes()
                    .ok_or("Tick interaction backing overflow")?,
            )?;
            continue;
        }
        // ports + output slot each retain an independent descriptor clone.
        add(bytes, descriptor)?;
        add(bytes, descriptor)?;
        add(bytes, size_of::<LoweredForePort>())?;
        add(bytes, size_of::<fore_route::OutputSlots>())?;
        add(bytes, size_of::<(usize, &LoweredForePort)>())?;
        let count = usize::from(port.item_capacity);
        outputs = outputs
            .checked_add(count)
            .ok_or("Tick Fore slot overflow")?;
        add(bytes, array::<ExternalForeDelivery>(count)?)?;
        add(bytes, array::<ExternalForeDelivery>(count)?)?;
        let identity = size_of::<crate::fore_delivery::ExternalForeIdentity>()
            .checked_add(2 * size_of::<usize>())
            .and_then(|n| n.checked_add(port.front_port_id.0.len()))
            .and_then(|n| n.checked_add(port.value_kind.0.len()))
            .ok_or("Tick Fore identity overflow")?;
        // Shared identity may be counted conservatively per escaped slot.
        add(
            bytes,
            identity
                .checked_mul(count)
                .ok_or("Tick Fore identity backing overflow")?,
        )?;
        // Vec-to-Arc publication temporarily retains the source payload too.
        add(
            bytes,
            (port.byte_capacity as usize)
                .checked_mul(count)
                .ok_or("Tick Fore payload staging overflow")?,
        )?;
        add(
            bytes,
            crate::fore_delivery::payload_backing_bytes(port.byte_capacity as usize)
                .and_then(|size| size.checked_mul(count))
                .ok_or("Tick Fore payload backing overflow")?,
        )?;
    }
    let _ = outputs;
    add(
        bytes,
        array::<fore_route::AcknowledgedInput>(
            crate::installed_std::keyboard_input_completion::MAX_COMPLETED_INPUT_EVENTS
                .next_power_of_two(),
        )?,
    )?;
    Ok(())
}

pub(super) fn validate_interaction(
    request: &BodyRunRequest<'_>,
    queue: &BodyLiveForeQueue,
) -> Result<(), String> {
    let mut fore_parts = request.plan.plots.iter().filter(|part| {
        part.plan
            .fragments
            .iter()
            .any(|fragment| !fragment.fore_ports.is_empty())
    });
    if !fore_parts.next().is_some_and(|part| {
        queue.matches_plan(&part.plan, request.control)
            && part.plan.fragments[0].fore_ports.len() == 1
    }) || fore_parts.next().is_some()
        || queue.face_interaction_backing_bytes().is_none()
    {
        return Err("funded interaction requires the exact lone local FaceInteraction Fore".into());
    }
    Ok(())
}
