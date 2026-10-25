//! Production owner-entry/producer/Mask/compositor proof with CPU membership,
//! Line and lifecycle fixtures. No installed Host, live transport or physical
//! display/Body workload execution is claimed by these fixtures.
use super::*;
use crate::display::RetainedPixelTarget;
use crate::native_face_scene::raster_layout_tests::Pixels;
use alloc::vec;
use conduit_body::{BodyPlayIdentity, BodyPlotPlan, ResidentPlot};
use conduit_core::*;
use conduit_face::*;
mod common {
    use alloc::string::String;
    include!("../../../../bodies/environment/history/tests/common/mod.rs");
}

fn setup() -> (
    NativeFaceMask,
    Face,
    RemoteOwnerMaskRouteSeal,
    crate::native_surface_provider::NativeSurfaceProvider,
) {
    let provider = crate::product_bases::fixture_surface_provider();
    let host = crate::product_bases::native_mask_host_advertisement(
        &"host/history-owner-cpu".into(),
        &"boot/history-owner-cpu".into(),
        1,
        &provider,
    );
    let (stage, _) =
        conduit_conduitos_mask_offer::prepare_history_component_stage_from_offer(&host).unwrap();
    let resident = ResidentPlot::new(
        stage.planned_mask.plan.source_document_id.clone(),
        stage.planned_mask.plan.checked_plot_id.clone(),
    );
    let partition = BodyPlotPlan {
        plot: resident,
        plan: stage.planned_mask.plan.clone(),
    };
    // Shared fixture establishes lifecycle/membership, never executes a Body
    // workload. The actual production Mask kernels execute below in present().
    let mut session = crate::protocol_test_support::protocol_body_session(
        partition,
        &host.host_id,
        &host.boot_id,
    );
    let realization = session.realization().unwrap().clone();
    let active = BodyPlayIdentity::bind(&realization.plan, 1);
    let sign = |n| {
        bind_sign(
            &host.host_id,
            &host.boot_id,
            Some(&active.active_play_id),
            n,
        )
        .sign_id
    };
    let wake = realization
        .wake
        .body_plan_ready(&realization.plan, sign(0))
        .unwrap()
        .body_play_started(&realization.plan, &active, sign(1))
        .unwrap();
    session
        .started(&host.host_id, &host.boot_id, active.clone(), wake)
        .unwrap();
    let mut history = common::prepared(1);
    history
        .read_next(&common::chunk(&[
            (1, 1000, true),
            (2, 2000, false),
            (4, 11000, true),
        ]))
        .unwrap();
    let fragment = history
        .contribution(FaceContributionBasis {
            checked_plot_id: stage.planned_mask.plan.checked_plot_id.clone(),
            plan_id: realization.plan.plan_id.clone(),
            active_play_id: active.active_play_id,
            ..common::basis()
        })
        .unwrap();
    let face = BodyFace::project(
        &session.evidence().body,
        Some(&session.realization().unwrap().wake),
        1,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![FaceContribution::from_face(
            FaceContributionRole::Foreground,
            fragment,
        )],
    )
    .unwrap()
    .face;
    assert!(face.subjects.iter().any(|s| s.role == FaceRole::Body));
    assert!(face.subjects.iter().any(|s| s.role == FaceRole::Region));
    assert!(!face.actions.is_empty());
    let owner = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "host/history-owner-cpu/owner".into(),
        boot_id: "boot/history-owner-cpu/owner".into(),
        offer_generation: OfferGeneration(1),
        profile: "fixture/owner".into(),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vec![],
    };
    let line = |name: &str, source: &HostAdvertisement, sink: &HostAdvertisement| {
        let mut line = process_owned_line_offer_with_limits(
            &format!("line/history/{name}"),
            &format!("binding/history/{name}"),
            "conduit.base/websocket-rfc6455@1".into(),
            "fixture/history-line",
            source,
            sink,
            LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 64 * 1024,
                maximum_buffered_bytes: 128 * 1024,
                maximum_frame_bytes: 64 * 1024,
            },
        );
        line.binding.credential = LinkCredentialReference::Opaque("credential/history-cpu".into());
        line.binding.authority = LinkAuthorityReference::Grant("authority/history-cpu".into());
        line.contract.scope = LineScope::RoutedNetwork;
        line.contract.security = LineSecurity::PlaintextNetwork;
        line
    };
    let route = RemoteOwnerMaskRouteSeal::seal_current(
        &session,
        &face,
        &owner,
        &host,
        &stage.planned_mask,
        &line("face", &owner, &host),
        &line("return", &host, &owner),
    )
    .unwrap();
    let mask = prepare(&provider, &route).unwrap();
    (mask, face, route, provider)
}
fn prepare(
    provider: &crate::native_surface_provider::NativeSurfaceProvider,
    route: &RemoteOwnerMaskRouteSeal,
) -> Result<NativeFaceMask, NativeFaceMaskError> {
    NativeFaceMask::prepare_with_owner_route(
        "host/history-owner-cpu".into(),
        "boot/history-owner-cpu".into(),
        OfferGeneration(1),
        "build",
        provider.entry.base_id.clone(),
        "surface/history-owner-cpu",
        provider,
        Some(route),
    )
}
fn key(mask: &mut NativeFaceMask, usage: u8, seq: u64, pixels: &mut Pixels) -> NativeFaceMaskInput {
    mask.key(
        conduit_human::KeyEvent::new(
            usage,
            conduit_human::KeyTransition::Pressed,
            conduit_human::KeyModifiers::NONE,
        )
        .unwrap(),
        seq,
        pixels,
    )
    .unwrap()
}

