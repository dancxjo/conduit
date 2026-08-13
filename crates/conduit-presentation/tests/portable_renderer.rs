#![cfg(feature = "form-catalog")]

use conduit_body::Body;
use conduit_core::{
    bind_active_play, kind_id, resource_offer, resource_requirement, ActivePlayId, ArtifactId,
    BootId, CapabilityId, CapabilityLimits, ExecutionProfileId, HostAdvertisement, HostId,
    HostOperationContractId, HostOperationRequirement, HostProfileId, ImplementationId,
    OfferGeneration, PlanId, SignId, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, parse, parse_syntax_document, KindSignature, ProfileCatalog,
    StartupCatalog,
};
use conduit_planner::{default_placements, plan, PlannerError};
use conduit_presentation::{
    renderer_kind_definition, renderer_offer, Manifestation, ManifestationError,
    ManifestationFailure, ManifestationLifecycle, Presentation, PresentationBasis,
    PresentationError, PresentationRelationship, PresentationRelationshipKind, PresentationRole,
    PresentationSubject, PresentationText, RendererRealizationOffer, MAX_RENDERER_VALUE_BYTES,
};

const WAYLAND_RESOURCE: &str = "conduit.resource/wayland-surface@1";
const DOM_RESOURCE: &str = "conduit.resource/browser-document@1";

fn checked_renderer_form() -> conduit_form::CheckedForm {
    let mut catalog = ProfileCatalog::new();
    catalog.insert(renderer_kind_definition()).unwrap();
    parse(
        "form 0\n\npatchbay-show {\n    renderer: presentation/renderer\n}\n",
        &catalog,
    )
    .expect("one ordinary portable renderer Face checks")
}

