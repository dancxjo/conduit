//! Project the recorded cognition Play into a portable Patchbay snapshot.
#[path = "pete_replay/realization.rs"]
mod realization;
use conduit_body::BodyConversationContext;
use conduit_browser_patchbay_workbench::RendererSnapshot;
use conduit_core::{PlanFragment, PortDirection};
use conduit_plot::ExpandedAuthoringPlot;
use conduit_std_host::StdRunReport;

pub fn snapshot(
    fragment: &PlanFragment,
    authoring: &ExpandedAuthoringPlot,
    context: &BodyConversationContext,
    report: &StdRunReport,
) -> Result<RendererSnapshot, String> {
    use conduit_kernel::debug_observation::DebugSubject;
    use conduit_plan_lowering::lowering::lower_plan_fragment;
    use patchbay_application::{
        DebuggerTimeline, DebuggerTimelineBinding, PatchbayActivePlot, PatchbayActivePlotState,
        PatchbayApplicationPort,
    };

    let kernel = report
        .kernel
        .as_ref()
        .ok_or("Play has no installed kernel report")?;
    let history = kernel
        .debug_history
        .as_ref()
        .ok_or("Play has no debugger history")?;
    let plan_id = fragment.plan_id.clone();
    if kernel.active_play.host_id != fragment.host_id || kernel.identity.plan_id != plan_id {
        return Err("debug history differs from the selected Host or Plan".into());
    }
    let plot = PatchbayActivePlot::project_authoring(
        authoring,
        "Pete cognition",
        plan_id.clone(),
        PatchbayActivePlotState::Playing,
        true,
    )
    .map_err(|e| format!("prepare Patchbay replay: {e:?}"))?;
    let patchbay = PatchbayApplicationPort::open_active(
        vec![plot],
        plan_id,
        Some(kernel.active_play_id.clone()),
    )
    .map_err(|e| format!("prepare Patchbay replay: {e:?}"))?;
    let mut snapshot = pete_replay_snapshot(
        fragment,
        context,
        &patchbay,
        kernel.active_play_id.clone(),
        report
            .observations
            .last()
            .ok_or("Play has no terminal Sign")?
            .sign_id
            .clone(),
    )?;
    let graph = patchbay.graph();
    let lowered =
        lower_plan_fragment(fragment).map_err(|e| format!("lower replay fragment: {e:?}"))?;
    let identity = &lowered.identity;
    let gear_for = |placement_id: &conduit_core::PlacementId| {
        let placement = fragment
            .placements
            .iter()
            .find(|p| &p.placement_id == placement_id)
            .ok_or("replay placement is missing")?;
        graph
            .gears
            .iter()
            .find(|gear| gear.gear_id == placement.gear_id)
            .ok_or("replay Gear is missing")
    };
    let port_for = |placement, direction, id: &str| {
        let gear = gear_for(placement)?;
        let ports = match direction {
            PortDirection::Input => &gear.inputs,
            PortDirection::Output => &gear.outputs,
        };
        ports
            .iter()
            .find(|port| port.descriptor.port_id.as_str() == id)
            .map(|port| &port.identity)
            .ok_or("replay Port is missing")
    };
    let bind = |runtime_subject, visible_subject: &String| -> Result<_, String> {
        Ok(DebuggerTimelineBinding {
            execution: history.execution,
            runtime_subject,
            visible_subject: snapshot
                .face
                .properties
                .iter()
                .find(|property| {
                    property.name == "semantic-id"
                        && property.value
                            == conduit_face::FacePropertyValue::Identity(visible_subject.clone())
                })
                .ok_or_else(|| format!("Face is missing {visible_subject}"))?
                .subject
                .clone(),
        })
    };
    let mut bindings = Vec::new();
    for (node, placement) in &identity.placements {
        bindings.push(bind(
            DebugSubject::Gear(*node),
            &gear_for(placement)?.identity,
        )?);
    }
    for port in &identity.ports {
        let placement = identity
            .placement_for_node(port.node)
            .ok_or("replay Port placement is missing")?;
        bindings.push(bind(
            match port.direction {
                PortDirection::Input => DebugSubject::Port {
                    gear: port.node,
                    port: port.port,
                },
                PortDirection::Output => DebugSubject::OutputPort {
                    gear: port.node,
                    port: port.port,
                },
            },
            port_for(placement, port.direction, port.port_id.as_str())?,
        )?);
    }
    for (cord, connection) in &identity.connections {
        if identity
            .fore_endpoints
            .iter()
            .any(|endpoint| endpoint.cord == *cord)
        {
            continue;
        }
        let connection = fragment
            .connections
            .iter()
            .find(|c| &c.connection_id == connection)
            .ok_or("replay Cord binding is missing")?;
        let source = port_for(
            &connection.source_placement_id,
            PortDirection::Output,
            connection.source_port_id.as_str(),
        )?;
        let sink = port_for(
            &connection.sink_placement_id,
            PortDirection::Input,
            connection.sink_port_id.as_str(),
        )?;
        let graph_cord = graph
            .cords
            .iter()
            .find(|c| &c.source_port == source && &c.sink_port == sink)
            .ok_or("replay Cord binding is missing")?;
        bindings.push(bind(DebugSubject::Cord(*cord), &graph_cord.identity)?);
    }
    for endpoint in &identity.fore_endpoints {
        let port = format!(
            "front/{}/{}",
            if endpoint.direction == PortDirection::Input {
                "input"
            } else {
                "output"
            },
            endpoint.front_port_id.as_str(),
        );
        let cord = graph
            .cords
            .iter()
            .find(|cord| cord.source_port == port || cord.sink_port == port)
            .ok_or("replay Cord binding is missing")?;
        bindings.push(bind(DebugSubject::Cord(endpoint.cord), &cord.identity)?);
    }
    let mut timeline =
        DebuggerTimeline::new(bindings).map_err(|e| format!("bind replay timeline: {e:?}"))?;
    for record in &history.records {
        timeline
            .observe(record)
            .map_err(|error| format!("observe replay event: {error:?}"))?;
    }
    if let Some(gap) = history.gap {
        timeline.note_gap(gap);
    }
    let response = timeline
        .events
        .iter()
        .rposition(|event| {
            event.event == "value-sent"
                && event.subject.ends_with("/front/output/episodes")
                && event
                    .value
                    .as_ref()
                    .is_some_and(|value| value.total_bytes > 0)
        })
        .ok_or("replay contains no episode output")?;
    timeline
        .trace_upstream(response)
        .map_err(|e| format!("trace replay: {e:?}"))?;
    snapshot
        .attach_timeline(timeline)
        .map_err(|e| e.to_string())?;
    Ok(snapshot)
}

