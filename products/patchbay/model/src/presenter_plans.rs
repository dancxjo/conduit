//! One canonical Patchbay meaning with direct and ordinary Back realizations.

use conduit_core::{
    resource_offer, BootId, HostAdvertisement, HostId, HostProfileId, OfferGeneration, Plan,
    SignId, PRESENTATION_RESOURCE_CLASS, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, expand_canonical_form_with_backs,
    parse_syntax_document, CanonicalBackCatalog, ProfileCatalog, StartupCatalog,
};

pub use conduit_semantic_catalog::PATCHBAY_PRESENTATION_KIND;

const USER_SOURCE: &str = "form patchbay-capstone {\n subject: text/literal(\"Gear demo with typed Ports and one Cord\")\n canvas: presentation/patchbay\n subject > canvas.subject\n}\n";
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchbayPresenterPlans {
    pub direct_expanded: conduit_form::ExpandedCanonicalForm,
    pub recursive_expanded: conduit_form::ExpandedCanonicalForm,
    pub direct_host: HostAdvertisement,
    pub recursive_host: HostAdvertisement,
    pub direct: Plan,
    pub recursive: Plan,
}

pub fn patchbay_presenter_plans() -> Result<PatchbayPresenterPlans, String> {
    let (startup, profile) = catalogs()?;
    let checked = check_syntax_document(&parse_syntax_document(USER_SOURCE), &startup)
        .map_err(|error| format!("check Patchbay specimen: {error:?}"))?;
    let direct_expanded = expand_canonical_form(&checked, "patchbay-capstone", &profile)
        .map_err(|error| error.to_string())?;
    let backs = backs(&startup, &profile)?;
    let recursive_expanded =
        expand_canonical_form_with_backs(&checked, "patchbay-capstone", &profile, &backs)
            .map_err(|error| error.to_string())?;
    let direct_host = direct_host();
    let recursive_host = recursive_host();
    let direct = plan(&direct_expanded, &direct_host)?;
    let recursive = plan(&recursive_expanded, &recursive_host)?;
    Ok(PatchbayPresenterPlans {
        direct_expanded,
        recursive_expanded,
        direct_host,
        recursive_host,
        direct,
        recursive,
    })
}

/// Production Patchbay input for inspecting the ordinary recursive
/// realization of the Patchbay presentation Face itself.
pub fn recursive_form_demonstration() -> Result<conduit_presentation::Presentation, String> {
    let proof = patchbay_presenter_plans()?;
    let (startup, profile) = catalogs()?;
    let editor = crate::FormEditor::from_source_with_catalogs(
        "patchbay-recursive-form.conduit".into(),
        USER_SOURCE.into(),
        startup.clone(),
        profile,
    )
    .map_err(|error| error.to_string())?;
    let mut graph = crate::PatchbayGraph::from_expanded(&proof.recursive_expanded)
        .map_err(|error| error.to_string())?;
    for back in &proof.recursive_expanded.realization_backs {
        let face = reviewed_back_face(back, &startup)?;
        let projection = crate::project_recursive_form_gear(
            &proof.recursive_expanded,
            &back.invocation_path,
            face,
            false,
        )
        .map_err(|error| format!("recursive Form projection: {error:?}"))?;
        graph
            .admit_recursive_form(&projection)
            .map_err(|error| error.to_string())?;
    }
    let request = crate::PatchbayRequestId::new("patchbay/recursive-form-plan")
        .map_err(|error| format!("{error:?}"))?;
    let plan = crate::PlanDocument::from_plan(request, &proof.recursive)
        .map_err(|error| format!("{error:?}"))?;
    let body = conduit_body::Body::born(
        proof.recursive.source_document_id.clone(),
        proof.recursive.checked_form_id.clone(),
        0,
        SignId::from("patchbay/recursive-form/born"),
    )
    .map_err(|error| error.to_string())?;
    let (body, wake) = body
        .wake(1, SignId::from("patchbay/recursive-form/woke"))
        .map_err(|error| error.to_string())?;
    let wake = wake
        .plan_ready(
            &proof.recursive,
            SignId::from("patchbay/recursive-form/planned"),
        )
        .map_err(|error| error.to_string())?;
    let presentation =
        crate::PatchbayPresentation::new(1, editor.view(), Some(plan), None, None, Vec::new())
            .map_err(|error| error.to_string())?
            .with_graph(graph)
            .map_err(|error| error.to_string())?
            .to_portable(&body, &wake)
            .map_err(|error| error.to_string())?;
    let body_subject = format!("body/{}", body.body_id.as_str());
    let mut subjects = presentation.subjects;
    subjects.push(conduit_presentation::PresentationSubject {
        identity: body_subject,
        role: conduit_presentation::PresentationRole::Body,
        label: "Recursive Form demonstration Body".into(),
        accessibility_name: "Body containing one recursively realized Form".into(),
    });
    conduit_presentation::Presentation::new_with_semantics(
        presentation.revision,
        presentation.basis,
        subjects,
        presentation.relationships,
        presentation.properties,
        presentation.text,
        presentation.actions,
        presentation.disclosures,
    )
    .map_err(|error| error.to_string())
}

