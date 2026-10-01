use conduit_core::{verify_plan, BaseImplementationId, BootId, HostId, SignId};
use conduit_presentation::{
    plan_face_utterances, render_linear_presentation, FaceUtteranceProvenance, ManifestationError,
    ManifestationFailure, ManifestationLifecycle, RendererExecution, RendererExecutionError,
};
use patchbay_application::{EntranceLayer, EntranceRefusal, PatchbayEntranceState};

use crate::{
    compare_entrances, cross_host_renderer_plan, portable_demonstration,
    prepare_renderer_execution, EntranceEquivalenceError, LocalFrontDoor, RendererAdapterIdentity,
    RendererAdapterKind,
};

fn identity(host: &str, boot: &str, target: &str) -> RendererAdapterIdentity {
    RendererAdapterIdentity {
        host_id: HostId::from(host),
        boot_id: BootId::from(boot),
        target_subject: target.into(),
    }
}

#[test]
fn one_portable_presentation_plans_to_distinct_real_renderer_executions() {
    let presentation = portable_demonstration().unwrap();
    let mut native = prepare_renderer_execution(
        presentation.clone(),
        RendererAdapterKind::NativeWayland,
        identity("native-host", "native-boot", "native/display-0"),
        SignId::from("native/prepared"),
    )
    .unwrap();
    let html = prepare_renderer_execution(
        presentation.clone(),
        RendererAdapterKind::HtmlDomSvg,
        identity("html-host", "html-boot", "html/document-0"),
        SignId::from("html/prepared"),
    )
    .unwrap();
    assert_eq!(native.presentation.identity, html.presentation.identity);
    assert_ne!(native.plan.plan_id, html.plan.plan_id);
    assert_ne!(native.active_play_id, html.active_play_id);
    let native_placement = &native.plan.fragments[0].placements[0];
    let html_placement = &html.plan.fragments[0].placements[0];
    assert_eq!(native_placement.host_id.as_str(), "native-host");
    assert_eq!(native_placement.boot_id.as_str(), "native-boot");
    assert_eq!(
        native_placement.implementation_id.as_str(),
        "presentation/renderer-wayland@1"
    );
    assert_eq!(
        native_placement.host_calls[0]
            .target_kind
            .as_ref()
            .unwrap()
            .as_str(),
        "presentation/base/wayland-surface@1"
    );
    assert_eq!(native_placement.resources.len(), 1);
    assert_eq!(html_placement.host_id.as_str(), "html-host");
    assert_eq!(html_placement.boot_id.as_str(), "html-boot");
    assert_eq!(
        html_placement.implementation_id.as_str(),
        "presentation/renderer-dom-svg@1"
    );
    assert_eq!(
        html_placement.host_calls[0]
            .target_kind
            .as_ref()
            .unwrap()
            .as_str(),
        "presentation/base/dom-svg@1"
    );
    assert_eq!(html_placement.resources.len(), 1);
    assert_eq!(
        native.manifestation.signs[0].placement_id,
        native_placement.placement_id
    );
    assert_eq!(
        html.manifestation.signs[0].placement_id,
        html_placement.placement_id
    );
    assert_eq!(
        native.manifestation.lifecycle,
        ManifestationLifecycle::Prepared
    );
    assert_eq!(
        html.manifestation.lifecycle,
        ManifestationLifecycle::Prepared
    );
    native
        .mark_available(SignId::from("native/available"))
        .unwrap();
    native.mark_closed(SignId::from("native/closed")).unwrap();
    assert_eq!(
        native.manifestation.lifecycle,
        ManifestationLifecycle::Closed
    );
    assert!(native.validate().is_ok() && html.validate().is_ok());
}

