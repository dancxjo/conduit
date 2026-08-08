use alloc::format;
use alloc::string::String;
use conduit_core::{ActivePlayId, ConnectionId, PlacementId, PlanId, PresentationId};
use core::fmt::Write;

use crate::ObservatoryReport;

pub fn render_text_report(report: &ObservatoryReport) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "host observatory report");
    let _ = writeln!(output, "hosts {}", report.hosts.len());
    for host in &report.hosts {
        let _ = writeln!(
            output,
            "host id={} boot={} state={:?} profile={} generation={} capabilities={} planners={:?} resources={:?}",
            host.host_id.as_str(),
            host.boot_id.as_str(),
            host.state,
            host.profile.as_str(),
            host.offer_generation.0,
            host.capability_count,
            host.planner_capabilities,
            host.resources
        );
    }
    let _ = writeln!(output, "capabilities {}", report.capabilities.len());
    for capability in &report.capabilities {
        let _ = writeln!(
            output,
            "capability host={} boot={} capability={} kind={} contract={} execution_profile={} implementation={} input_ports={} output_ports={} host_operations={:?} resource_requirements={:?} authority_requirements={:?} active_limit={} queue_items={} queue_bytes={} freshness={:?} support={:?} availability={:?}",
            capability.host_id.as_str(),
            capability.boot_id.as_str(),
            capability.capability_id.as_str(),
            capability.kind_id.as_str(),
            capability.kind_contract_revision.as_str(),
            capability.execution_profile_id.as_str(),
            capability.implementation_id.as_str(),
            capability.inputs.len(),
            capability.outputs.len(),
            capability.host_operations,
            capability.resource_requirements,
            capability.authority_requirements,
            capability.limits.max_active_instances,
            capability.limits.max_queue_items,
            capability.limits.max_queue_bytes,
            capability.freshness,
            capability.support,
            capability.availability
        );
    }
    let _ = writeln!(output, "links {}", report.links.len());
    for link in &report.links {
        let binding = &link.binding;
        let _ = writeln!(
            output,
            "link id={} source_host={} source_boot={} source_endpoint={} sink_host={} sink_boot={} sink_endpoint={} provider={:?} provider_instance={} state={:?} availability={:?} in_flight_limit={} payload_limit={} buffered_limit={} frame_limit={} authority={:?}",
            binding.binding_id.as_str(),
            binding.source.host_id.as_str(),
            binding.source.boot_id.as_str(),
            binding.source.endpoint_id.as_str(),
            binding.sink.host_id.as_str(),
            binding.sink.boot_id.as_str(),
            binding.sink.endpoint_id.as_str(),
            binding.provider,
            binding.provider_instance_id.as_str(),
            link.state,
            binding.availability,
            binding.limits.maximum_in_flight_items,
            binding.limits.maximum_payload_bytes,
            binding.limits.maximum_buffered_bytes,
            binding.limits.maximum_frame_bytes,
            binding.authority
        );
    }
    let _ = writeln!(output, "plans {}", report.plans.len());
    for plan in &report.plans {
        let _ = writeln!(
            output,
            "plan id={} source_document={} checked_form={} expanded_form={} fragments={} placements={} connections={}",
            plan.plan_id.as_str(),
            plan.source_document_id.as_str(),
            plan.checked_form_id.as_str(),
            plan.expanded_form_id.as_str(),
            plan.fragment_count,
            plan.placement_count,
            plan.connection_count
        );
    }
    let _ = writeln!(output, "fragments {}", report.fragments.len());
    for fragment in &report.fragments {
        let _ = writeln!(
            output,
            "fragment plan={} fragment={} host={} boot={}",
            fragment.plan_id.as_str(),
            fragment.fragment_id.as_str(),
            fragment.host_id.as_str(),
            fragment.boot_id.as_str()
        );
    }
    let _ = writeln!(output, "placements {}", report.placements.len());
    for placement in &report.placements {
        let _ = writeln!(
            output,
            "placement plan={} placement={} host={} boot={} capability={} kind={} contract={} execution_profile={} implementation={} host_operations={:?} resources={:?} authority={:?}",
            placement.plan_id.as_str(),
            placement.placement_id.as_str(),
            placement.host_id.as_str(),
            placement.boot_id.as_str(),
            placement.capability_id.as_str(),
            placement.kind_id.as_str(),
            placement.kind_contract_revision.as_str(),
            placement.execution_profile_id.as_str(),
            placement.implementation_id.as_str(),
            placement.host_operations,
            placement.resources,
            placement.authority
        );
    }
    let _ = writeln!(output, "connections {}", report.connections.len());
    for connection in &report.connections {
        let _ = writeln!(
            output,
            "connection plan={} connection={} source={} sink={} value_kind={} provider={:?} link_binding={:?} queue_items={} queue_bytes={}",
            connection.plan_id.as_str(),
            connection.connection_id.as_str(),
            connection.source_placement_id.as_str(),
            connection.sink_placement_id.as_str(),
            connection.value_kind.as_str(),
            connection.provider,
            connection.link_binding,
            connection.item_capacity,
            connection.byte_capacity
        );
    }
    let _ = writeln!(output, "plays {}", report.plays.len());
    for play in &report.plays {
        let _ = writeln!(
            output,
            "play id={} plan={} host={} boot={} lifecycle={:?} terminal={:?} failure={} placements={} connections={}",
            play.active_play_id.as_str(),
            play.plan_id.as_str(),
            play.host_id.as_str(),
            play.boot_id.as_str(),
            play.lifecycle,
            play.terminal_disposition,
            play.failure_message.as_deref().unwrap_or("none"),
            play.placements.len(),
            play.connections.len()
        );
        for placement in &play.placements {
            let _ = writeln!(
                output,
                "play-placement play={} placement={} lifecycle={:?} terminal={:?} failure={}",
                play.active_play_id.as_str(),
                placement.placement_id.as_str(),
                placement.lifecycle,
                placement.terminal_disposition,
                placement.failure_message.as_deref().unwrap_or("none")
            );
        }
        for connection in &play.connections {
            let pressure = connection.pressure.as_ref().map_or_else(
                || "unknown".into(),
                |pressure| {
                    format!(
                        "in_flight={:?} buffered_bytes={:?} events={} last_sequence={:?}",
                        pressure.current_in_flight_items,
                        pressure.current_buffered_bytes,
                        pressure.pressure_events,
                        pressure.last_pressure_sequence
                    )
                },
            );
            let _ = writeln!(
                output,
                "play-connection play={} connection={} lifecycle={:?} terminal={:?} pressure={} failure={}",
                play.active_play_id.as_str(),
                connection.connection_id.as_str(),
                connection.lifecycle,
                connection.terminal_disposition,
                pressure,
                connection.failure_message.as_deref().unwrap_or("none")
            );
        }
    }
    let _ = writeln!(output, "evidence {}", report.evidence.len());
    for evidence in &report.evidence {
        let _ = writeln!(
            output,
            "evidence id={} active_play={} presentation={} host={} boot={} plan={} placement={} connection={} kind={:?}",
            evidence.evidence_id.as_str(),
            evidence.active_play_id.as_ref().map(ActivePlayId::as_str).unwrap_or("none"),
            evidence.presentation_id.as_ref().map(PresentationId::as_str).unwrap_or("none"),
            evidence.host_id.as_str(),
            evidence.boot_id.as_str(),
            evidence.plan_id.as_ref().map(PlanId::as_str).unwrap_or("none"),
            evidence.placement_id.as_ref().map(PlacementId::as_str).unwrap_or("none"),
            evidence.connection_id.as_ref().map(ConnectionId::as_str).unwrap_or("none"),
            evidence.kind
        );
    }
    let _ = writeln!(
        output,
        "retention bounded={} capacity={} retained={} visible_gaps={} explanation={}",
        report.retention.bounded,
        report.retention.item_capacity,
        report.retention.retained_items,
        report.retention.visible_gap_count,
        report.retention.explanation
    );
    output
}
