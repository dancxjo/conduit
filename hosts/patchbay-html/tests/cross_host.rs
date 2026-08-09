use conduit_core::{bind_active_play, verify_plan, ConnectionBase};
use conduit_presentation::ManifestationLifecycle;
use patchbay_html::cross_host_demonstration_snapshot;
use patchbay_model::{CROSS_HOST_RENDERER_GEAR, CROSS_HOST_SOURCE_GEAR, PRESENTATION_PROJECT_KIND};

#[test]
fn one_exact_presentation_crosses_the_planned_line_into_the_html_renderer() {
    let snapshot = cross_host_demonstration_snapshot().unwrap();
    let plan = &snapshot.renderer.plan;
    assert!(verify_plan(plan));
    assert_eq!(plan.fragments.len(), 2);

    let source = plan
        .fragments
        .iter()
        .find(|fragment| {
            fragment
                .placements
                .iter()
                .any(|placement| placement.gear_id.as_str() == CROSS_HOST_SOURCE_GEAR)
        })
        .unwrap();
    let renderer = plan
        .fragments
        .iter()
        .find(|fragment| {
            fragment
                .placements
                .iter()
                .any(|placement| placement.gear_id.as_str() == CROSS_HOST_RENDERER_GEAR)
        })
        .unwrap();
    assert_ne!(source.host_id, renderer.host_id);
    assert_ne!(source.boot_id, renderer.boot_id);
    assert_eq!(
        source.placements[0].kind_id.as_str(),
        PRESENTATION_PROJECT_KIND
    );

    let connection = source.connections.first().unwrap();
    let line = connection.selected_line.as_ref().unwrap();
    assert_eq!(line.binding.base, ConnectionBase::WebSocket);
    assert_eq!(line.line_id.as_str(), "patchbay-renderer/line/websocket");
    assert_eq!(
        line.binding.binding_id.as_str(),
        "patchbay-renderer/binding/websocket"
    );
    assert_eq!(
        line.binding.base_instance_id.as_str(),
        "patchbay-renderer/websocket-instance"
    );
    assert_eq!(line.binding.source.host_id, source.host_id);
    assert_eq!(line.binding.source.boot_id, source.boot_id);
    assert_eq!(line.binding.sink.host_id, renderer.host_id);
    assert_eq!(line.binding.sink.boot_id, renderer.boot_id);
    assert_eq!(line.binding.limits.maximum_in_flight_items, 1);
    assert_eq!(
        line.binding.limits.maximum_payload_bytes,
        connection.byte_capacity
    );

    let manifestation = &snapshot.renderer.manifestation;
    assert_eq!(manifestation.lifecycle, ManifestationLifecycle::Prepared);
    assert_eq!(manifestation.plan_id, plan.plan_id);
    assert_eq!(
        manifestation.active_play_id,
        bind_active_play(&plan.plan_id, &renderer.host_id, &renderer.boot_id, 0).active_play_id
    );
    // The presented subject's Plan remains distinct from the delivery Plan that
    // realizes this renderer invocation.
    assert_ne!(
        snapshot.presentation.basis.plan_id.as_ref(),
        Some(&plan.plan_id)
    );
}
