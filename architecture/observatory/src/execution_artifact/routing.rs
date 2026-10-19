//! Exact carried kind and source context for Cord and output-Fore content.
use super::*;
use conduit_core::{ConnectionTrack, KindId, PortDirection, RoutedContentEvidence};

pub(super) fn evidence(kind: &ObservationKind) -> Option<&RoutedContentEvidence> {
    match kind {
        ObservationKind::RoutedContent { evidence }
        | ObservationKind::ForeOutputContent { evidence, .. } => Some(evidence),
        _ => None,
    }
}
fn validate_extent(track: ConnectionTrack, byte_length: u32, capacity: u32) -> Result<(), String> {
    if byte_length > capacity {
        return Err("routed content exceeds its admitted track extent".into());
    }
    if matches!(
        track,
        ConnectionTrack::NormalClose | ConnectionTrack::Quiescence
    ) && byte_length != 0
    {
        return Err("Empty control track content must have zero bytes".into());
    }
    Ok(())
}
pub(super) fn declared_kind(artifact: &SignArtifact) -> Result<&KindId, String> {
    let item = evidence(&artifact.sign.kind).ok_or("value artifact requires routed content")?;
    let plan = artifact
        .play
        .as_ref()
        .map(|p| &p.plan)
        .or(artifact.plan.as_ref())
        .ok_or("routed content lacks exact Plan context")?;
    if artifact.sign.active_play_id.is_none() || artifact.sign.face_id.is_some() {
        return Err("routed content Sign has invalid execution context".into());
    }
    let placement = artifact
        .sign
        .placement_id
        .as_ref()
        .ok_or("routed content lacks producer")?;
    let fragment = plan
        .fragments
        .iter()
        .find(|f| {
            f.host_id == artifact.sign.host_id
                && f.boot_id == artifact.sign.boot_id
                && f.placements.iter().any(|p| &p.placement_id == placement)
        })
        .ok_or("routed content producer belongs to a different Host or boot")?;
    let producer = fragment
        .placements
        .iter()
        .find(|p| &p.placement_id == placement)
        .unwrap();
    if !producer
        .outputs
        .iter()
        .any(|p| p.port_id == item.output_port)
    {
        return Err("routed content output port is not planned".into());
    }
    if [&item.digest_domain, &item.digest_scheme]
        .iter()
        .any(|s| s.is_empty() || s.len() > 192)
    {
        return Err("routed content digest identity exceeds its finite bound".into());
    }
    match &artifact.sign.kind {
        ObservationKind::RoutedContent { .. } => {
            let cord = artifact
                .sign
                .connection_id
                .as_ref()
                .ok_or("routed content lacks exact Cord")?;
            let connection = plan
                .fragments
                .iter()
                .flat_map(|f| &f.connections)
                .find(|c| &c.connection_id == cord)
                .ok_or("routed content Cord is not planned")?;
            if &connection.source_placement_id != placement
                || connection.source_port_id != item.output_port
            {
                return Err("routed content differs from its exact Cord source".into());
            }
            validate_extent(connection.track, item.byte_length, connection.byte_capacity)?;
            Ok(&connection.value_kind)
        }
        ObservationKind::ForeOutputContent {
            front_port_id,
            track,
            ..
        } => {
            if artifact.sign.connection_id.is_some() {
                return Err("output Fore content cannot invent a Core connection".into());
            }
            let mut matching = fragment.fore_ports.iter().filter(|p| {
                p.direction == PortDirection::Output
                    && &p.front_port_id == front_port_id
                    && p.track == *track
                    && &p.placement_id == placement
                    && p.gear_port_id == item.output_port
            });
            let fore = matching
                .next()
                .ok_or("routed output Fore is not sealed in its exact fragment")?;
            if matching.next().is_some() {
                return Err("routed output Fore context is ambiguous".into());
            }
            validate_extent(fore.track, item.byte_length, fore.byte_capacity)?;
            // VerifyPlan checks the producer's payload/abnormal/close semantics.
            // The sealed Fore carries this track kind, not always payload kind.
            Ok(&fore.value_kind)
        }
        _ => Err("unsupported routed content context".into()),
    }
}