fn host(
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

fn plan_for(form: &conduit_form::CheckedForm, host: HostAdvertisement) -> conduit_core::Plan {
    let placements = default_placements(form, std::slice::from_ref(&host)).unwrap();
    plan(form, &[host], &placements, &[]).unwrap()
}

fn presentation(form: &conduit_form::CheckedForm, plan: &conduit_core::Plan) -> Presentation {
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
            seed_id: Some(body.seed_id),
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

#[test]
fn unchanged_face_plans_to_exact_wayland_and_dom_realizations() {
    let form = checked_renderer_form();
    let wayland = plan_for(
        &form,
        host(
            "linux-host",
            "linux-boot",
            "renderer-wayland",
            "presentation/renderer-wayland@1",
            "patchbay-native/wayland@1",
            "presentation/base/wayland-surface@1",
            WAYLAND_RESOURCE,
        ),
    );
    let html = plan_for(
        &form,
        host(
            "browser-host",
            "browser-boot",
            "renderer-dom-svg",
            "presentation/renderer-dom-svg@1",
            "patchbay-html/dom-svg@1",
            "presentation/base/dom-svg@1",
            DOM_RESOURCE,
        ),
    );

    assert_eq!(wayland.source_document_id, html.source_document_id);
    assert_eq!(wayland.checked_form_id, html.checked_form_id);
    assert_eq!(wayland.expanded_form_id, html.expanded_form_id);
    assert_ne!(wayland.plan_id, html.plan_id);
    let native = &wayland.fragments[0].placements[0];
    let browser = &html.fragments[0].placements[0];
    assert_eq!(native.kind_id, browser.kind_id);
    assert_eq!(
        native.kind_contract_revision,
        browser.kind_contract_revision
    );
    assert_eq!(native.inputs, browser.inputs);
    assert_eq!(native.outputs, browser.outputs);
    assert_ne!(native.implementation_id, browser.implementation_id);
    assert_ne!(native.artifact_id, browser.artifact_id);
    assert_ne!(native.host_id, browser.host_id);
    assert_ne!(native.boot_id, browser.boot_id);
    assert_ne!(native.host_operations, browser.host_operations);
    assert_ne!(native.resources[0].class_id, browser.resources[0].class_id);
}

#[test]
fn headless_host_is_valid_but_cannot_invent_a_renderer_offer() {
    let form = checked_renderer_form();
    let headless = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("headless"),
        boot_id: BootId::from("headless-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("headless@1"),
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    assert!(matches!(
        default_placements(&form, &[headless]),
        Err(PlannerError::UnknownCapability(_))
    ));
}

#[test]
fn renderer_face_can_be_composed_as_an_ordinary_form_back() {
    let mut startup = StartupCatalog::new();
    startup
        .insert(KindSignature {
            kind: "presentation/renderer".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let syntax = parse_syntax_document(
        "form patchbay-show (\n    > presentation: Presentation\n    manifestation: Manifestation >\n) {\n    renderer: presentation/renderer\n    presentation > renderer.presentation\n    renderer.manifestation > manifestation\n}\n",
    );
    let checked = check_syntax_document(&syntax, &startup).expect("portable renderer Back checks");
    let form = &checked.forms[0];
    assert_eq!(form.runtime_ports.len(), 2);
    assert_eq!(form.gears[0].kind, "presentation/renderer");
    assert_eq!(form.cords.len(), 2);
}

#[test]
fn manifestation_is_exact_bounded_and_fails_closed_on_stale_identity() {
    let form = checked_renderer_form();
    let plan = plan_for(
        &form,
        host(
            "linux-host",
            "linux-boot",
            "renderer-wayland",
            "presentation/renderer-wayland@1",
            "patchbay-native/wayland@1",
            "presentation/base/wayland-surface@1",
            WAYLAND_RESOURCE,
        ),
    );
    let presentation = presentation(&form, &plan);
    let placement = plan.fragments[0].placements[0].placement_id.clone();
    let active = bind_active_play(
        &plan.plan_id,
        &plan.fragments[0].host_id,
        &plan.fragments[0].boot_id,
        1,
    );
    let prepared = Manifestation::prepared(
        &presentation,
        &plan,
        active.active_play_id,
        placement,
        "linux-host/display-0".into(),
        SignId::from("manifestation/prepared"),
    )
    .unwrap();
    let available = prepared
        .transition(
            ManifestationLifecycle::Available,
            SignId::from("manifestation/available"),
        )
        .unwrap();
    assert_eq!(available.signs.len(), 2);
    assert_eq!(available.signs[1].plan_id, available.plan_id);
    assert_eq!(available.signs[1].active_play_id, available.active_play_id);
    assert_eq!(available.signs[1].placement_id, available.placement_id);
    let failed = available
        .fail(
            ManifestationFailure::OutputRejected,
            SignId::from("manifestation/failed"),
        )
        .unwrap();
    assert_eq!(failed.lifecycle, ManifestationLifecycle::Failed);
    assert_eq!(failed.failure, Some(ManifestationFailure::OutputRejected));
    assert!(failed.validate_against(&presentation, &plan).is_ok());
    let mut drifted_sign = failed.clone();
    drifted_sign.signs[1].plan_id = PlanId::from("different-plan");
    assert_eq!(
        drifted_sign.validate_against(&presentation, &plan),
        Err(ManifestationError::InvalidTransition)
    );
    let realized = available.validate_against(&presentation, &plan).unwrap();
    assert_eq!(
        realized.implementation_id.as_str(),
        "presentation/renderer-wayland@1"
    );
    assert!(matches!(
        available.transition(
            ManifestationLifecycle::Prepared,
            SignId::from("manifestation/backwards")
        ),
        Err(ManifestationError::InvalidTransition)
    ));
    assert!(matches!(
        available.transition(
            ManifestationLifecycle::Closed,
            SignId::from("manifestation/prepared")
        ),
        Err(ManifestationError::InvalidTransition)
    ));

    let other_plan = plan_for(
        &form,
        host(
            "browser-host",
            "browser-boot",
            "renderer-dom-svg",
            "presentation/renderer-dom-svg@1",
            "patchbay-html/dom-svg@1",
            "presentation/base/dom-svg@1",
            DOM_RESOURCE,
        ),
    );
    assert!(matches!(
        available.validate_against(&presentation, &other_plan),
        Err(ManifestationError::MissingRendererPlacement | ManifestationError::StaleIdentity)
    ));
}

#[test]
fn presentation_rejects_unbounded_and_drifting_semantic_content() {
    let form = checked_renderer_form();
    let plan = plan_for(
        &form,
        host(
            "browser-host",
            "browser-boot",
            "renderer-dom-svg",
            "presentation/renderer-dom-svg@1",
            "patchbay-html/dom-svg@1",
            "presentation/base/dom-svg@1",
            DOM_RESOURCE,
        ),
    );
    let valid = presentation(&form, &plan);
    assert!(valid.validate().is_ok());

    let mut drifting = valid.clone();
    drifting.subjects[0].label = "Different".into();
    assert_eq!(drifting.validate(), Err(PresentationError::InvalidIdentity));

    let mut duplicate = valid.basis.clone();
    duplicate.sign_ids.push(duplicate.sign_ids[0].clone());
    assert_eq!(
        Presentation::new(
            valid.revision,
            duplicate,
            valid.subjects.clone(),
            valid.relationships.clone(),
            valid.properties.clone(),
            valid.text.clone()
        ),
        Err(PresentationError::DuplicateSign)
    );

    let mut noncanonical = valid.clone();
    noncanonical.basis.sign_ids.push(SignId::from("aaa"));
    assert_eq!(
        noncanonical.validate(),
        Err(PresentationError::NonCanonicalSign)
    );

    let mut incoherent = valid.basis.clone();
    incoherent.plan_id = None;
    incoherent.active_play_id = Some(ActivePlayId::from("play/without-plan"));
    assert_eq!(
        Presentation::new(
            valid.revision,
            incoherent,
            valid.subjects.clone(),
            valid.relationships.clone(),
            valid.properties.clone(),
            valid.text.clone()
        ),
        Err(PresentationError::InvalidBasis)
    );

    let mut intent_only = valid.basis.clone();
    intent_only.plan_id = None;
    intent_only.active_play_id = None;
    assert!(Presentation::new(
        valid.revision,
        intent_only,
        valid.subjects.clone(),
        valid.relationships.clone(),
        valid.properties.clone(),
        valid.text.clone(),
    )
    .is_ok());

    let oversized_text = (0..513)
        .map(|_| PresentationText {
            subject: "patchbay/form".into(),
            text: "x".repeat(1_024),
        })
        .collect();
    assert_eq!(
        Presentation::new(
            valid.revision,
            valid.basis,
            valid.subjects,
            valid.relationships,
            valid.properties,
            oversized_text
        ),
        Err(PresentationError::TooManyBytes)
    );
}
