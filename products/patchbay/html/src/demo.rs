use crate::RendererSnapshot;
use conduit_core::{BootId, HostId, SignId};
use patchbay_model::{
    PatchbayNavigationProjection, RendererAdapterIdentity, RendererAdapterKind, RendererExecution,
};

pub fn demonstration_snapshot() -> Result<RendererSnapshot, String> {
    let (presentation, parts) = patchbay_model::portable_demonstration_with_parts_and_adapter(
        &patchbay_hosted::HostedPatchbayAdapter,
    )?;
    let execution = RendererExecution::prepare(
        presentation,
        RendererAdapterKind::HtmlDomSvg,
        RendererAdapterIdentity {
            host_id: HostId::from("patchbay-html/host"),
            boot_id: BootId::from("patchbay-html/boot"),
            target_subject: "patchbay-html/document-0".into(),
        },
        SignId::from("patchbay-html/manifestation-prepared"),
    )
    .map_err(|error| error.to_string())?;
    let mut snapshot =
        RendererSnapshot::from_execution(execution).map_err(|error| error.to_string())?;
    snapshot
        .attach_parts(parts)
        .map_err(|error| error.to_string())?;
    let navigation = PatchbayNavigationProjection::for_embodied(&snapshot.presentation)?;
    snapshot
        .attach_navigation(navigation)
        .map_err(|error| error.to_string())?;
    attach_documentary_debugger(&mut snapshot)?;
    Ok(snapshot)
}

