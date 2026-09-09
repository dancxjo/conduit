use super::*;

#[test]
fn a_text_revision_damages_only_its_retained_surface() {
    let (original, plan, prepared) = specimen();
    let base = HostBaseId::from("display/base/0");
    let admission = CompositorAdmission::new(
        prepared.host_id.clone(),
        prepared.boot_id.clone(),
        prepared.offer_generation,
        prepared.presenter_implementation_id.clone(),
        base.clone(),
        vec![prepared.placement_id.clone()],
        vec!["surface/main".into()],
    )
    .unwrap();
    let mut compositor = NativeCompositor::admitted(admission);
    compositor
        .admit_surface(
            "surface/main",
            LayoutRect {
                x: 0,
                y: 0,
                width: 16,
                height: 16,
            },
            0,
        )
        .unwrap();
    let mut display = MemoryDisplay::new();
    let mut previous = Vec::new();
    for (revision, text) in [(1, "i"), (2, "W")] {
        let presentation = Presentation::new(
            revision,
            original.basis.clone(),
            original.subjects.clone(),
            vec![],
            vec![],
            vec![PresentationText {
                subject: "face/main".into(),
                text: text.into(),
            }],
        )
        .unwrap();
        let active = bind_active_play(&plan.plan_id, &prepared.host_id, &prepared.boot_id, 1);
        let manifestation = Manifestation::prepared(
            &presentation,
            &plan,
            active,
            prepared.placement_id.clone(),
            "face/main".into(),
            "surface/main".into(),
            SignId::from("text/prepared"),
        )
        .unwrap()
        .transition(
            ManifestationLifecycle::Available,
            SignId::from("text/available"),
        )
        .unwrap();
        let bounds = LayoutRect {
            x: 0,
            y: 0,
            width: 16,
            height: 16,
        };
        let mut scene = GraphicsScene::empty();
        scene
            .push(
                GraphicsCommand::rect(
                    bounds,
                    bounds,
                    GraphicsPaintRole::Background,
                    GraphicsShapeStyle::Fill,
                )
                .unwrap(),
            )
            .unwrap();
        scene
            .push(
                GraphicsCommand::text(bounds, bounds, GraphicsPaintRole::Foreground, text)
                    .unwrap()
                    .with_text_role(conduit_presentation::GraphicsTextRole::Label)
                    .unwrap(),
            )
            .unwrap();
        compositor
            .update_surface(
                &presentation,
                &manifestation,
                &plan,
                "surface/main",
                &base,
                &scene,
            )
            .unwrap();
        let frame = compositor.compose_frame(&mut display).unwrap();
        if revision == 2 {
            assert!(!frame.conservative_fallback);
            assert_eq!(frame.pixels_written, 16 * 16);
            assert!(frame.pixels_written < display.format.width * display.format.height);
            assert_ne!(display.pixels, previous);
            for y in 0..16 {
                for x in 16..32 {
                    assert_eq!(display.pixels[y * 32 + x], previous[y * 32 + x]);
                }
            }
        }
        previous.clone_from(&display.pixels);
    }
}