fn pete_replay_snapshot(
    fragment: &PlanFragment,
    context: &BodyConversationContext,
    patchbay: &patchbay_application::PatchbayApplicationPort,
    active_play_id: conduit_core::ActivePlayId,
    terminal_sign: conduit_core::SignId,
) -> Result<RendererSnapshot, String> {
    use conduit_browser_patchbay_workbench::RendererSnapshot;
    use conduit_face::{Face, FaceBasis, FaceProperty, FacePropertyValue, FaceRole, FaceSubject};
    use conduit_patchbay_workbench::{
        prepare_renderer_execution, RendererAdapterIdentity, RendererAdapterKind,
    };
    use patchbay_application::{graph_face, PatchbayNavigationProjection};

    let body = format!("body/{}", context.body_id.as_str());
    let graph = patchbay.graph();
    let plot = format!("plot/{}", graph.checked_plot_id.as_str());
    let mut subjects = vec![FaceSubject {
        identity: body.clone(),
        role: FaceRole::Body,
        name: "Pete".into(),
    }];
    let mut properties = vec![FaceProperty {
        subject: plot.clone(),
        name: "host-id".into(),
        value: FacePropertyValue::Identity(fragment.host_id.as_str().into()),
    }];
    let mut relationships = Vec::new();
    let mut texts = Vec::new();
    let mut disclosures = Vec::new();
    graph_face::append(
        graph,
        &body,
        &mut graph_face::Content {
            subjects: &mut subjects,
            relationships: &mut relationships,
            properties: &mut properties,
            texts: &mut texts,
            disclosures: &mut disclosures,
        },
    );
    realization::append(fragment, patchbay, &mut properties)?;
    let face = Face::new_with_semantics(
        1,
        FaceBasis {
            body_id: Some(context.body_id.clone()),
            wake_id: Some(context.wake_id.clone()),
            source_document_id: Some(graph.source_document_id.clone()),
            checked_plot_id: Some(graph.checked_plot_id.clone()),
            expanded_plot_id: Some(graph.expanded_plot_id.clone()),
            plan_id: Some(fragment.plan_id.clone()),
            active_play_id: Some(active_play_id),
            sign_ids: vec![terminal_sign],
        },
        subjects,
        relationships,
        properties,
        texts,
        Vec::new(),
        disclosures,
    )
    .map_err(|e| format!("build replay Face: {e:?}"))?;
    let mut navigation = PatchbayNavigationProjection::for_embodied(&face)?;
    navigation.cursor.place = conduit_face::FacePlace::Program;
    navigation.cursor.aspect = conduit_face::FaceAspect::Signs;
    navigation.cursor.focus = Some(plot);
    let execution = prepare_renderer_execution(
        face,
        RendererAdapterKind::HtmlDomSvg,
        RendererAdapterIdentity {
            host_id: "host/patchbay-pete-replay".into(),
            boot_id: "boot/patchbay-pete-replay".into(),
            target_subject: "patchbay/pete-replay".into(),
        },
        "sign/patchbay-pete-replay".into(),
    )
    .map_err(|e| format!("build replay Face: {e:?}"))?;
    let mut snapshot = RendererSnapshot::from_execution(execution).map_err(|e| e.to_string())?;
    snapshot
        .attach_navigation(navigation)
        .map_err(|e| e.to_string())?;
    Ok(snapshot)
}
