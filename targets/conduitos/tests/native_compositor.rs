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
    display::{DisplayError, DisplayFormat, PixelTarget},
    native_compositor::{
        CompositorAdmission, MAX_COMPOSITOR_SURFACES, NATIVE_PRESENTER_IMPLEMENTATION,
        NativeCompositor, NativeCompositorError,
    },
};

const SURFACE_CLASS: &str = "presentation/surface";

#[derive(Clone)]
struct MemoryDisplay {
    format: DisplayFormat,
    pixels: Vec<u32>,
    lost: bool,
}

impl MemoryDisplay {
    fn new() -> Self {
        Self {
            format: DisplayFormat {
                width: 32,
                height: 16,
                pitch: 128,
                bits_per_pixel: 32,
                red_shift: 16,
                green_shift: 8,
                blue_shift: 0,
            },
            pixels: vec![0; 32 * 16],
            lost: false,
        }
    }

    fn lost() -> Self {
        let mut display = Self::new();
        display.lost = true;
        display
    }
}

impl PixelTarget for MemoryDisplay {
    fn format(&self) -> DisplayFormat {
        self.format
    }

    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
        if self.lost {
            return Err(DisplayError::Lost);
        }
        let index = usize::try_from(y * self.format.width + x).unwrap();
        self.pixels[index] = pixel;
        Ok(())
    }
}

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
    let mut compositor = NativeCompositor::admitted(admission, MemoryDisplay::new());
    let scene = scene();
    let receipt = compositor
        .compose(
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

    assert_eq!(
        compositor.compose(
            &presentation,
            &manifestation,
            &plan,
            "surface/main",
            &HostBaseId::from("display/base/0"),
            &scene,
        ),
        Err(NativeCompositorError::SurfaceOccupied)
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
        let mut compositor = NativeCompositor::admitted(admission.clone(), MemoryDisplay::new());
        assert_eq!(
            compositor.compose(
                &presentation,
                &changed,
                &plan,
                "surface/main",
                &base,
                &scene
            ),
            Err(expected)
        );
        assert!(compositor.receipts().is_empty());
    }
    let mut compositor = NativeCompositor::admitted(admission.clone(), MemoryDisplay::new());
    assert_eq!(
        compositor.compose(
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
        compositor.compose(
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
    let mut lost = NativeCompositor::admitted(admission, MemoryDisplay::lost());
    assert_eq!(
        lost.compose(
            &presentation,
            &available,
            &plan,
            "surface/main",
            &base,
            &scene,
        ),
        Err(NativeCompositorError::Display(DisplayError::Lost))
    );
    assert!(lost.receipts().is_empty());
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
            seed_id: Some(body.seed_id),
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
