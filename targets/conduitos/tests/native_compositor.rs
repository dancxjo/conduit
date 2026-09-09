use conduit_body::Body;
use conduit_core::{
    ArtifactId, BootId, CapabilityId, CapabilityLimits, ExecutionProfileId, HostAdvertisement,
    HostBaseId, HostId, HostOperationContractId, HostOperationRequirement, HostProfileId,
    ImplementationId, OfferGeneration, PROTOCOL_VERSION, SignId, bind_active_play, kind_id,
    resource_offer, resource_requirement,
};
use conduit_form::{ProfileCatalog, parse};
use conduit_planner::{default_placements, plan};
use conduit_presentation::{
    GraphicsCommand, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle, LayoutRect,
    MAX_RENDERER_VALUE_BYTES, Manifestation, ManifestationLifecycle, Presentation,
    PresentationBasis, PresentationRole, PresentationSubject, PresentationText,
    RendererRealizationOffer, renderer_kind_definition, renderer_offer,
};
use conduitos::{
    display::DisplayError,
    native_compositor::{
        CompositorAdmission, MAX_COMPOSITOR_SURFACES, NATIVE_PRESENTER_IMPLEMENTATION,
        NativeCompositor, NativeCompositorError,
    },
};

#[path = "common/native_compositor.rs"]
mod support;
#[path = "common/native_text_damage.rs"]
mod text_damage;
use support::MemoryDisplay;

const SURFACE_CLASS: &str = "presentation/surface";

#[test]
fn admitted_native_compositor_binds_exact_manifestation_and_scanout() {
    let (presentation, plan, mut manifestation) = specimen();
    manifestation = manifestation
        .transition(
            ManifestationLifecycle::Available,
            SignId::from("manifestation/available"),
        )
        .unwrap();
    let base = HostBaseId::from("display/base/0");
    let admission = CompositorAdmission::new(
        manifestation.host_id.clone(),
        manifestation.boot_id.clone(),
        manifestation.offer_generation,
        manifestation.presenter_implementation_id.clone(),
        base.clone(),
        vec![manifestation.placement_id.clone()],
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
                width: 32,
                height: 16,
            },
            0,
        )
        .unwrap();
    let scene = scene();
    let receipt = compositor
        .update_surface(
            &presentation,
            &manifestation,
            &plan,
            "surface/main",
            &base,
            &scene,
        )
        .unwrap();
    assert_eq!(receipt.presentation_id, presentation.identity);
    assert_eq!(receipt.manifestation_id, manifestation.manifestation_id);
    assert_eq!(receipt.plan_id, plan.plan_id);
    assert_eq!(receipt.active_play_id, manifestation.active_play_id);
    assert_eq!(receipt.play_sequence, manifestation.play_sequence);
    assert_eq!(receipt.host_id, manifestation.host_id);
    assert_eq!(receipt.boot_id, manifestation.boot_id);
    assert_eq!(receipt.display_base_id, base);
    assert_eq!(receipt.display.commands, 1);
    assert!(receipt.display.pixels_written > 0);
    let mut display = MemoryDisplay::new();
    let frame = compositor.compose_frame(&mut display).unwrap();
    assert_eq!(frame.frame_sequence, 1);
    assert_eq!(frame.surfaces_composed, 1);
    assert_ne!(display.pixels[33], 0);
    assert_eq!(
        compositor.update_surface(
            &presentation,
            &manifestation,
            &plan,
            "surface/main",
            &HostBaseId::from("display/base/0"),
            &scene,
        ),
        Err(NativeCompositorError::StaleSurfaceRevision)
    );
}