fn export_pixels(pixels: &Pixels, name: &str, face: &Face) {
    let Some(directory) = std::env::var_os("CONDUIT_HISTORY_NATIVE_PROOF_OUTPUT") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory).unwrap();
    let extent = pixels.format();
    let mut ppm = format!("P6\n{} {}\n255\n", extent.width, extent.height).into_bytes();
    for y in 0..extent.height {
        for x in 0..extent.width {
            let pixel = pixels.read_pixel(x, y).unwrap();
            ppm.extend_from_slice(&[(pixel >> 16) as u8, (pixel >> 8) as u8, pixel as u8]);
        }
    }
    std::fs::write(directory.join(format!("{name}.ppm")), ppm).unwrap();
    std::fs::write(
        directory.join("native-owner-face.json"),
        serde_json::to_vec_pretty(face).unwrap(),
    )
    .unwrap();
}

#[test]
fn actual_body_face_owner_entry_graph_and_complete_inspector_share_one_acknowledged_session() {
    let (mut mask, face, _, _) = setup();
    let original = face.clone();
    let graph = conduit_environment_history::HistoryGraph::prepare(
        &face,
        conduit_environment_history::HISTORY_SUBJECT,
    )
    .unwrap();
    let mut pixels = Pixels::new(640, 480);
    let receipt = mask.present(face.clone(), 1, 1, &mut pixels).unwrap();
    assert!(mask.history_visible);
    export_pixels(&pixels, "native-owner-graph", &face);
    assert!(mask.component_fallback().is_none());
    assert_eq!(mask.publication().unwrap().face_id, face.identity.as_str());
    assert!(mask.publication().unwrap().kernel_signs > 0);
    assert!(mask.render_receipt().unwrap().kernel_signs > 0);
    assert!(mask.render_receipt().unwrap().scanout_pixels_written > 0);
    let show = mask.show().unwrap().clone();
    assert_eq!(show.show.lifecycle, ManifestationLifecycle::Available);
    let first = &graph.samples()[0];
    let failed = &graph.samples()[1];
    let y = graph.y(first.temperature_milli_c().unwrap(), false) + 1;
    let point = pixels
        .read_pixel(u32::from(graph.x(first) + 1), u32::from(y))
        .unwrap();
    let blank = pixels
        .read_pixel(u32::from(graph.x(failed) + 1), u32::from(y))
        .unwrap();
    assert_ne!(point, blank);
    assert!(matches!(
        mask.pointer(u32::from(graph.x(first)), u32::from(y), 1, &mut pixels)
            .unwrap(),
        NativeFaceMaskInput::Unchanged
    ));
    assert!(matches!(
        key(&mut mask, 0x28, 2, &mut pixels),
        NativeFaceMaskInput::Unchanged
    ));
    assert!(mask.show().is_some());
    assert!(matches!(
        key(&mut mask, 0x3b, 3, &mut pixels),
        NativeFaceMaskInput::Redrawn(_)
    ));
    assert!(!mask.history_visible);
    export_pixels(&pixels, "native-owner-inspector", &face);
    assert_eq!(mask.show().unwrap().show_id, show.show_id);
    assert_eq!(mask.scene().unwrap().face(), &original);
    let scene = mask.scene.as_mut().unwrap();
    let mut saw_context = false;
    let mut saw_history = false;
    scene.show_details(true);
    for _ in 0..scene.pages() {
        let frame = scene.frame().unwrap();
        saw_context |= frame
            .scene
            .commands()
            .iter()
            .any(|c| c.payload().contains("operator-context"));
        saw_history |= frame
            .scene
            .commands()
            .iter()
            .any(|c| c.payload().contains("sampled-at-boot-ms"));
        scene.turn_page(true);
    }
    assert!(
        saw_context && saw_history,
        "same-Face inspector must retain Body/context and complete recording facts"
    );
    assert!(matches!(
        key(&mut mask, 0x2b, 4, &mut pixels),
        NativeFaceMaskInput::Redrawn(_)
    ));
    assert!(
        mask.scene().unwrap().focused().is_some(),
        "Body actions remain reachable through real inspector keyboard input"
    );
    assert!(matches!(
        key(&mut mask, 0x3b, 5, &mut pixels),
        NativeFaceMaskInput::Redrawn(_)
    ));
    assert!(mask.history_visible);
    assert_eq!(mask.show().unwrap().show_id, show.show_id);
    assert_eq!(mask.show().unwrap().show.face_id, face.identity);
    assert_eq!(mask.render_receipt().unwrap().mask_plan_id, receipt.plan_id);
    let mut stale_face = face.clone();
    stale_face.revision += 1;
    assert!(mask.present(stale_face, 2, 2, &mut pixels).is_err());
    assert_eq!(mask.show().unwrap().show_id, show.show_id);
}

