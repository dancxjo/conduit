//! Actual kernel observations projected by Patchbay's bounded debugger model.
//! Export happens after execution and never supplies or repairs causal records.
use conduit_body::BodyPlan;
use conduit_core::PortDirection;
use conduit_kernel::debug_observation::{DebugEventKind, DebugObservationRecord, DebugSubject};
use conduit_std_host::body_execution::BodyRunReport;
use patchbay_application::{DebuggerTimeline, DebuggerTimelineBinding};
use serde::Serialize;
use std::{error::Error, fs::OpenOptions, io::Write, path::Path};

const MAX_ARTIFACT_BYTES: usize = 4 * 1024 * 1024;
#[derive(Debug, Serialize)]
pub(super) struct DebuggerSummary {
    pub recorded_count: usize,
    pub retained_timeline_events: usize,
    pub collector_dropped_records: u64,
    pub timeline_evicted_events: u64,
    pub observer_refusal: Option<String>,
    pub combined_direction_port_bindings: usize,
    pub final_response_sequence: Option<u64>,
    pub upstream_trace_complete: bool,
    pub upstream_trace_scope: &'static str,
    pub full_cognition_causal_chain_verified: bool,
    pub upstream_missing_parent_sequences: Vec<u64>,
    pub upstream_reaches_situation_input: bool,
    pub upstream_cognition_kinds: Vec<String>,
    pub rendered_browser_verified: bool,
}
#[derive(Serialize)]
struct SubjectIdentity {
    visible_subject: String,
    runtime_subject: String,
    /// Full exact Plan/fragment/placement/Cord/Fore identities, never truncated.
    identities: Vec<String>,
    visible_alias: bool,
    combined_port_directions: bool,
}
#[derive(Serialize)]
struct TimelineArtifact<'a> {
    schema: &'static str,
    scope: &'static str,
    type_names: &'a [String],
    bindings: &'a [SubjectIdentity],
    timeline: &'a DebuggerTimeline,
    summary: &'a DebuggerSummary,
}
#[derive(Serialize)]
struct TraceArtifact<'a> {
    schema: &'static str,
    scope: &'static str,
    /// This contains only actual retained records in the parent-pointer chain.
    timeline: &'a DebuggerTimeline,
    missing_collector_parents: &'a [u64],
    trace_complete: bool,
    rendered_browser_verified: bool,
}