#[test]
fn production_patchbay_face_preserves_meaning_across_graphical_aural_and_linear_masks() {
    let mut session = LocalFrontDoor::with_identity(
        crate::host_adapter::test_host_adapter_arc(),
        HostId::from("front-door/conformance"),
        BootId::from("front-door/conformance/boot-1"),
    )
    .unwrap();
    session.plan_and_play().unwrap();
    let face = session.project().unwrap().presentation;
    let native_entrance = PatchbayEntranceState::enter(&face).unwrap();
    let browser_entrance = PatchbayEntranceState::enter(&face).unwrap();
    let equivalence = compare_entrances(&face, &native_entrance, &browser_entrance).unwrap();
    assert!(equivalence.equivalent);
    let native = prepare_renderer_execution(
        face.clone(),
        RendererAdapterKind::NativeWayland,
        identity("native-host", "native-boot", "native/display-0"),
        SignId::from("native/prepared"),
    )
    .unwrap();
    let browser = prepare_renderer_execution(
        face.clone(),
        RendererAdapterKind::HtmlDomSvg,
        identity("browser-host", "browser-boot", "browser/document-0"),
        SignId::from("browser/prepared"),
    )
    .unwrap();
    let aural = plan_face_utterances(&face).unwrap();
    let linear = render_linear_presentation(&face).unwrap();

    for realized in [&native.presentation, &browser.presentation] {
        assert_eq!(realized, &face);
    }
    assert_eq!(aural.source_face_identity, face.identity.as_str());
    assert_eq!(aural.source_face_revision, face.revision);
    assert_eq!(linear.presentation_id, face.identity);
    assert_eq!(linear.revision, face.revision);

    let records = linear.lines.join("\n");
    for subject in &face.subjects {
        assert!(records.contains(&format!("id={:?}", subject.identity)));
        assert!(records.contains(&format!("name={:?}", subject.name)));
        assert!(aural.clauses.iter().any(|clause| {
            clause.provenance == FaceUtteranceProvenance::subject(subject.identity.clone()).unwrap()
        }));
    }
    for relationship in &face.relationships {
        assert!(records.contains(&format!("source={:?}", relationship.source)));
        assert!(records.contains(&format!("target={:?}", relationship.target)));
    }
    for action in &face.actions {
        assert!(records.contains(&format!("id={:?}", action.identity)));
        assert!(aural.clauses.iter().any(|clause| {
            clause.provenance == FaceUtteranceProvenance::action(action.identity.clone()).unwrap()
        }));
    }

    let native_placement = &native.plan.fragments[0].placements[0];
    let browser_placement = &browser.plan.fragments[0].placements[0];
    assert_ne!(native.plan.plan_id, browser.plan.plan_id);
    assert_ne!(
        native_placement.implementation_id,
        browser_placement.implementation_id
    );
    // Different graphical furniture is harmless lost appearance: neither the
    // spoken nor linear Mask promotes renderer-local geometry into Face truth.
    let spoken = aural
        .clauses
        .iter()
        .map(|clause| clause.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    for output in [&records, &spoken] {
        assert!(!output.contains("pixel="));
        assert!(!output.contains("dom-id="));
        assert!(!output.contains(native_placement.implementation_id.as_str()));
        assert!(!output.contains(browser_placement.implementation_id.as_str()));
    }
}

#[test]
fn native_and_browser_share_selection_actions_and_layers_without_renderer_identity() {
    let mut session = LocalFrontDoor::with_identity(
        crate::host_adapter::test_host_adapter_arc(),
        HostId::from("front-door/conformance"),
        BootId::from("front-door/conformance/boot-1"),
    )
    .unwrap();
    session.plan_and_play().unwrap();
    let presentation = session.project().unwrap().presentation;
    let mut native = PatchbayEntranceState::enter(&presentation).unwrap();
    let mut browser = PatchbayEntranceState::enter(&presentation).unwrap();
    let host = presentation
        .subjects
        .iter()
        .find(|subject| subject.role == conduit_presentation::PresentationRole::Host)
        .unwrap()
        .identity
        .clone();
    native.select(&presentation, &host).unwrap();
    browser.select(&presentation, &host).unwrap();
    native
        .show_layer(&presentation, EntranceLayer::Realization)
        .unwrap();
    browser
        .show_layer(&presentation, EntranceLayer::Realization)
        .unwrap();
    assert_eq!(
        native.select(&presentation, "renderer-local/native/window"),
        Err(EntranceRefusal::UnknownSubject)
    );
    assert_eq!(
        browser.select(&presentation, "renderer-local/browser/dom"),
        Err(EntranceRefusal::UnknownSubject)
    );

    let report = compare_entrances(&presentation, &native, &browser).unwrap();
    assert!(report.equivalent);
    assert_eq!(report.selected_subject.as_deref(), Some(host.as_str()));
    assert_eq!(report.refusal, Some(EntranceRefusal::UnknownSubject));
    assert!(report
        .subjects
        .iter()
        .any(|subject| subject.role == conduit_presentation::PresentationRole::Play));
    assert!(report.temporal_references.is_empty());
    assert!(report.temporal_facts.is_empty());
    let encoded = serde_json::to_string(&report).unwrap();
    assert!(!encoded.contains("dom") && !encoded.contains("window") && !encoded.contains("pixel"));

    browser.layer = EntranceLayer::World;
    assert_eq!(
        compare_entrances(&presentation, &native, &browser),
        Err(EntranceEquivalenceError::SemanticDrift)
    );
}

#[test]
fn renderer_failure_is_typed_and_cannot_mutate_source_play_identity() {
    let presentation = portable_demonstration().unwrap();
    let source_play = presentation.basis.active_play_id.clone();
    let source_identity = presentation.identity.clone();
    let mut execution = prepare_renderer_execution(
        presentation,
        RendererAdapterKind::NativeWayland,
        identity("native-host", "native-boot", "native/display-0"),
        SignId::from("native/prepared"),
    )
    .unwrap();
    execution
        .mark_failed(
            ManifestationFailure::OutputRejected,
            SignId::from("native/output-rejected"),
        )
        .unwrap();
    assert_eq!(
        execution.manifestation.lifecycle,
        ManifestationLifecycle::Failed
    );
    assert_eq!(
        execution.manifestation.failure,
        Some(ManifestationFailure::OutputRejected)
    );
    assert_eq!(execution.presentation.identity, source_identity);
    assert_eq!(execution.presentation.basis.active_play_id, source_play);
    assert!(execution.validate().is_ok());
}

#[test]
fn stale_manifestation_correlation_fails_closed() {
    let presentation = portable_demonstration().unwrap();
    let mut execution = prepare_renderer_execution(
        presentation,
        RendererAdapterKind::HtmlDomSvg,
        identity("html-host", "html-boot", "html/document-0"),
        SignId::from("html/prepared"),
    )
    .unwrap();
    execution.manifestation.presentation_revision += 1;
    assert_eq!(
        execution.validate(),
        Err(RendererExecutionError::Manifestation(
            ManifestationError::StaleIdentity
        ))
    );
}

#[test]
fn self_inspection_is_the_exact_renderer_plan_placement_and_sign_chain() {
    let presentation = portable_demonstration().unwrap();
    let mut execution = prepare_renderer_execution(
        presentation,
        RendererAdapterKind::NativeWayland,
        identity("native-host", "native-boot", "native/display-0"),
        SignId::from("native/prepared"),
    )
    .unwrap();
    execution
        .mark_available(SignId::from("native/available"))
        .unwrap();
    let inspection = execution.self_inspection().unwrap();
    let placement = inspection.renderer_placement().unwrap();
    assert_eq!(inspection.plan, execution.plan);
    assert_eq!(inspection.manifestation, execution.manifestation);
    assert_eq!(placement.placement_id, execution.placement_id);
    assert_eq!(placement.inputs, conduit_presentation::renderer_inputs());
    assert_eq!(placement.outputs, conduit_presentation::renderer_outputs());
    assert_eq!(placement.resources.len(), 1);
    assert_eq!(placement.host_calls.len(), 1);
    assert_eq!(inspection.manifestation.signs.len(), 2);
}

#[test]
fn self_inspection_rejects_tampered_plan_placement_and_manifestation_sign() {
    let presentation = portable_demonstration().unwrap();
    let execution = prepare_renderer_execution(
        presentation.clone(),
        RendererAdapterKind::HtmlDomSvg,
        identity("html-host", "html-boot", "html/document-0"),
        SignId::from("html/prepared"),
    )
    .unwrap();
    let mut inspection = execution.self_inspection().unwrap();
    inspection.plan.fragments[0].placements[0].artifact_id =
        conduit_core::ArtifactId::from("tampered");
    assert!(inspection.validate_against(&presentation).is_err());

    let mut inspection = execution.self_inspection().unwrap();
    inspection.manifestation.placement_id = conduit_core::PlacementId::from("missing");
    assert!(inspection.validate_against(&presentation).is_err());

    let mut inspection = execution.self_inspection().unwrap();
    inspection.manifestation.signs[0].plan_id = conduit_core::PlanId::from("tampered");
    assert!(inspection.validate_against(&presentation).is_err());
}

#[test]
fn unchanged_renderer_form_plans_one_exact_cross_host_websocket_line() {
    let exact = cross_host_renderer_plan(
        HostId::from("patchbay-source"),
        BootId::from("patchbay-source-boot"),
        identity("patchbay-html", "patchbay-html-boot", "html/document-0"),
    )
    .unwrap();
    assert!(verify_plan(&exact.plan));
    assert_eq!(exact.plan.fragments.len(), 2);
    let source = exact
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == "patchbay-source")
        .unwrap();
    let sink = exact
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == "patchbay-html")
        .unwrap();
    assert_eq!(
        source.placements[0].kind_id.as_str(),
        "presentation/patchbay-project"
    );
    assert_eq!(sink.placements[0].kind_id.as_str(), "presentation/renderer");
    let connection = source.connections.first().unwrap();
    assert_eq!(
        connection.selected_line.as_ref().unwrap().binding.base,
        BaseImplementationId::from("conduit.base/websocket-rfc6455@1")
    );
    assert_eq!(connection.item_capacity, 1);
    assert_eq!(
        connection.byte_capacity,
        conduit_presentation::MAX_RENDERER_VALUE_BYTES
    );
    let binding = conduit_wire::SessionBinding::from_planned_connection(
        exact.plan.plan_id.clone(),
        source.fragment_id.clone(),
        sink.fragment_id.clone(),
        connection,
    )
    .unwrap();
    assert_eq!(binding.source.host_id, source.host_id);
    assert_eq!(binding.sink.host_id, sink.host_id);
    assert_eq!(
        binding.attachment.base,
        BaseImplementationId::from("conduit.base/websocket-rfc6455@1")
    );
    assert_eq!(binding.attachment.line_id, exact.line.line_id);
}