#[test]
fn admission_and_exact_identity_fail_closed_before_scanout() {
    let (presentation, plan, manifestation) = specimen();
    let base = HostBaseId::from("display/base/0");
    assert_eq!(
        CompositorAdmission::new(
            manifestation.host_id.clone(),
            manifestation.boot_id.clone(),
            manifestation.offer_generation,
            manifestation.presenter_implementation_id.clone(),
            base.clone(),
            vec![manifestation.placement_id.clone(); MAX_COMPOSITOR_SURFACES + 1],
            vec!["surface/main".into()],
        ),
        Err(NativeCompositorError::TooManySurfaces)
    );
    assert_eq!(
        CompositorAdmission::new(
            manifestation.host_id.clone(),
            manifestation.boot_id.clone(),
            manifestation.offer_generation,
            manifestation.presenter_implementation_id.clone(),
            base.clone(),
            vec![manifestation.placement_id.clone()],
            vec!["surface/main".into(), "surface/main".into()],
        ),
        Err(NativeCompositorError::DuplicateSurface)
    );
    let admission = CompositorAdmission::new(
        manifestation.host_id.clone(),
        manifestation.boot_id.clone(),
        manifestation.offer_generation,
        manifestation.presenter_implementation_id.clone(),
        base.clone(),
        vec![manifestation.placement_id.clone()],
        vec!["surface/main".into()],
    )
    .unwrap();
    let scene = scene();
    for (changed, expected) in [
        (
            {
                let mut value = manifestation.clone();
                value.boot_id = BootId::from("stale-boot");
                value
            },
            NativeCompositorError::StaleIdentity,
        ),
        (
            {
                let mut value = manifestation.clone();
                value.offer_generation.0 += 1;
                value
            },
            NativeCompositorError::StaleIdentity,
        ),
    ] {
        let mut compositor = NativeCompositor::admitted(admission.clone());
        compositor
            .admit_surface("surface/main", surface_bounds(), 0)
            .unwrap();
        assert_eq!(
            compositor.update_surface(
                &presentation,
                &changed,
                &plan,
                "surface/main",
                &base,
                &scene
            ),
            Err(expected)
        );
        assert_eq!(compositor.receipts().count(), 0);
    }
    let mut compositor = NativeCompositor::admitted(admission.clone());
    compositor
        .admit_surface("surface/main", surface_bounds(), 0)
        .unwrap();
    assert_eq!(
        compositor.update_surface(
            &presentation,
            &manifestation,
            &plan,
            "surface/unadmitted",
            &base,
            &scene,
        ),
        Err(NativeCompositorError::UnadmittedSurface)
    );
    assert_eq!(
        compositor.update_surface(
            &presentation,
            &manifestation,
            &plan,
            "surface/main",
            &HostBaseId::from("display/base/wrong"),
            &scene,
        ),
        Err(NativeCompositorError::StaleIdentity)
    );

    let available = manifestation
        .transition(
            ManifestationLifecycle::Available,
            SignId::from("manifestation/lost-display"),
        )
        .unwrap();
    let mut lost = NativeCompositor::admitted(admission);
    lost.admit_surface("surface/main", surface_bounds(), 0)
        .unwrap();
    lost.update_surface(
        &presentation,
        &available,
        &plan,
        "surface/main",
        &base,
        &scene,
    )
    .unwrap();
    assert_eq!(
        lost.compose_frame(&mut MemoryDisplay::lost()),
        Err(NativeCompositorError::Display(DisplayError::Lost))
    );
    assert_eq!(lost.receipts().count(), 1);
}

#[test]
fn retained_surfaces_update_independently_and_compose_by_geometry_and_z() {
    let (presentation, plan, manifestation) = specimen();
    let first = manifestation
        .transition(
            ManifestationLifecycle::Available,
            SignId::from("available/first"),
        )
        .unwrap();
    let active = bind_active_play(
        &plan.plan_id,
        &manifestation.host_id,
        &manifestation.boot_id,
        2,
    );
    let second = Manifestation::prepared(
        &presentation,
        &plan,
        active,
        manifestation.placement_id.clone(),
        "face/main".into(),
        "surface/overlay".into(),
        SignId::from("prepared/second"),
    )
    .unwrap()
    .transition(
        ManifestationLifecycle::Available,
        SignId::from("available/second"),
    )
    .unwrap();
    let base = HostBaseId::from("display/base/0");
    let admission = CompositorAdmission::new(
        manifestation.host_id.clone(),
        manifestation.boot_id.clone(),
        manifestation.offer_generation,
        manifestation.presenter_implementation_id.clone(),
        base.clone(),
        vec![manifestation.placement_id.clone()],
        vec!["surface/main".into(), "surface/overlay".into()],
    )
    .unwrap();
    let mut compositor = NativeCompositor::admitted(admission);
    compositor
        .admit_surface(
            "surface/main",
            LayoutRect {
                x: 1,
                y: 1,
                width: 8,
                height: 4,
            },
            0,
        )
        .unwrap();
    compositor
        .admit_surface(
            "surface/overlay",
            LayoutRect {
                x: 5,
                y: 2,
                width: 8,
                height: 4,
            },
            1,
        )
        .unwrap();
    compositor
        .update_surface(
            &presentation,
            &first,
            &plan,
            "surface/main",
            &base,
            &colored_scene(GraphicsPaintRole::Accent),
        )
        .unwrap();
    compositor
        .update_surface(
            &presentation,
            &second,
            &plan,
            "surface/overlay",
            &base,
            &colored_scene(GraphicsPaintRole::Status),
        )
        .unwrap();

    let mut display = MemoryDisplay::new();
    let first_frame = compositor.compose_frame(&mut display).unwrap();
    assert_eq!(first_frame.surfaces_composed, 2);
    assert_eq!(display.pixels[1 + 32], 0x53b2ff);
    assert_eq!(display.pixels[5 + 2 * 32], 0xffbe46);

    compositor
        .set_surface_visible("surface/overlay", false)
        .unwrap();
    let second_frame = compositor.compose_frame(&mut display).unwrap();
    assert_eq!(second_frame.frame_sequence, 2);
    assert_eq!(display.pixels[5 + 2 * 32], 0x53b2ff);
    assert_eq!(compositor.receipts().count(), 2);
}