fn reviewed_back_face(
    back: &conduit_core::RealizationBack,
    startup: &StartupCatalog,
) -> Result<conduit_core::CheckedFace, String> {
    for source in [
        conduit_semantic_catalog::PATCHBAY_ROOT_BACK_SOURCE,
        conduit_semantic_catalog::PATCHBAY_GEAR_FACE_BACK_SOURCE,
        conduit_semantic_catalog::PATCHBAY_PORT_BACK_SOURCE,
        conduit_semantic_catalog::PATCHBAY_CORD_BACK_SOURCE,
    ] {
        let document = check_syntax_document(&parse_syntax_document(source), startup)
            .map_err(|error| format!("check reviewed Patchbay Back: {error:?}"))?;
        if document.source_document_id != back.source_document_id {
            continue;
        }
        if let Some(form) = document
            .forms
            .iter()
            .find(|form| form.checked_form_id == back.checked_form_id)
        {
            return Ok(form.checked_face());
        }
    }
    Err(format!(
        "checked Face for recursive Back {} is absent",
        back.kind_id.as_str()
    ))
}

fn plan(
    form: &conduit_form::ExpandedCanonicalForm,
    host: &HostAdvertisement,
) -> Result<Plan, String> {
    let placements =
        conduit_planner::default_expanded_placements(form, core::slice::from_ref(host)).map_err(
            |error| {
                let unmatched = form
                    .gears
                    .iter()
                    .filter(|gear| {
                        !host
                            .capabilities
                            .iter()
                            .any(|offer| offer.checked_face() == gear.checked_face())
                    })
                    .map(|gear| gear.kind_id.as_str())
                    .collect::<Vec<_>>();
                format!("{error}; unmatched checked faces={unmatched:?}")
            },
        )?;
    conduit_planner::plan_expanded_canonical_with_options(
        form,
        core::slice::from_ref(host),
        &placements,
        &[conduit_core::BaseImplementationId::from(
            "conduit.base/local@1",
        )],
        conduit_planner::PlanningOptions {
            connection_bases: &std::collections::BTreeMap::new(),
            line_candidates: &std::collections::BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 64,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(|error| error.to_string())
}

fn catalogs() -> Result<(StartupCatalog, ProfileCatalog), String> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_layout_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_presentation_composition_catalogs(
        &mut startup,
        &mut profile,
    )?;
    conduit_semantic_catalog::install_graphics_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_graphics_presentation_catalog(&mut startup, &mut profile)?;
    conduit_presentation::install_bitmap_presentation_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_patchbay_presentation_catalogs(&mut startup, &mut profile)?;
    Ok((startup, profile))
}

fn backs(
    startup: &StartupCatalog,
    profile: &ProfileCatalog,
) -> Result<CanonicalBackCatalog, String> {
    let mut backs = CanonicalBackCatalog::new();
    conduit_semantic_catalog::install_patchbay_presentation_backs(startup, profile, &mut backs)?;
    Ok(backs)
}

fn direct_host() -> HostAdvertisement {
    host(
        "patchbay-browser",
        "patchbay-browser-boot",
        "patchbay/browser-direct@1",
        vec![
            text_literal_fixture_offer("patchbay/browser-text-literal@1"),
            conduit_std_offers::patchbay_presentation_offers()[0].clone(),
        ],
    )
}

fn recursive_host() -> HostAdvertisement {
    host(
        "patchbay-constrained",
        "patchbay-constrained-boot",
        "patchbay/constrained-recursive@1",
        vec![
            text_literal_fixture_offer("patchbay/constrained-text-literal@1"),
            conduit_std_offers::text_presentation_offer(),
            conduit_std_offers::layout_viewport_offer(),
            conduit_std_offers::layout_inset_offer(),
            conduit_std_offers::layout_column_offer(),
            conduit_std_offers::layout_align_offer(),
            conduit_std_offers::layout_stack_offer(),
            conduit_std_offers::presentation_icon_offer(),
            conduit_std_offers::presentation_frame_offer(),
            conduit_std_offers::presentation_badge_offer(),
            conduit_std_offers::graphics_rect_offer(),
            conduit_std_offers::graphics_text_offer(),
            conduit_std_offers::graphics_icon_offer(),
            conduit_std_offers::graphics_presentation_offer(),
        ],
    )
}

fn host(
    host: &str,
    boot: &str,
    profile: &str,
    capabilities: Vec<conduit_core::CapabilityOffer>,
) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from(profile),
        resources: vec![resource_offer(
            &format!("{host}/display"),
            PRESENTATION_RESOURCE_CLASS,
            32,
        )],
        capabilities,
        planner_capabilities: Vec::new(),
    }
}

fn text_literal_fixture_offer(implementation: &str) -> conduit_core::CapabilityOffer {
    let contract = conduit_text::text_literal_semantics();
    conduit_core::CapabilityOffer {
        startup_parameters: vec![conduit_core::FaceStartupParameter {
            name: "value".into(),
            value_type: "Text".into(),
            has_default: false,
        }],
        shorthand: None,
        capability_id: conduit_core::CapabilityId::from(implementation),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: conduit_core::ExecutionProfileId::from(
                "patchbay/presenter-fixture@1",
            ),
            implementation_id: conduit_core::ImplementationId::from(implementation),
            artifact_id: conduit_core::ArtifactId::from("patchbay/presenter-fixture@1"),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}
