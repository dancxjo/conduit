//! Post-Play projection of retained native routing evidence into shared Signs.
//! External Fore boundary routes remain native evidence: they are not assigned
//! a fictitious producing Gear or an internal planned Cord.
use super::{content_signs::StdSignLog, execution_observations::ObservationContext};
use conduit_core::{bind_sign, Observation, ObservationKind, PortDirection, RoutedContentEvidence};
use conduit_kernel::KernelEventKind;

pub(super) fn project(
    signs: &StdSignLog,
    context: &mut ObservationContext<'_>,
) -> Result<Vec<Observation>, String> {
    let Some(admission) = signs.admission() else {
        return Ok(Vec::new());
    };
    let mut observations = Vec::with_capacity(usize::from(admission.event_items));
    for binding in signs.bindings() {
        let event = signs
            .events()
            .find(|event| event.sequence == binding.event_sequence)
            .ok_or("routed content has no retained native event")?;
        if event.kind != KernelEventKind::ValueRouted
            || binding.byte_length > admission.maximum_digest_input_bytes
        {
            return Err("routed content differs from its admitted native event".into());
        }
        if context
            .lowered
            .fore_endpoints
            .iter()
            .any(|endpoint| endpoint.cord == binding.cord)
        {
            continue;
        }
        let connection_id = context
            .lowered
            .connection_for_cord(binding.cord)
            .ok_or("routed content Cord lacks exact planned identity")?;
        let placement_id = context
            .lowered
            .placement_for_node(event.node)
            .ok_or("routed content producer lacks exact placement")?;
        let port = event.port.ok_or("routed content event lacks output port")?;
        let output = context
            .lowered
            .port_identity(event.node, PortDirection::Output, port)
            .ok_or("routed content output lacks exact port identity")?;
        let connection = context
            .fragment
            .connections
            .iter()
            .find(|connection| &connection.connection_id == connection_id)
            .ok_or("routed content connection is outside its fragment")?;
        if &connection.source_placement_id != placement_id
            || connection.source_port_id != output.port_id
        {
            return Err("routed content producer differs from the planned Cord source".into());
        }
        let sign = bind_sign(
            &context.advertisement.host_id,
            &context.advertisement.boot_id,
            Some(&context.active_play.active_play_id),
            *context.next_sign_sequence,
        );
        *context.next_sign_sequence = context
            .next_sign_sequence
            .checked_add(1)
            .ok_or("routed content Sign sequence exhausted")?;
        // This is a routing event, not a Host request or a presentation effect.
        context
            .execution_identity
            .bind_sign(&sign, None, None, None)
            .map_err(|why| format!("bind routed content Sign: {why:?}"))?;
        observations.push(Observation {
            sign_id: sign.sign_id,
            active_play_id: Some(context.active_play.active_play_id.clone()),
            face_id: None,
            host_id: context.advertisement.host_id.clone(),
            boot_id: context.advertisement.boot_id.clone(),
            plan_id: Some(context.fragment.plan_id.clone()),
            placement_id: Some(placement_id.clone()),
            connection_id: Some(connection_id.clone()),
            kind: ObservationKind::RoutedContent {
                evidence: RoutedContentEvidence {
                    native_event_sequence: event.sequence,
                    output_port: output.port_id.clone(),
                    byte_length: binding.byte_length,
                    digest_domain: admission.digest_domain.into(),
                    digest_scheme: admission.digest_scheme.into(),
                    digest: binding.digest,
                },
            },
        });
    }
    Ok(observations)
}