#[test]
fn invalid_history_and_unreachable_inspector_fall_back_without_dropping_the_face() {
    let (mut mask, mut face, route, provider) = setup();
    let p = face
        .properties
        .iter_mut()
        .find(|p| p.subject == "environment-history" && p.name == "clock")
        .unwrap();
    p.value = FacePropertyValue::Text("wall-clock".into());
    face.reseal_prepared().unwrap();
    let mut pixels = Pixels::new(640, 480);
    mask.present(face.clone(), 1, 1, &mut pixels).unwrap();
    assert!(!mask.history_visible);
    assert!(mask.history_scene.is_none());
    assert!(
        mask.component_fallback()
            .unwrap()
            .starts_with("HistoryGraph")
    );
    assert_eq!(mask.scene().unwrap().face(), &face);
    let mut no_input = prepare(&provider, &route)
        .unwrap()
        .without_local_input()
        .unwrap();
    no_input.present(face.clone(), 1, 1, &mut pixels).unwrap();
    assert_eq!(
        no_input.component_fallback(),
        Some("HistoryInspectorInputUnavailable")
    );
    assert!(!no_input.history_visible);
    assert_eq!(no_input.scene().unwrap().face(), &face);
    let mut stale = route.clone();
    stale.mask_host.boot_id = "boot/replaced".into();
    assert!(prepare(&provider, &stale).is_err());
    let mut foreign = route;
    foreign.planned_mask.mask.plot_name = "other-history-root".into();
    assert!(prepare(&provider, &foreign).is_err());
}

#[test]
fn selected_history_inspector_survives_a_fresh_mask_publication() {
    let (mut mask, face, _, _) = setup();
    let mut pixels = Pixels::new(640, 480);
    mask.present(face.clone(), 1, 1, &mut pixels).unwrap();
    key(&mut mask, 0x3b, 1, &mut pixels);
    assert!(!mask.history_visible);
    let previous = mask.show().unwrap().show_id.clone();
    let mut refreshed = face;
    refreshed.revision += 1;
    refreshed.reseal_prepared().unwrap();
    mask.present(refreshed.clone(), 2, 2, &mut pixels).unwrap();
    assert!(!mask.history_visible);
    assert!(mask.history_scene.is_some());
    assert_eq!(mask.scene().unwrap().face(), &refreshed);
    assert_ne!(mask.show().unwrap().show_id, previous);
    key(&mut mask, 0x3b, 2, &mut pixels);
    assert!(mask.history_visible);
}
