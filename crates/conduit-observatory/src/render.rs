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
    let _ = writeln!(output, "bases {}", report.bases.len());
    for base in &report.bases {
        let _ = writeln!(
            output,
            "base id={} kind={} host={} boot={} state={:?} capacity_units={}",
            base.base_id.as_str(),
            base.kind_id.as_str(),
            base.host_id.as_str(),
            base.boot_id.as_str(),
            base.state,
            base.capacity_units
        );
    }
    let _ = writeln!(output, "lines {}", report.lines.len());
    for line in &report.lines {
        let binding = &line.offer.binding;
        let _ = writeln!(
            output,
            "line id={} binding={} source_host={} source_boot={} source_endpoint={} sink_host={} sink_boot={} sink_endpoint={} base={:?} base_instance={} state={:?} availability={:?} shape={:?} duplex={:?} ordering={:?} reliability={:?} continuation={:?} security={:?} in_flight_limit={} payload_limit={} buffered_limit={} frame_limit={} authority={:?}",
            line.offer.line_id.as_str(),
            binding.binding_id.as_str(),
            binding.source.host_id.as_str(),
            binding.source.boot_id.as_str(),
            binding.source.endpoint_id.as_str(),
            binding.sink.host_id.as_str(),
            binding.sink.boot_id.as_str(),
            binding.sink.endpoint_id.as_str(),
            binding.base,
            binding.base_instance_id.as_str(),
            line.state,
            line.offer.availability.availability,
            line.offer.contract.traffic_shape,
            line.offer.contract.duplex,
            line.offer.contract.ordering,
            line.offer.contract.reliability,
            line.offer.contract.continuation,
            line.offer.contract.security,
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
            "plan id={} source_document={} checked_form={} expanded_form={} fragments={} placements={} connections={} execution_regions={}",
            plan.plan_id.as_str(),
            plan.source_document_id.as_str(),
            plan.checked_form_id.as_str(),
            plan.expanded_form_id.as_str(),
            plan.fragment_count,
            plan.placement_count,
            plan.connection_count,
            plan.execution_region_count
        );
    }
    let _ = writeln!(
        output,
        "execution_regions {}",
        report.execution_regions.len()
    );
    for region in &report.execution_regions {
        let _ = writeln!(
            output,
            "execution_region plan={} fragment={} region={} admitted_placements={:?} profile={} scheduling={:?} lane_count={} lane_pool={} lane_class={} lane_units={} lane_base={} runtime_memory_bytes={} timer_slots={} cord_items={} cord_bytes={} sign_items={} sign_bytes={} preemption_required={} isolation_required={}",
            region.plan_id.as_str(), region.fragment_id.as_str(), region.region_id.as_str(),
            region.admitted_placements, region.execution_profile_id.as_str(), region.scheduling,
            region.lane_count, region.lane_resource.pool_id.as_str(),
            region.lane_resource.class_id.as_str(), region.lane_resource.units,
            region.lane_base_id.as_str(), region.requirements.runtime_memory_bytes,
            region.requirements.timer_slots, region.requirements.cord_item_capacity,
            region.requirements.cord_byte_capacity, region.requirements.mandatory_sign_items,
            region.requirements.mandatory_sign_bytes, region.preemption_required,
            region.isolation_required
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
            "placement plan={} placement={} host={} boot={} capability={} kind={} contract={} execution_profile={} implementation={} artifact={} host_operations={:?} resources={:?} authority={:?}",
            placement.plan_id.as_str(),
            placement.placement_id.as_str(),
            placement.host_id.as_str(),
            placement.boot_id.as_str(),
            placement.capability_id.as_str(),
            placement.kind_id.as_str(),
            placement.kind_contract_revision.as_str(),
            placement.execution_profile_id.as_str(),
            placement.implementation_id.as_str(),
            placement.artifact_id.as_str(),
            placement.host_operations,
            placement.resources,
            placement.authority
        );
    }
    let _ = writeln!(output, "connections {}", report.connections.len());
    for connection in &report.connections {
        let _ = writeln!(
            output,
            "connection plan={} connection={} source={} sink={} value_kind={} selected_line={:?} admitted_lines={} queue_items={} queue_bytes={}",
            connection.plan_id.as_str(),
            connection.connection_id.as_str(),
            connection.source_placement_id.as_str(),
            connection.sink_placement_id.as_str(),
            connection.value_kind.as_str(),
            connection.selected_line,
            connection.admitted_lines.len(),
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
    let _ = writeln!(output, "sign {}", report.signs.len());
    for sign in &report.signs {
        let _ = writeln!(
            output,
            "sign id={} history={} active_play={} presentation={} host={} boot={} plan={} placement={} connection={} kind={:?}",
            sign.sign_id.as_str(),
            if sign.historical { "historical" } else { "current" },
            sign.active_play_id.as_ref().map(ActivePlayId::as_str).unwrap_or("none"),
            sign.presentation_id.as_ref().map(PresentationId::as_str).unwrap_or("none"),
            sign.host_id.as_str(),
            sign.boot_id.as_str(),
            sign.plan_id.as_ref().map(PlanId::as_str).unwrap_or("none"),
            sign.placement_id.as_ref().map(PlacementId::as_str).unwrap_or("none"),
            sign.connection_id.as_ref().map(ConnectionId::as_str).unwrap_or("none"),
            sign.kind
        );
    }
    let _ = writeln!(
        output,
        "boot provenance [sealed] {}",
        report.sealed_boot_provenance.len()
    );
    for provenance in &report.sealed_boot_provenance {
        let _ = writeln!(
            output,
            "sealed boot host={} boot={} firmware={} adapter={} version={} revision={} image={} build={} profile={} inclusion_paths={} memory_regions={} runtime_arena_bytes={} boot_artifacts={} initial_plan_artifact={} recovery_plan_artifact={} framebuffers={} proof_class={:?}",
            provenance.host_id.as_str(),
            provenance.boot_id.as_str(),
            provenance.firmware_environment,
            provenance.adapter_name,
            provenance.adapter_version,
            provenance.adapter_revision,
            provenance.image_id.as_str(),
            provenance.build_id.as_str(),
            provenance
                .image_build_trace
                .as_ref()
                .map_or("none", |trace| trace.profile_id.as_str()),
            provenance
                .image_build_trace
                .as_ref()
                .map_or(0, |trace| trace.inclusions.len()),
            provenance.memory_map.normalized_region_count,
            provenance.memory_map.runtime_arena_bytes,
            provenance.boot_artifacts.len(),
            provenance
                .initial_plan_artifact_id
                .as_ref()
                .map_or("none", conduit_core::ArtifactId::as_str),
            provenance
                .recovery_plan_artifact_id
                .as_ref()
                .map_or("none", conduit_core::ArtifactId::as_str),
            provenance.framebuffers.len(),
            provenance.proof_class,
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