#[test]
fn planned_renderer_execution_distinguishes_missing_and_ambiguous_placements() {
    let exact = cross_host_renderer_plan(
        HostId::from("patchbay-source"),
        BootId::from("patchbay-source-boot"),
        identity("patchbay-html", "patchbay-html-boot", "html/document-0"),
    )
    .unwrap();
    let presentation = portable_demonstration().unwrap();

    let mut missing = exact.plan.clone();
    for fragment in &mut missing.fragments {
        fragment
            .placements
            .retain(|placement| placement.kind_id.as_str() != "presentation/renderer");
    }
    assert_eq!(
        RendererExecution::prepare_planned(
            presentation.clone(),
            missing,
            "html/document-0".into(),
            SignId::from("missing"),
        ),
        Err(RendererExecutionError::MissingPlacement)
    );

    let mut ambiguous = exact.plan;
    let duplicate = ambiguous
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.kind_id.as_str() == "presentation/renderer")
        .unwrap()
        .clone();
    ambiguous.fragments[0].placements.push(duplicate);
    assert_eq!(
        RendererExecution::prepare_planned(
            presentation,
            ambiguous,
            "html/document-0".into(),
            SignId::from("ambiguous"),
        ),
        Err(RendererExecutionError::AmbiguousPlacement)
    );
}