pub(super) fn export(
    output: &Path,
    plan: &BodyPlan,
    report: &BodyRunReport,
) -> Result<DebuggerSummary, Box<dyn Error>> {
    let history = report
        .debug_observations
        .as_ref()
        .ok_or("Body report has no debug observation history")?;
    let expected = conduit_std_host::body_debug_observation::body_debug_execution_identity(
        plan,
        &report.play,
    )?;
    if history.execution != expected {
        return Err("debug history belongs to another Body Plan/Play".into());
    }
    let mut bindings = Vec::new();
    let mut identities = Vec::new();
    let mut node_kinds = Vec::new();
    let mut response_ports = Vec::new();
    let mut situation_ports = Vec::new();
    for map in &report.partitions {
        let (plot_index, plot, fragment) = plan
            .plots
            .iter()
            .enumerate()
            .find_map(|(index, plot)| {
                if plot.plan.plan_id != map.plan_id {
                    return None;
                }
                plot.plan
                    .fragments
                    .iter()
                    .find(|fragment| fragment.fragment_id == map.fragment_id)
                    .map(|fragment| (index, plot, fragment))
            })
            .ok_or("kernel identity partition is absent from the exact Body Plan")?;
        let qualification = format!(
            "plot={};plan={};fragment={};host={};boot={}",
            plot.plot.source_document_id.as_str(),
            map.plan_id.as_str(),
            map.fragment_id.as_str(),
            fragment.host_id.as_str(),
            fragment.boot_id.as_str()
        );
        for (node, placement_id) in &map.placements {
            let placement = fragment
                .placements
                .iter()
                .find(|placement| placement.placement_id == *placement_id)
                .ok_or("kernel node placement is absent from its exact fragment")?;
            node_kinds.push((*node, placement.kind_id.as_str().to_string()));
            let visible = format!("p{plot_index}/{}", placement.gear_id.as_str());
            bind(
                &mut bindings,
                &mut identities,
                history.execution,
                DebugSubject::Gear(*node),
                visible,
                format!("p{plot_index}/gear/{}", node.0),
                vec![format!(
                    "{qualification};placement={};gear={};kind={}",
                    placement_id.as_str(),
                    placement.gear_id.as_str(),
                    placement.kind_id.as_str()
                )],
                false,
            )?;
        }
        for port in &map.ports {
            let subject = DebugSubject::Port {
                gear: port.node,
                port: port.port,
            };
            // A legacy DebugSubject::Port has no direction bit. Group every
            // matching directional Plan port rather than choosing one by order.
            if bindings
                .iter()
                .any(|binding: &DebuggerTimelineBinding| binding.runtime_subject == subject)
            {
                continue;
            }
            let placement_id = map
                .placement_for_node(port.node)
                .ok_or("port has no exact node placement")?;
            let placement = fragment
                .placements
                .iter()
                .find(|placement| placement.placement_id == *placement_id)
                .ok_or("port placement missing")?;
            let ports: Vec<_> = map
                .ports
                .iter()
                .filter(|candidate| candidate.node == port.node && candidate.port == port.port)
                .collect();
            let names: Vec<_> = ports
                .iter()
                .map(|candidate| {
                    format!(
                        "{}:{}",
                        direction(candidate.direction),
                        candidate.port_id.as_str()
                    )
                })
                .collect();
            let actual = ports
                .iter()
                .map(|candidate| {
                    format!(
                        "{qualification};placement={};{}-port={}",
                        placement_id.as_str(),
                        direction(candidate.direction),
                        candidate.port_id.as_str()
                    )
                })
                .collect();
            for candidate in &ports {
                if candidate.direction == PortDirection::Output
                    && candidate.port_id.as_str() == "response"
                    && placement
                        .kind_id
                        .as_str()
                        .starts_with("experience/autobiography-retain")
                {
                    response_ports.push((port.node, port.port));
                }
                if candidate.direction == PortDirection::Input
                    && candidate.port_id.as_str() == "frame"
                    && placement.kind_id.as_str() == "experience/situation-current"
                {
                    situation_ports.push((port.node, port.port));
                }
            }
            bind(
                &mut bindings,
                &mut identities,
                history.execution,
                subject,
                format!(
                    "p{plot_index}/{}[{}]",
                    placement.gear_id.as_str(),
                    names.join("|")
                ),
                format!(
                    "p{plot_index}/port/{}/{}[{}]",
                    port.node.0,
                    port.port.0,
                    names.join("|")
                ),
                actual,
                ports.len() > 1,
            )?;
        }
        for (cord, connection_id) in &map.connections {
            bind(
                &mut bindings,
                &mut identities,
                history.execution,
                DebugSubject::Cord(*cord),
                format!("p{plot_index}/{}", connection_id.as_str()),
                format!("p{plot_index}/cord/{}", cord.0),
                vec![format!(
                    "{qualification};connection={}",
                    connection_id.as_str()
                )],
                false,
            )?;
        }
        for fore in &map.fore_endpoints {
            let subject = DebugSubject::Cord(fore.cord);
            let actual = format!(
                "{qualification};fore={};direction={};track={:?};endpoint={:?}",
                fore.front_port_id.as_str(),
                direction(fore.direction),
                fore.track,
                fore.endpoint
            );
            if let Some(index) = bindings
                .iter()
                .position(|binding| binding.runtime_subject == subject)
            {
                identities[index].identities.push(actual);
            } else {
                bind(
                    &mut bindings,
                    &mut identities,
                    history.execution,
                    subject,
                    format!(
                        "p{plot_index}/fore/{}:{}",
                        direction(fore.direction),
                        fore.front_port_id.as_str()
                    ),
                    format!("p{plot_index}/cord/{}", fore.cord.0),
                    vec![actual],
                    false,
                )?;
            }
        }
    }
    let mut timeline =
        DebuggerTimeline::new(bindings.clone()).map_err(|e| format!("Patchbay bindings: {e:?}"))?;
    for record in &history.records {
        record
            .validate_for(
                history.execution,
                conduit_kernel::debug_observation::MAX_DEBUG_VALUE_PREVIEW_BYTES as u8,
            )
            .map_err(|e| format!("invalid actual debug record: {e:?}"))?;
        if record
            .type_identity
            .is_some_and(|identity| usize::from(identity) >= history.type_names.len())
        {
            return Err("debug record type identity has no admitted name".into());
        }
        timeline
            .observe(record)
            .map_err(|e| format!("Patchbay observation: {e:?}"))?;
    }
    if let Some(gap) = history.gap {
        timeline.note_gap(gap);
    }
    let response = history.records.iter().rev().find(|record| {
        record.kind == DebugEventKind::ValueSent
            && observed_port(record).is_some_and(|port| response_ports.contains(&port))
    });
    let mut trace_timeline =
        DebuggerTimeline::new(bindings).map_err(|e| format!("Patchbay trace bindings: {e:?}"))?;
    let mut chain = Vec::new();
    let mut missing = Vec::new();
    if let Some(response) = response {
        let mut current = *response;
        loop {
            chain.push(current);
            let Some(parent) = current.causal_parent_sequence else {
                break;
            };
            match history
                .records
                .iter()
                .find(|record| record.execution == current.execution && record.sequence == parent)
            {
                Some(record) => current = *record,
                None => {
                    missing.push(parent);
                    break;
                }
            }
        }
        chain.reverse();
        for record in &chain {
            trace_timeline
                .observe(record)
                .map_err(|e| format!("Patchbay causal observation: {e:?}"))?;
        }
        if let Some(gap) = history.gap {
            trace_timeline.note_gap(gap);
        }
        if let Some(index) = trace_timeline.events.len().checked_sub(1) {
            trace_timeline
                .trace_upstream(index)
                .map_err(|e| format!("Patchbay upstream trace: {e:?}"))?;
        }
        if let Some(index) = timeline
            .events
            .iter()
            .position(|event| event.sequence == response.sequence)
        {
            timeline
                .trace_upstream(index)
                .map_err(|e| format!("Patchbay retained upstream trace: {e:?}"))?;
        }
    }
    if let Some(trace) = &trace_timeline.trace {
        for parent in &trace.missing_parent_sequences {
            if !missing.contains(parent) {
                missing.push(*parent);
            }
        }
    }
    let reaches_source = chain.iter().any(|record| {
        record.kind == DebugEventKind::ValueReceived
            && observed_port(record).is_some_and(|port| situation_ports.contains(&port))
    });
    let mut kinds = Vec::new();
    for record in &chain {
        if let Some(node) = node(record) {
            if let Some((_, kind)) = node_kinds.iter().find(|(candidate, _)| *candidate == node) {
                if !kinds.contains(kind) {
                    kinds.push(kind.clone());
                }
            }
        }
    }
    // Completeness describes this retained single-parent chain, not all fan-in
    // causes or a rendered browser inspection. Any collector gap stays partial.
    let complete = response.is_some()
        && missing.is_empty()
        && history.gap.is_none()
        && history.refusal.is_none()
        && trace_timeline.evicted_events == 0
        && reaches_source;
    let summary = DebuggerSummary {
        recorded_count: history.records.len(),
        retained_timeline_events: timeline.events.len(),
        collector_dropped_records: history.gap.map_or(0, |gap| gap.dropped_records),
        timeline_evicted_events: timeline.evicted_events,
        observer_refusal: history.refusal.map(|error| format!("{error:?}")),
        combined_direction_port_bindings: identities
            .iter()
            .filter(|binding| binding.combined_port_directions)
            .count(),
        final_response_sequence: response.map(|record| record.sequence),
        upstream_trace_complete: complete,
        upstream_trace_scope:
            "retained single-parent pointer chain only; fan-in branches are not inferred",
        full_cognition_causal_chain_verified: false,
        upstream_missing_parent_sequences: missing.clone(),
        upstream_reaches_situation_input: reaches_source,
        upstream_cognition_kinds: kinds,
        rendered_browser_verified: false,
    };
    let artifact=TimelineArtifact{schema:"conduit.pete.live-debugger-projection@1",scope:"Actual bounded kernel history projected through Patchbay DebuggerTimeline; gaps and evictions remain explicit.",type_names:&history.type_names,bindings:&identities,timeline:&timeline,summary:&summary};
    let trace=TraceArtifact{schema:"conduit.pete.live-debugger-upstream@1",scope:"Actual retained causal-parent chain of the final model-backed cognition response; no synthetic events or inferred parents. This is not full multi-input causality or rendered browser proof.",timeline:&trace_timeline,missing_collector_parents:&missing,trace_complete:complete,rendered_browser_verified:false};
    let timeline_bytes = serde_json::to_vec_pretty(&artifact)?;
    let trace_bytes = serde_json::to_vec_pretty(&trace)?;
    if timeline_bytes
        .len()
        .checked_add(trace_bytes.len())
        .is_none_or(|size| size > MAX_ARTIFACT_BYTES)
    {
        return Err("Patchbay JSON artifacts exceed the total 4MiB bound".into());
    }
    write_new(&output.join("debugger-timeline.json"), &timeline_bytes)?;
    write_new(&output.join("debugger-upstream-trace.json"), &trace_bytes)?;
    Ok(summary)
}
fn observed_port(
    record: &DebugObservationRecord,
) -> Option<(conduit_kernel::NodeId, conduit_kernel::PortId)> {
    match record.subject {
        DebugSubject::Port { gear, port } => Some((gear, port)),
        _ => match record.related_subject {
            Some(DebugSubject::Port { gear, port }) => Some((gear, port)),
            _ => None,
        },
    }
}

