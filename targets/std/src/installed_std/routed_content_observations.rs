//! Post-Play projection of retained native routing evidence into shared Signs.
//! Output Fore routes retain their sealed Fore context and actual producer.
//! They are never assigned a fictitious internal planned Cord.
use super::{content_signs::StdSignLog, execution_observations::ObservationContext};
use conduit_core::{
    bind_sign, ConnectionId, Observation, ObservationKind, PlacementId, PortDirection,
    RoutedContentEvidence,
};
use conduit_kernel::{KernelEventKind, SignQuery};

pub(super) fn identity_capacity(signs: &StdSignLog) -> Result<usize, String> {
    signs.admission().map_or(Ok(0), |admission| {
        usize::from(admission.event_items)
            .checked_add(1)
            .ok_or_else(|| "routing Sign/gap identity capacity overflow".into())
    })
}

pub(super) fn project(
    signs: &StdSignLog,
    context: &mut ObservationContext<'_>,
) -> Result<Vec<Observation>, String> {
    let Some(admission) = signs.admission() else {
        return Ok(Vec::new());
    };
    let mut observations = Vec::with_capacity(identity_capacity(signs)?);
    if let Some(gap) = signs.retention_gap() {
        observations.push(bind(
            context,
            None,
            None,
            ObservationKind::ExecutionEventGap {
                first_sequence: gap.first_sequence,
                last_sequence: gap.last_sequence,
                entries: gap.entries,
            },
        )?);
    }
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
        let placement_id = context
            .lowered
            .placement_for_node(event.node)
            .ok_or("routed content producer lacks exact placement")?;
        let port = event.port.ok_or("routed content event lacks output port")?;
        let output = context
            .lowered
            .port_identity(event.node, PortDirection::Output, port)
            .ok_or("routed content output lacks exact port identity")?;
        let producer = placement_id.clone();
        let mut endpoints = context
            .lowered
            .fore_endpoints
            .iter()
            .filter(|endpoint| endpoint.cord == binding.cord);
        let endpoint = endpoints.next();
        if endpoints.next().is_some() {
            return Err("routing endpoint is ambiguous".into());
        }
        let (cord, fore) = if let Some(endpoint) = endpoint {
            if endpoint.direction != PortDirection::Output {
                return Err("captured route has no producing output Fore context".into());
            }
            let mut matching = context.fragment.fore_ports.iter().filter(|p| {
                p.direction == PortDirection::Output
                    && p.front_port_id == endpoint.front_port_id
                    && p.track == endpoint.track
                    && p.value_kind == endpoint.value_kind
                    && &p.placement_id == placement_id
                    && p.gear_port_id == output.port_id
            });
            let port = matching
                .next()
                .ok_or("captured output Fore is not sealed")?;
            if matching.next().is_some() || binding.byte_length > port.byte_capacity {
                return Err("captured output Fore context or extent is invalid".into());
            }
            (None, Some((port.front_port_id.clone(), port.track)))
        } else {
            let connection_id = context
                .lowered
                .connection_for_cord(binding.cord)
                .ok_or("routed content Cord lacks exact planned identity")?;
            let connection = context
                .fragment
                .connections
                .iter()
                .find(|c| &c.connection_id == connection_id)
                .ok_or("routed content connection is outside its fragment")?;
            if &connection.source_placement_id != placement_id
                || connection.source_port_id != output.port_id
                || binding.byte_length > connection.byte_capacity
            {
                return Err(
                    "routed content producer differs from planned Cord source or extent".into(),
                );
            }
            (Some(connection_id.clone()), None)
        };
        let evidence = RoutedContentEvidence {
            native_event_sequence: event.sequence,
            output_port: output.port_id.clone(),
            byte_length: binding.byte_length,
            digest_domain: admission.digest_domain.into(),
            digest_scheme: admission.digest_scheme.into(),
            digest: binding.digest,
        };
        let kind = match fore {
            Some((front_port_id, track)) => ObservationKind::ForeOutputContent {
                front_port_id,
                track,
                evidence,
            },
            None => ObservationKind::RoutedContent { evidence },
        };
        observations.push(bind(context, Some(producer), cord, kind)?);
    }
    Ok(observations)
}

fn bind(
    context: &mut ObservationContext<'_>,
    placement_id: Option<PlacementId>,
    connection_id: Option<ConnectionId>,
    kind: ObservationKind,
) -> Result<Observation, String> {
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
    // Retained native evidence is not a Host request or a presentation effect.
    context
        .execution_identity
        .bind_sign(&sign, None, None, None)
        .map_err(|why| format!("bind routed content Sign: {why:?}"))?;
    Ok(Observation {
        sign_id: sign.sign_id,
        active_play_id: Some(context.active_play.active_play_id.clone()),
        face_id: None,
        host_id: context.advertisement.host_id.clone(),
        boot_id: context.advertisement.boot_id.clone(),
        plan_id: Some(context.fragment.plan_id.clone()),
        placement_id,
        connection_id,
        kind,
    })
}

#[cfg(test)]
mod tests;
