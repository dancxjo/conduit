#![allow(dead_code)]

use conduit_body::Body;
use conduit_core::{
    kind_id, resource_offer, resource_requirement, ArtifactId, BootId, CapabilityId,
    CapabilityLimits, ExecutionProfileId, HostAdvertisement, HostId, HostOperationContractId,
    HostOperationRequirement, HostProfileId, ImplementationId, OfferGeneration, SignId,
    PROTOCOL_VERSION,
};
use conduit_form::{parse, ProfileCatalog};
use conduit_planner::{default_placements, plan};
use conduit_presentation::{
    renderer_kind_definition, renderer_offer, Presentation, PresentationBasis,
    PresentationRelationship, PresentationRelationshipKind, PresentationRole, PresentationSubject,
    PresentationText, RendererRealizationOffer, MAX_RENDERER_VALUE_BYTES,
};

pub const WAYLAND_RESOURCE: &str = "conduit.resource/wayland-surface@1";
pub const DOM_RESOURCE: &str = "conduit.resource/browser-document@1";

pub fn checked_renderer_form() -> conduit_form::CheckedForm {
    let mut catalog = ProfileCatalog::new();
    catalog.insert(renderer_kind_definition()).unwrap();
    parse(
        "form patchbay-show {\n    renderer: presentation/renderer\n}\n",
        &catalog,
    )
    .expect("one ordinary portable renderer Face checks")
}

pub fn host(
    host: &str,
    boot: &str,
    capability: &str,
    implementation: &str,
    artifact: &str,
    target: &str,
    resource_class: &str,
) -> HostAdvertisement {
    let limits = CapabilityLimits {
        max_active_instances: 1,
        max_queue_items: 1,
        max_queue_bytes: MAX_RENDERER_VALUE_BYTES,
    };
    let pool_id = format!("{host}/surface");
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("presentation/host@1"),
        resources: vec![resource_offer(&pool_id, resource_class, 1)],
        capabilities: vec![renderer_offer(RendererRealizationOffer {
            capability_id: CapabilityId::from(capability),
            execution_profile_id: ExecutionProfileId::from("presentation/renderer-hosted@1"),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(artifact),
            host_operation: HostOperationRequirement {
                contract_id: HostOperationContractId::from("conduit.host/present@1"),
                target_kind: Some(kind_id(target)),
                maximum_in_flight: 1,
                maximum_input_bytes: MAX_RENDERER_VALUE_BYTES,
                maximum_output_bytes: MAX_RENDERER_VALUE_BYTES,
            },
            resource_requirement: resource_requirement(resource_class, 1),
            limits,
        })],
        planner_capabilities: vec![],
    }
}

pub fn plan_for(form: &conduit_form::CheckedForm, host: HostAdvertisement) -> conduit_core::Plan {
    let placements = default_placements(form, std::slice::from_ref(&host)).unwrap();
    plan(form, &[host], &placements, &[]).unwrap()
}

pub fn presentation(form: &conduit_form::CheckedForm, plan: &conduit_core::Plan) -> Presentation {
    let body = Body::born(
        form.source_document_id.clone(),
        form.checked_form_id.clone(),
        1,
        SignId::from("patchbay/sign/bornd"),
    )
    .unwrap();
    let (body, wake) = body.wake(1, SignId::from("patchbay/sign/woke")).unwrap();
    Presentation::new(
        7,
        PresentationBasis {
            body_id: Some(body.body_id),
            wake_id: Some(wake.wake_id),
            source_document_id: Some(form.source_document_id.clone()),
            checked_form_id: Some(form.checked_form_id.clone()),
            expanded_form_id: Some(form.expanded_form_id.clone()),
            plan_id: Some(plan.plan_id.clone()),
            active_play_id: None,
            sign_ids: vec![SignId::from("patchbay/sign/source")],
        },
        vec![
            PresentationSubject {
                identity: "patchbay/form".into(),
                role: PresentationRole::Form,
                label: "Patchbay".into(),
                accessibility_name: "Patchbay Form".into(),
            },
            PresentationSubject {
                identity: "patchbay/renderer".into(),
                role: PresentationRole::Gear,
                label: "Renderer".into(),
                accessibility_name: "Portable presentation renderer".into(),
            },
        ],
        vec![PresentationRelationship {
            source: "patchbay/form".into(),
            target: "patchbay/renderer".into(),
            kind: PresentationRelationshipKind::Contains,
        }],
        vec![],
        vec![PresentationText {
            subject: "patchbay/renderer".into(),
            text: "Presentation to Manifestation".into(),
        }],
    )
    .unwrap()
}
