//! Exact connection occurrence and admitted-line verification over borrowed Plan data.
use super::{
    AdmittedLine, LinkAuthorityReference, LinkCredentialReference, Plan, PlannedConnection,
    PlannedGear, LOCAL_BASE_IMPLEMENTATION_ID,
};
use alloc::vec::Vec;

pub(super) fn verify_plan_connections(plan: &Plan) -> bool {
    let count = plan
        .fragments
        .iter()
        .map(|fragment| fragment.connections.len())
        .sum();
    let mut connections = Vec::with_capacity(count);
    connections.extend(
        plan.fragments
            .iter()
            .flat_map(|fragment| &fragment.connections),
    );
    // Sort only borrowed scratch; the Plan and its canonical order stay intact.
    connections.sort_unstable_by(|left, right| left.connection_id.cmp(&right.connection_id));
    let mut remaining = connections.as_slice();
    while let Some(connection) = remaining.first().copied() {
        let count = remaining
            .partition_point(|candidate| candidate.connection_id == connection.connection_id);
        let (occurrences, rest) = remaining.split_at(count);
        remaining = rest;
        if occurrences.iter().any(|candidate| *candidate != connection) {
            return false;
        }
        let mut source = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .filter(|placement| placement.placement_id == connection.source_placement_id);
        let mut sink = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .filter(|placement| placement.placement_id == connection.sink_placement_id);
        let (Some(source_placement), Some(sink_placement)) = (source.next(), sink.next()) else {
            return false;
        };
        if source.next().is_some() || sink.next().is_some() {
            return false;
        }
        let source = source_placement;
        let sink = sink_placement;
        if source.host_id == sink.host_id {
            if occurrences.len() != 1
                || connection.selected_line.is_some()
                || !connection.admitted_lines.is_empty()
            {
                return false;
            }
        } else {
            let Some(selected) = &connection.selected_line else {
                return false;
            };
            if occurrences.len() != 2
                || connection.admitted_lines.is_empty()
                || !connection.permits_line(selected)
                || connection
                    .admitted_lines
                    .iter()
                    .enumerate()
                    .any(|(index, candidate)| {
                        connection.admitted_lines[..index].iter().any(|prior| {
                            prior.line_id == candidate.line_id
                                || prior.binding.binding_id == candidate.binding.binding_id
                        })
                    })
                || connection
                    .admitted_lines
                    .iter()
                    .any(|candidate| invalid_admitted_line(candidate, source, sink, connection))
            {
                return false;
            }
        }
    }
    true
}

fn invalid_admitted_line(
    candidate: &AdmittedLine,
    source: &PlannedGear,
    sink: &PlannedGear,
    connection: &PlannedConnection,
) -> bool {
    let binding = &candidate.binding;
    candidate.line_id.as_str().is_empty()
        || binding.binding_id.as_str().is_empty()
        || candidate.line_id.as_str() == binding.binding_id.as_str()
        || candidate.line_id.as_str() == binding.base_instance_id.as_str()
        || candidate.line_id.as_str() == binding.source.endpoint_id.as_str()
        || candidate.line_id.as_str() == binding.sink.endpoint_id.as_str()
        || binding.binding_id.as_str() == binding.base_instance_id.as_str()
        || binding.base.as_str() == LOCAL_BASE_IMPLEMENTATION_ID
        || binding.base_instance_id.as_str().is_empty()
        || binding.source.host_id != source.host_id
        || binding.source.boot_id != source.boot_id
        || binding.source.endpoint_id.as_str().is_empty()
        || binding.sink.host_id != sink.host_id
        || binding.sink.boot_id != sink.boot_id
        || binding.sink.endpoint_id.as_str().is_empty()
        || binding.source.endpoint_id == binding.sink.endpoint_id
        || binding.limits.maximum_in_flight_items < connection.item_capacity
        || binding.limits.maximum_payload_bytes < connection.byte_capacity
        || binding.limits.maximum_buffered_bytes < connection.byte_capacity
        || binding.limits.maximum_frame_bytes < binding.limits.maximum_payload_bytes
        || matches!(
            &binding.credential,
            LinkCredentialReference::Opaque(reference) if reference.as_str().is_empty()
        )
        || matches!(
            &binding.authority,
            LinkAuthorityReference::Grant(grant_id) if grant_id.as_str().is_empty()
        )
}