pub fn recursive_form_demonstration_snapshot() -> Result<RendererSnapshot, String> {
    let presentation = patchbay_model::recursive_form_demonstration()?;
    let execution = RendererExecution::prepare(
        presentation,
        RendererAdapterKind::HtmlDomSvg,
        RendererAdapterIdentity {
            host_id: HostId::from("patchbay-html/recursive-form"),
            boot_id: BootId::from("patchbay-html/recursive-form/boot"),
            target_subject: "patchbay-html/recursive-form/document".into(),
        },
        SignId::from("patchbay-html/recursive-form/prepared"),
    )
    .map_err(|error| error.to_string())?;
    execution.validate().map_err(|error| error.to_string())?;
    patchbay_model::PatchbayEntranceState::enter(&execution.presentation)
        .map_err(|error| format!("recursive Form entrance: {error:?}"))?;
    let mut snapshot =
        RendererSnapshot::from_execution(execution).map_err(|error| error.to_string())?;
    let navigation = PatchbayNavigationProjection::for_embodied(&snapshot.presentation)?;
    snapshot
        .attach_navigation(navigation)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

fn attach_documentary_debugger(snapshot: &mut RendererSnapshot) -> Result<(), String> {
    let find = |role| {
        snapshot
            .presentation
            .subjects
            .iter()
            .find(|subject| subject.role == role)
            .map(|subject| subject.identity.clone())
            .ok_or_else(|| format!("documentary debugger fixture has no {role:?}"))
    };
    let gear = find(conduit_presentation::PresentationRole::Gear)?;
    let port = find(conduit_presentation::PresentationRole::Port)?;
    let cord = find(conduit_presentation::PresentationRole::Cord)?;
    let execution = serde_json::json!({
        "body": vec![21; 32], "plan": vec![22; 32], "play": vec![23; 32]
    });
    let debugger: patchbay_model::DebuggerPresentation = serde_json::from_value(
        serde_json::json!({
            "schema": patchbay_model::DEBUGGER_PRESENTATION_SCHEMA,
            "execution": execution,
            "revision": 3,
            "tick": 0,
            "reduced_motion": false,
            "gap": { "dropped_records": 2, "first_retained_sequence": 40 },
            "activities": [
                { "subject": gear, "line_subject": null, "host": 1, "phase": "faulted", "latest_kind": "fault", "latest_sequence": 40, "observed_count": 1, "coalesced_count": 0, "last_activity_tick": 0, "latest_value": null, "retained_fault_code": 17 },
                { "subject": port, "line_subject": null, "host": 1, "phase": "active", "latest_kind": "value-received", "latest_sequence": 41, "observed_count": 2, "coalesced_count": 1, "last_activity_tick": 0, "latest_value": { "kind": "text", "summary": "\"hello watch\"", "type_identity": 11, "total_bytes": 11, "truncated": false }, "retained_fault_code": null },
                { "subject": cord, "line_subject": null, "host": 1, "phase": "active", "latest_kind": "value-sent", "latest_sequence": 42, "observed_count": 3, "coalesced_count": 2, "last_activity_tick": 0, "latest_value": { "kind": "scalar", "summary": "42", "type_identity": 12, "total_bytes": 2, "truncated": false }, "retained_fault_code": null }
            ]
        }),
    )
    .map_err(|error| error.to_string())?;
    let watches: patchbay_model::DebuggerWatchSet = serde_json::from_value(serde_json::json!({
        "schema": patchbay_model::DEBUGGER_WATCH_SCHEMA,
        "execution": debugger.execution,
        "revision": 0,
        "focused_subject": null,
        "eligible_subjects": [[gear, "gear"], [port, "port"], [cord, "cord"]],
        "watches": []
    }))
    .map_err(|error| error.to_string())?;
    let debugger_execution = debugger.execution.clone();
    snapshot
        .attach_debugger(debugger)
        .map_err(|error| error.to_string())?;
    snapshot
        .attach_watches(watches)
        .map_err(|error| error.to_string())?;
    let events: Vec<patchbay_model::DebuggerTimelineEvent> = serde_json::from_value(
        serde_json::json!([
            { "execution": execution, "sequence": 39, "host_sequence": 39, "host": 1, "form": 1, "subject": cord, "related_subject": null, "event": "value-sent", "value": { "kind": "scalar", "summary": "41", "type_identity": 12, "total_bytes": 2, "truncated": false }, "fault_code": null, "causal_parent_sequence": null, "invocation_sequence": 39 },
            { "execution": execution, "sequence": 40, "host_sequence": 40, "host": 1, "form": 1, "subject": gear, "related_subject": null, "event": "fault", "value": null, "fault_code": 17, "causal_parent_sequence": 39, "invocation_sequence": 39 },
            { "execution": execution, "sequence": 41, "host_sequence": 41, "host": 1, "form": 1, "subject": port, "related_subject": null, "event": "value-received", "value": { "kind": "text", "summary": "\"hello watch\"", "type_identity": 11, "total_bytes": 11, "truncated": false }, "fault_code": null, "causal_parent_sequence": 39, "invocation_sequence": 39 },
            { "execution": execution, "sequence": 42, "host_sequence": 42, "host": 1, "form": 1, "subject": cord, "related_subject": null, "event": "value-sent", "value": { "kind": "scalar", "summary": "42", "type_identity": 12, "total_bytes": 2, "truncated": false }, "fault_code": null, "causal_parent_sequence": 41, "invocation_sequence": 39 }
        ]),
    )
    .map_err(|error| error.to_string())?;
    let retained_bytes: usize = events
        .iter()
        .map(patchbay_model::DebuggerTimelineEvent::retained_bytes)
        .sum();
    let timeline: patchbay_model::DebuggerTimeline = serde_json::from_value(serde_json::json!({
        "schema": patchbay_model::DEBUGGER_TIMELINE_SCHEMA,
        "revision": 4,
        "mode": "live",
        "cursor": 3,
        "selected_event": null,
        "subject_filter": null,
        "events": events,
        "retained_bytes": retained_bytes,
        "evicted_events": 0,
        "gap": { "dropped_records": 2, "first_retained_sequence": 39 }
    }))
    .map_err(|error| error.to_string())?;
    snapshot
        .attach_timeline(timeline)
        .map_err(|error| error.to_string())?;
    snapshot
        .attach_debugger_control(patchbay_model::DebuggerExecutionControl::new(
            debugger_execution,
            vec![gear],
        ))
        .map_err(|error| error.to_string())
}

pub fn llm_documentary_snapshot() -> Result<RendererSnapshot, String> {
    let presentation = patchbay_model::llm_documentary_presentation_with_adapter(
        &patchbay_hosted::HostedPatchbayAdapter,
    )?;
    let execution = RendererExecution::prepare(
        presentation,
        RendererAdapterKind::HtmlDomSvg,
        RendererAdapterIdentity {
            host_id: HostId::from("patchbay-html/llm-documentary"),
            boot_id: BootId::from("patchbay-html/llm-documentary/boot"),
            target_subject: "patchbay-html/llm-documentary/document".into(),
        },
        SignId::from("patchbay-html/llm-documentary/prepared"),
    )
    .map_err(|error| error.to_string())?;
    let mut snapshot =
        RendererSnapshot::from_execution(execution).map_err(|error| error.to_string())?;
    let navigation = PatchbayNavigationProjection::for_embodied(&snapshot.presentation)?;
    snapshot
        .attach_navigation(navigation)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

pub fn llm_embodiment_snapshot(stage: usize) -> Result<RendererSnapshot, String> {
    let presentation = patchbay_model::llm_embodiment_documentary_presentations()
        .map_err(|error| format!("{error:?}"))?
        .into_iter()
        .nth(stage)
        .ok_or("LLM embodiment stage must be 0, 1, or 2")?;
    let execution = RendererExecution::prepare(
        presentation,
        RendererAdapterKind::HtmlDomSvg,
        RendererAdapterIdentity {
            host_id: HostId::from("patchbay-html/llm-embodiment"),
            boot_id: BootId::from("patchbay-html/llm-embodiment/boot"),
            target_subject: format!("patchbay-html/llm-embodiment/{stage}"),
        },
        SignId::from(format!("patchbay-html/llm-embodiment/{stage}/prepared")),
    )
    .map_err(|error| error.to_string())?;
    let mut snapshot =
        RendererSnapshot::from_execution(execution).map_err(|error| error.to_string())?;
    let navigation = PatchbayNavigationProjection::for_embodied(&snapshot.presentation)?;
    snapshot
        .attach_navigation(navigation)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

pub fn text_lab_split_snapshot(base: &str) -> Result<RendererSnapshot, String> {
    let explanation = patchbay_model::text_lab_split_explanation(base)?;
    text_lab_snapshot(explanation)
}

pub fn text_lab_split_loss_snapshot(
    base: &str,
    receipt: &conduit_semantic_catalog::TextLabLineLossReceipt,
) -> Result<RendererSnapshot, String> {
    text_lab_snapshot(patchbay_model::text_lab_split_loss_explanation(
        base, receipt,
    )?)
}

fn text_lab_snapshot(
    explanation: patchbay_model::TextLabSplitExplanation,
) -> Result<RendererSnapshot, String> {
    let execution = RendererExecution::prepare(
        explanation.presentation,
        RendererAdapterKind::HtmlDomSvg,
        RendererAdapterIdentity {
            host_id: HostId::from("patchbay-html/text-lab"),
            boot_id: BootId::from("patchbay-html/text-lab/boot"),
            target_subject: "patchbay-html/text-lab/document".into(),
        },
        SignId::from("patchbay-html/text-lab/prepared"),
    )
    .map_err(|error| error.to_string())?;
    let mut snapshot =
        RendererSnapshot::from_execution(execution).map_err(|error| error.to_string())?;
    snapshot
        .attach_navigation(explanation.navigation)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}
