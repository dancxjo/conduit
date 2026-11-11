//! Read-only historical inspection of an actual exported Pete cognition run.
mod bindings;
mod causal;
mod episode_values;
use crate::{RendererSnapshot, MAX_SNAPSHOT_BYTES};
use conduit_body::{Body, BodyPlan, BodyPlayIdentity, Wake};
use conduit_core::{BootId, HostId, SignId};
use conduit_face::{
    Face, FaceBasis, FaceProperty, FacePropertyValue, FaceRole, FaceSubject, FaceText,
};
use conduit_patchbay_workbench::{
    prepare_renderer_execution, PatchbayGraph, PatchbayRequestId, PlanDocument, PlotEditor,
    RendererAdapterIdentity, RendererAdapterKind,
};
use conduit_plot::{ProfileCatalog, StartupCatalog};
use patchbay_application::{graph_face, DebuggerTimeline};
use serde::Deserialize;
use std::{error::Error, fs, path::Path};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;
#[derive(Deserialize)]
struct Run {
    play: BodyPlayIdentity,
    terminal: String,
    kernel_events: Vec<serde_json::Value>,
}
#[derive(Deserialize)]
struct ImportedBinding {
    visible_subject: String,
    runtime_subject: String,
}
#[derive(Deserialize)]
struct DebugArtifact {
    timeline: DebuggerTimeline,
    bindings: Vec<ImportedBinding>,
}