fn direction(value: PortDirection) -> &'static str {
    match value {
        PortDirection::Input => "in",
        PortDirection::Output => "out",
    }
}
fn node(record: &DebugObservationRecord) -> Option<conduit_kernel::NodeId> {
    match record.subject {
        DebugSubject::Gear(node) | DebugSubject::Port { gear: node, .. } => Some(node),
        DebugSubject::Cord(_) => match record.related_subject {
            Some(DebugSubject::Gear(node) | DebugSubject::Port { gear: node, .. }) => Some(node),
            _ => None,
        },
    }
}
#[allow(clippy::too_many_arguments)]
fn bind(
    bindings: &mut Vec<DebuggerTimelineBinding>,
    identities: &mut Vec<SubjectIdentity>,
    execution: conduit_kernel::debug_observation::DebugExecutionIdentity,
    subject: DebugSubject,
    visible: String,
    fallback: String,
    actual: Vec<String>,
    combined: bool,
) -> Result<(), Box<dyn Error>> {
    if bindings
        .iter()
        .any(|binding| binding.runtime_subject == subject)
    {
        return Err("ambiguous repeated kernel subject across partitions".into());
    }
    let alias = visible.len() > patchbay_application::MAX_DEBUGGER_SUMMARY_BYTES;
    let visible = if alias { fallback } else { visible };
    if visible.len() > patchbay_application::MAX_DEBUGGER_SUMMARY_BYTES {
        return Err("exact direction-combined debugger label exceeds its bound".into());
    }
    identities.push(SubjectIdentity {
        visible_subject: visible.clone(),
        runtime_subject: format!("{subject:?}"),
        identities: actual,
        visible_alias: alias,
        combined_port_directions: combined,
    });
    bindings.push(DebuggerTimelineBinding {
        execution,
        runtime_subject: subject,
        visible_subject: visible,
    });
    Ok(())
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    Ok(())
}
