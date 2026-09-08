use conduit_body::Body;
use conduit_core::{
    ArtifactId, BootId, CapabilityId, CapabilityLimits, ExecutionProfileId, HostAdvertisement,
    HostBaseId, HostId, HostOperationContractId, HostOperationRequirement, HostProfileId,
    ImplementationId, OfferGeneration, PROTOCOL_VERSION, Plan, SignId, bind_active_play, kind_id,
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
use conduitos::native_compositor::{
    CompositorAdmission, InputRoute, NATIVE_PRESENTER_IMPLEMENTATION, NativeCompositor,
};

pub const BASE: &str = "display/base/0";
pub const MAIN: &str = "surface/main";
pub const TOP: &str = "surface/top";

pub fn delivered<T>(route: InputRoute<T>) -> T {
    match route {
        InputRoute::Delivered(value) => value,
        InputRoute::NoTarget => panic!("expected routed input"),
    }
}

pub fn update(
    compositor: &mut NativeCompositor,
    presentation: &Presentation,
    plan: &Plan,
    manifestation: &Manifestation,
    surface: &str,
) {
    compositor
        .update_surface(
            presentation,
            manifestation,
            plan,
            surface,
            &HostBaseId::from(BASE),
            &scene(),
        )
        .unwrap();
}

pub fn update_with_scene(
    compositor: &mut NativeCompositor,
    presentation: &Presentation,
    plan: &Plan,
    manifestation: &Manifestation,
    surface: &str,
    scene: &GraphicsScene,
) {
    compositor
        .update_surface(
            presentation,
            manifestation,
            plan,
            surface,
            &HostBaseId::from(BASE),
            scene,
        )
        .unwrap();
}

pub fn fixture(
    revision: u64,
) -> (
    Presentation,
    Plan,
    Manifestation,
    Manifestation,
    NativeCompositor,
) {
    let mut catalog = ProfileCatalog::new();
    catalog.insert(renderer_kind_definition()).unwrap();
    let form = parse(
        "form face {\n    renderer: presentation/renderer\n}\n",
        &catalog,
    )
    .unwrap();
    let host = native_host();
    let placements = default_placements(&form, core::slice::from_ref(&host)).unwrap();
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
        revision,
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
            text: "Input route".into(),
        }],
    )
    .unwrap();
    let placement = &plan.fragments[0].placements[0];
    let manifestation = |surface: &str, sequence: u64| {
        Manifestation::prepared(
            &presentation,
            &plan,
            bind_active_play(
                &plan.plan_id,
                &placement.host_id,
                &placement.boot_id,
                sequence,
            ),
            placement.placement_id.clone(),
            "face/main".into(),
            surface.into(),
            SignId::from(format!("prepared/{surface}")),
        )
        .unwrap()
        .transition(
            ManifestationLifecycle::Available,
            SignId::from(format!("available/{surface}")),
        )
        .unwrap()
    };
    let main = manifestation(MAIN, revision * 2);
    let top = manifestation(TOP, revision * 2 + 1);
    let admission = CompositorAdmission::new(
        placement.host_id.clone(),
        placement.boot_id.clone(),
        placement.offer_generation,
        placement.implementation_id.clone(),
        HostBaseId::from(BASE),
        vec![placement.placement_id.clone()],
        vec![MAIN.into(), TOP.into()],
    )
    .unwrap();
    let mut compositor = NativeCompositor::admitted(admission);
    compositor
        .admit_surface(
            MAIN,
            LayoutRect {
                x: 1,
                y: 1,
                width: 12,
                height: 8,
            },
            0,
        )
        .unwrap();
    compositor
        .admit_surface(
            TOP,
            LayoutRect {
                x: 5,
                y: 2,
                width: 10,
                height: 7,
            },
            1,
        )
        .unwrap();
    (presentation, plan, main, top, compositor)
}

fn scene() -> GraphicsScene {
    let bounds = LayoutRect {
        x: 0,
        y: 0,
        width: 10,
        height: 7,
    };
    let mut scene = GraphicsScene::empty();
    scene
        .push(
            GraphicsCommand::rect(
                bounds,
                bounds,
                GraphicsPaintRole::Accent,
                GraphicsShapeStyle::Fill,
            )
            .unwrap(),
        )
        .unwrap();
    scene
}

fn native_host() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("conduitos/native"),
        boot_id: BootId::from("conduitos/boot/1"),
        offer_generation: OfferGeneration(3),
        profile: HostProfileId::from("conduitos/native@1"),
        resources: vec![resource_offer(MAIN, "presentation/surface", 2)],
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
            resource_requirement: resource_requirement("presentation/surface", 1),
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: MAX_RENDERER_VALUE_BYTES,
            },
        })],
        planner_capabilities: vec![],
    }
}