pub fn snapshot_from_pete_cognition_run(directory: &Path) -> Result<RendererSnapshot> {
    let body: Body = read(directory, "body.json")?;
    let wake: Wake = read(directory, "wake.json")?;
    let plan: BodyPlan = read(directory, "body-plan.json")?;
    let run: Run = read(directory, "body-run.json")?;
    let mut debug: DebugArtifact = read(directory, "debugger-timeline.json")?;
    body.validate()
        .map_err(|e| format!("historical Body: {e:?}"))?;
    wake.validate()
        .map_err(|e| format!("historical Wake: {e:?}"))?;
    plan.validate_for(&wake)
        .map_err(|e| format!("historical BodyPlan: {e:?}"))?;
    if body.body_id != plan.body_id
        || body.workset != wake.workset
        || body.workload_revision != wake.workload_revision
        || !matches!(&body.state,conduit_body::BodyState::Awake{wake_id} if wake_id==&wake.wake_id)
        || !run.play.validate_for(&plan)
    {
        return Err("historical Body/Wake/Plan/Play mismatch".into());
    }
    let execution =
        conduit_std_host::body_debug_observation::body_debug_execution_identity(&plan, &run.play)?;
    if debug
        .timeline
        .events
        .iter()
        .any(|event| event.execution != execution.into())
    {
        return Err("timeline belongs to another actual Body Play".into());
    }
    let mut graphs = Vec::new();
    for partition in &plan.plots {
        if partition.plan.fragments.len() != 1 {
            return Err("historical local inspector requires one fragment per actual Plot".into());
        }
        let cognition = partition
            .plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .any(|gear| gear.kind_id.as_str() == "experience/situation-current");
        let (source, entry, path) = if cognition {
            (
                conduit_pete::PETE_SITUATED_COGNITION_PLOT_SOURCE,
                "pete-situated-cognition",
                "plots/pete-cognition/situated.conduit",
            )
        } else {
            (
                include_str!("../../../../plots/clock/main.conduit"),
                "clock-demo",
                "plots/clock/main.conduit",
            )
        };
        let editor = if cognition {
            let mut startup = StartupCatalog::new();
            let mut profile = ProfileCatalog::new();
            conduit_pete::install_pete_situated_cognition_catalogs(&mut startup, &mut profile)?;
            PlotEditor::from_source_with_catalogs(path.into(), source.into(), startup, profile)
        } else {
            PlotEditor::from_source(path.into(), source.into())
        }
        .map_err(|e| format!("historical source editor: {e:?}"))?;
        let authoring = editor
            .expand_plot_for_authoring(entry)
            .map_err(|e| format!("historical graph expansion: {e:?}"))?;
        if authoring.expanded.source_document_id != partition.plan.source_document_id
            || authoring.expanded.checked_plot_id != partition.plan.checked_plot_id
            || authoring.expanded.expanded_plot_id != partition.plan.expanded_plot_id
        {
            return Err("exported Plan does not match the exact current authored source".into());
        }
        let document = PlanDocument::from_plan(
            PatchbayRequestId::new(format!("historical/{}", graphs.len()))
                .map_err(|e| format!("inspection request: {e:?}"))?,
            &partition.plan,
        )
        .map_err(|e| format!("actual PlanDocument: {e:?}"))?;
        if document.exact != partition.plan {
            return Err("PlanDocument changed actual Plan".into());
        }
        graphs.push(
            PatchbayGraph::from_authoring(&authoring)
                .map_err(|e| format!("actual authored graph: {e:?}"))?,
        );
    }
    let body_subject = format!("body/{}", body.body_id.as_str());
    let mut subjects = vec![FaceSubject {
        identity: body_subject.clone(),
        role: FaceRole::Body,
        name: "Pete cognition — historical replay".into(),
    }];
    let mut relationships = vec![];
    let mut properties = vec![];
    let mut texts=vec![FaceText{subject:body_subject.clone(),text:format!("Historical, read-only inspection. Recorded Body Play {} ended {}. The saved Body/Wake are pre-Play snapshots; they do not claim current liveness. This browser renderer is separate from that execution.",run.play.active_play_id.as_str(),run.terminal)}];
    let mut disclosures = vec![];
    for graph in &graphs {
        graph_face::append(
            graph,
            &body_subject,
            &mut graph_face::Content {
                subjects: &mut subjects,
                relationships: &mut relationships,
                properties: &mut properties,
                texts: &mut texts,
                disclosures: &mut disclosures,
            },
        );
    }
    for (partition, graph) in plan.plots.iter().zip(&graphs) {
        let subject = format!("plot/{}", graph.checked_plot_id.as_str());
        properties.push(FaceProperty {
            subject: subject.clone(),
            name: "historical-partition-plan".into(),
            value: FacePropertyValue::Text(partition.plan.plan_id.as_str().into()),
        });
        properties.push(FaceProperty {
            subject,
            name: "historical-source-document".into(),
            value: FacePropertyValue::Text(partition.plan.source_document_id.as_str().into()),
        });
    }
    for (name, value) in [
        (
            "inspection-scope",
            "historical-replay; read-only imported actual run",
        ),
        ("body-plan-id", plan.plan_id.as_str()),
        ("body-play-id", run.play.active_play_id.as_str()),
        ("historical-terminal", run.terminal.as_str()),
    ] {
        properties.push(FaceProperty {
            subject: body_subject.clone(),
            name: name.into(),
            value: if matches!(name, "body-plan-id" | "body-play-id") {
                FacePropertyValue::Identity(value.into())
            } else {
                FacePropertyValue::Text(value.into())
            },
        });
    }
    let mapping = bindings::subjects(
        &plan,
        &graphs,
        &mut subjects,
        &mut relationships,
        &mut properties,
    )?;
    let mut translations = Vec::new();
    for binding in &debug.bindings {
        let canonical = mapping
            .iter()
            .find(|(runtime, _)| runtime == &binding.runtime_subject)
            .map(|(_, canonical)| canonical.clone())
            .ok_or("artifact runtime binding is absent from the actual lowered Plan")?;
        if translations
            .iter()
            .any(|(prior, _)| prior == &binding.visible_subject)
        {
            return Err("duplicate imported visible subject alias".into());
        }
        translations.push((binding.visible_subject.clone(), canonical));
    }
    let translate = |name: &str| -> Result<String> {
        translations
            .iter()
            .find(|(prior, _)| prior == name)
            .map(|(_, actual)| actual.clone())
            .ok_or_else(|| "timeline subject has no actual Plan binding".into())
    };
    for event in &mut debug.timeline.events {
        event.subject = translate(&event.subject)?;
        event.related_subject = event
            .related_subject
            .as_deref()
            .map(&translate)
            .transpose()?;
    }
    if let Some(trace) = &mut debug.timeline.trace {
        if trace.execution != execution.into() {
            return Err("trace belongs to another Body Play".into());
        }
        for step in &mut trace.steps {
            step.subject = translate(&step.subject)?;
        }
    }
    debug.timeline.subject_filter = debug
        .timeline
        .subject_filter
        .as_deref()
        .map(&translate)
        .transpose()?;
    debug.timeline.retained_bytes = debug
        .timeline
        .events
        .iter()
        .map(patchbay_application::DebuggerTimelineEvent::retained_bytes)
        .sum();
    if debug.timeline.retained_bytes > patchbay_application::MAX_DEBUGGER_TIMELINE_BYTES {
        return Err("canonical-subject timeline exceeds Patchbay's finite history bound".into());
    }
    debug.timeline.pause();
    causal::append(
        directory,
        &plan,
        &run,
        &body_subject,
        &graphs,
        &mut graph_face::Content {
            subjects: &mut subjects,
            relationships: &mut relationships,
            properties: &mut properties,
            texts: &mut texts,
            disclosures: &mut disclosures,
        },
    )?;
    episode_values::append(
        directory,
        &body_subject,
        &mut graph_face::Content {
            subjects: &mut subjects,
            relationships: &mut relationships,
            properties: &mut properties,
            texts: &mut texts,
            disclosures: &mut disclosures,
        },
    )?;
    let face = Face::new_with_semantics(
        1,
        FaceBasis {
            body_id: Some(body.body_id.clone()),
            wake_id: Some(wake.wake_id.clone()),
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            // A Body-wide Plan has no single ExpandedPlot identity. Its exact
            // historical IDs are properties, not a forged per-Plot Face basis.
            plan_id: None,
            active_play_id: None,
            sign_ids: body.sign_ids.clone(),
        },
        subjects,
        relationships,
        properties,
        texts,
        vec![],
        disclosures,
    )
    .map_err(|error| format!("historical Face contract: {error:?}"))?;
    let renderer = prepare_renderer_execution(
        face,
        RendererAdapterKind::HtmlDomSvg,
        RendererAdapterIdentity {
            host_id: HostId::from("patchbay/pete-historical-inspector"),
            boot_id: BootId::from("patchbay/pete-historical-inspector/boot"),
            target_subject: "patchbay/pete-historical-inspector/document".into(),
        },
        SignId::from("patchbay/pete-historical-inspector/prepared"),
    )
    .map_err(|e| format!("historical renderer: {e:?}"))?;
    let mut snapshot = RendererSnapshot::from_execution(renderer)?;
    let navigation =
        patchbay_application::PatchbayNavigationProjection::for_embodied(&snapshot.face)?;
    snapshot.attach_navigation(navigation)?;
    snapshot.attach_timeline(debug.timeline)?;
    if snapshot.encode()?.len() > MAX_SNAPSHOT_BYTES {
        return Err("historical snapshot exceeds finite renderer envelope".into());
    }
    Ok(snapshot)
}
fn read<T: serde::de::DeserializeOwned>(directory: &Path, name: &str) -> Result<T> {
    let path = directory.join(name);
    let metadata = fs::metadata(&path)?;
    if !metadata.is_file() || metadata.len() > MAX_INPUT_BYTES as u64 {
        return Err(format!("bounded historical input refused: {name}").into());
    }
    let bytes = fs::read(path)?;
    if bytes.len() > MAX_INPUT_BYTES {
        return Err("historical input grew beyond its bound".into());
    }
    Ok(serde_json::from_slice(&bytes)?)
}
