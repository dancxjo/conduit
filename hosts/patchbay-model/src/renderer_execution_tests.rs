use conduit_core::{BootId, ClueId, HostId};
use conduit_presentation::{ManifestationError, ManifestationFailure, ManifestationLifecycle};

use crate::{
    portable_demonstration, RendererAdapterIdentity, RendererAdapterKind, RendererExecution,
    RendererExecutionError,
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
    let mut native = RendererExecution::prepare(
        presentation.clone(),
        RendererAdapterKind::NativeWayland,
        identity("native-host", "native-boot", "native/display-0"),
        ClueId::from("native/prepared"),
    )
    .unwrap();
    let html = RendererExecution::prepare(
        presentation.clone(),
        RendererAdapterKind::HtmlDomSvg,
        identity("html-host", "html-boot", "html/document-0"),
        ClueId::from("html/prepared"),
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
        native_placement.host_operations[0]
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
        html_placement.host_operations[0]
            .target_kind
            .as_ref()
            .unwrap()
            .as_str(),
        "presentation/base/dom-svg@1"
    );
    assert_eq!(html_placement.resources.len(), 1);
    assert_eq!(
        native.manifestation.clues[0].placement_id,
        native_placement.placement_id
    );
    assert_eq!(
        html.manifestation.clues[0].placement_id,
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
        .mark_available(ClueId::from("native/available"))
        .unwrap();
    native.mark_closed(ClueId::from("native/closed")).unwrap();
    assert_eq!(
        native.manifestation.lifecycle,
        ManifestationLifecycle::Closed
    );
    assert!(native.validate().is_ok() && html.validate().is_ok());
}

#[test]
fn renderer_failure_is_typed_and_cannot_mutate_source_play_identity() {
    let presentation = portable_demonstration().unwrap();
    let source_play = presentation.basis.active_play_id.clone();
    let source_identity = presentation.identity.clone();
    let mut execution = RendererExecution::prepare(
        presentation,
        RendererAdapterKind::NativeWayland,
        identity("native-host", "native-boot", "native/display-0"),
        ClueId::from("native/prepared"),
    )
    .unwrap();
    execution
        .mark_failed(
            ManifestationFailure::OutputRejected,
            ClueId::from("native/output-rejected"),
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
    let mut execution = RendererExecution::prepare(
        presentation,
        RendererAdapterKind::HtmlDomSvg,
        identity("html-host", "html-boot", "html/document-0"),
        ClueId::from("html/prepared"),
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