#[test]
fn removed_and_resized_surfaces_reuse_finite_backing_storage() {
    let (_, _, manifestation) = specimen();
    let admission = CompositorAdmission::new(
        manifestation.host_id,
        manifestation.boot_id,
        manifestation.offer_generation,
        manifestation.presenter_implementation_id,
        HostBaseId::from("display/base/0"),
        vec![manifestation.placement_id],
        vec!["surface/main".into()],
    )
    .unwrap();
    let mut compositor = NativeCompositor::admitted(admission);
    compositor
        .admit_surface("surface/main", surface_bounds(), 0)
        .unwrap();
    assert_eq!(compositor.allocated_pixels(), 32 * 16);

    compositor.remove_surface("surface/main").unwrap();
    compositor
        .admit_surface("surface/main", surface_bounds(), 0)
        .unwrap();
    assert_eq!(compositor.allocated_pixels(), 32 * 16);

    let wide = LayoutRect {
        width: 64,
        height: 8,
        ..surface_bounds()
    };
    compositor.place_surface("surface/main", wide, 0).unwrap();
    assert_eq!(compositor.allocated_pixels(), 2 * 32 * 16);
    compositor
        .place_surface("surface/main", surface_bounds(), 0)
        .unwrap();
    assert_eq!(compositor.allocated_pixels(), 2 * 32 * 16);
}

fn surface_bounds() -> LayoutRect {
    LayoutRect {
        x: 0,
        y: 0,
        width: 32,
        height: 16,
    }
}

fn colored_scene(paint: GraphicsPaintRole) -> GraphicsScene {
    let rect = LayoutRect {
        x: 0,
        y: 0,
        width: 8,
        height: 4,
    };
    let mut scene = GraphicsScene::empty();
    scene
        .push(GraphicsCommand::rect(rect, rect, paint, GraphicsShapeStyle::Fill).unwrap())
        .unwrap();
    scene
}

fn specimen() -> (Presentation, conduit_core::Plan, Manifestation) {
    let mut catalog = ProfileCatalog::new();
    catalog.insert(renderer_kind_definition()).unwrap();
    let form = parse(
        "form face {\n    renderer: presentation/renderer\n}\n",
        &catalog,
    )
    .unwrap();
    let host = native_host();
    let placements = default_placements(&form, std::slice::from_ref(&host)).unwrap();
    let plan = plan(&form, &[host], &placements, &[]).unwrap();
    let body = Body::born(
        form.source_document_id.clone(),
        form.checked_form_id.clone(),
        1,
        SignId::from("body/born"),
    )
    .unwrap();
    let (body, wake) = body.wake(1, SignId::from("body/wake")).unwrap();
    let presentation = Presentation::new(
        1,
        PresentationBasis {
            body_id: Some(body.body_id),
            wake_id: Some(wake.wake_id),
            source_document_id: Some(form.source_document_id),
            checked_form_id: Some(form.checked_form_id),
            expanded_form_id: Some(form.expanded_form_id),
            plan_id: Some(plan.plan_id.clone()),
            active_play_id: None,
            sign_ids: vec![SignId::from("presentation/source")],
        },
        vec![PresentationSubject {
            identity: "face/main".into(),
            role: PresentationRole::Form,
            label: "Main".into(),
            accessibility_name: "Main face".into(),
        }],
        vec![],
        vec![],
        vec![PresentationText {
            subject: "face/main".into(),
            text: "Native compositor".into(),
        }],
    )
    .unwrap();
    let placement = &plan.fragments[0].placements[0];
    let active = bind_active_play(&plan.plan_id, &placement.host_id, &placement.boot_id, 1);
    let manifestation = Manifestation::prepared(
        &presentation,
        &plan,
        active,
        placement.placement_id.clone(),
        "face/main".into(),
        "surface/main".into(),
        SignId::from("manifestation/prepared"),
    )
    .unwrap();
    (presentation, plan, manifestation)
}

fn native_host() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("conduitos/native"),
        boot_id: BootId::from("conduitos/boot/1"),
        offer_generation: OfferGeneration(3),
        profile: HostProfileId::from("conduitos/native@1"),
        resources: vec![resource_offer("surface/main", SURFACE_CLASS, 1)],
        capabilities: vec![renderer_offer(RendererRealizationOffer {
            capability_id: CapabilityId::from("presenter/native"),
            execution_profile_id: ExecutionProfileId::from("conduitos/native@1"),
            implementation_id: ImplementationId::from(NATIVE_PRESENTER_IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduitos/native-image@1"),
            host_operation: HostOperationRequirement {
                contract_id: HostOperationContractId::from("conduit.host/present@1"),
                target_kind: Some(kind_id("presentation/base/native-compositor@1")),
                maximum_in_flight: 1,
                maximum_input_bytes: MAX_RENDERER_VALUE_BYTES,
                maximum_output_bytes: MAX_RENDERER_VALUE_BYTES,
            },
            resource_requirement: resource_requirement(SURFACE_CLASS, 1),
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: MAX_RENDERER_VALUE_BYTES,
            },
        })],
        planner_capabilities: vec![],
    }
}

fn scene() -> GraphicsScene {
    let rect = LayoutRect {
        x: 1,
        y: 1,
        width: 8,
        height: 4,
    };
    let mut scene = GraphicsScene::empty();
    scene
        .push(
            GraphicsCommand::rect(
                rect,
                rect,
                GraphicsPaintRole::Accent,
                GraphicsShapeStyle::Fill,
            )
            .unwrap(),
        )
        .unwrap();
    scene
}
