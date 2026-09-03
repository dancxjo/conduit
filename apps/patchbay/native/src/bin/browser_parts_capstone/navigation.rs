//! Portable Cord-to-Line navigation over the live cross-browser Plan.

use conduit_body::{Body, BodyMembership, CandidateInventory, PartId};
use conduit_core::{HostAdvertisement, SignId};
use conduit_observatory::{
    build_report, CapabilityAvailability, CapabilityStatusReport, CapabilitySupport, HostReport,
    LineReport, ObservatorySnapshot, OfferFreshness, OperationalState, RetentionReport,
    SNAPSHOT_SCHEMA,
};
use conduit_presentation::{
    NavigationOperation, NavigationState, PresentationAspect, PresentationDepth, PresentationPlace,
    PresentationRelationshipKind, PresentationRole, MAX_NAVIGATION_HISTORY,
};
use patchbay_model::{
    FormEditor, PartsView, PatchbayGraph, PatchbayNavigationProjection, PatchbayPresentation,
    PatchbayRequestId, PlanDocument,
};
use serde_json::{json, Value};

use super::planning::CrossBrowserPlan;

const SOURCE: &str = include_str!("../../../../../../examples/webchat.conduit");

pub(super) fn cord_line_receipt(
    body: &Body,
    membership: &BodyMembership,
    candidates: &CandidateInventory,
    here: &PartId,
    cross: &CrossBrowserPlan,
    source: &HostAdvertisement,
    sink: &HostAdvertisement,
) -> Result<Value, String> {
    let (body, wake) = body
        .clone()
        .wake(2, SignId::from("browser-parts-capstone/navigation-woke"))
        .map_err(debug("wake navigation Body"))?;
    let wake = wake
        .plan_ready(
            &cross.plan,
            SignId::from("browser-parts-capstone/navigation-planned"),
        )
        .map_err(debug("admit navigation Plan"))?;
    let parts = PartsView::project(
        &body,
        membership,
        candidates,
        here,
        Some(&cross.plan),
        None,
        true,
    )
    .map_err(debug("project navigation Parts"))?;
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_net::install_external_websocket_catalogs(&mut startup, &mut profile)?;
    conduit_chat::install_browser_chat_catalogs(&mut startup, &mut profile)?;
    let mut editor = FormEditor::from_source_with_catalogs(
        "examples/webchat.conduit".into(),
        SOURCE.into(),
        startup,
        profile,
    )
    .map_err(|error| error.to_string())?;
    editor
        .open_back("webchat-browser-demo")
        .map_err(|error| error.to_string())?;
    let expanded = editor
        .expand_form("webchat-browser-demo")
        .map_err(|error| error.to_string())?;
    if expanded.source_document_id != body.source_document_id
        || expanded.checked_form_id != body.checked_form_id
    {
        return Err("live navigation Body does not bind the canonical webchat Form".into());
    }
    let plan_document = PlanDocument::from_plan(
        PatchbayRequestId::new("browser-parts-capstone/navigation-plan")
            .map_err(debug("navigation request identity"))?,
        &cross.plan,
    )
    .map_err(debug("navigation Plan document"))?;
    let snapshot = ObservatorySnapshot {
        schema: SNAPSHOT_SCHEMA.into(),
        hosts: vec![available_host(source), available_host(sink)],
        bases: vec![],
        lines: vec![LineReport {
            offer: cross.line.clone(),
            state: OperationalState::Available,
        }],
        plans: vec![cross.plan.clone()],
        plays: vec![],
        observations: vec![],
        historical_observations: vec![],
        sealed_boot_provenance: vec![],
        retention: RetentionReport {
            item_capacity: 1,
            retained_items: 0,
            dropped_items: 0,
        },
    };
    let projection = PatchbayPresentation::new(
        1,
        editor.view(),
        Some(plan_document),
        None,
        Some(build_report(&snapshot)?),
        vec![],
    )
    .map_err(|error| error.to_string())?
    .with_graph(PatchbayGraph::from_expanded(&expanded).map_err(|error| error.to_string())?)
    .map_err(|error| error.to_string())?;
    let presentation = projection
        .to_portable_front_door(&body, &wake, &parts)
        .map_err(|error| error.to_string())?;
    let navigation = PatchbayNavigationProjection::for_embodied(&presentation)?;
    let (cord, line, follow) = exact_cord_line(&presentation, &navigation)?;
    let basis = presentation.basis.clone();
    let mut state = NavigationState::new(
        &navigation.navigation,
        navigation.cursor.clone(),
        MAX_NAVIGATION_HISTORY,
    )
    .map_err(debug("initialize Cord navigation"))?;
    navigate(
        &mut state,
        &presentation,
        &navigation,
        NavigationOperation::Show(PresentationAspect::Plan),
    )?;
    navigate(
        &mut state,
        &presentation,
        &navigation,
        NavigationOperation::Focus(cord.clone()),
    )?;
    let program = state.cursor().clone();
    navigate(
        &mut state,
        &presentation,
        &navigation,
        NavigationOperation::Follow(follow.clone()),
    )?;
    navigate(
        &mut state,
        &presentation,
        &navigation,
        NavigationOperation::Disclose(PresentationDepth::Exact),
    )?;
    let exact = state.cursor().clone();
    navigate(
        &mut state,
        &presentation,
        &navigation,
        NavigationOperation::Back,
    )?;
    navigate(
        &mut state,
        &presentation,
        &navigation,
        NavigationOperation::Back,
    )?;
    let returned = state.cursor().clone();
    if returned != program || presentation.basis != basis {
        return Err("pure Cord-to-Line navigation changed its cursor history or basis".into());
    }
    Ok(json!({
        "schema": "conduit.presentation/live-cord-line-navigation@1",
        "source_document_id": cross.plan.source_document_id.as_str(),
        "checked_form_id": cross.plan.checked_form_id.as_str(),
        "expanded_form_id": cross.plan.expanded_form_id.as_str(),
        "body_id": body.body_id.as_str(),
        "wake_id": wake.wake_id.as_str(),
        "plan_id": cross.plan.plan_id.as_str(),
        "cord_subject": cord,
        "line_subject": line,
        "line_id": cross.line.line_id.as_str(),
        "follow_id": follow,
        "program_cursor": program,
        "exact_line_cursor": exact,
        "returned_cursor": returned,
        "semantic_basis_preserved": true,
        "play_claimed": false,
    }))
}

fn available_host(advertisement: &HostAdvertisement) -> HostReport {
    HostReport {
        advertisement: advertisement.clone(),
        state: OperationalState::Available,
        devices: Vec::new(),
        capabilities: advertisement
            .capabilities
            .iter()
            .map(|offer| CapabilityStatusReport {
                capability_id: offer.capability_id.clone(),
                freshness: OfferFreshness::Fresh,
                support: CapabilitySupport::Supported,
                availability: CapabilityAvailability::Available,
            })
            .collect(),
    }
}

fn exact_cord_line(
    presentation: &conduit_presentation::Presentation,
    navigation: &PatchbayNavigationProjection,
) -> Result<(String, String, String), String> {
    let relationship = presentation
        .relationships
        .iter()
        .find(|relationship| {
            relationship.kind == PresentationRelationshipKind::Realizes
                && role(presentation, &relationship.source) == Some(PresentationRole::Cord)
                && role(presentation, &relationship.target) == Some(PresentationRole::Line)
        })
        .ok_or("portable Presentation has no exact Cord-to-Line realization")?;
    let follow = navigation
        .navigation
        .follows
        .iter()
        .find(|follow| {
            follow.source_subject == relationship.source
                && follow.target_subject == relationship.target
                && follow.target_place == PresentationPlace::Body
                && follow.target_aspect == PresentationAspect::Plan
        })
        .ok_or("portable navigation has no Program-to-Body Cord realization FOLLOW")?;
    Ok((
        relationship.source.clone(),
        relationship.target.clone(),
        follow.identity.clone(),
    ))
}

fn role(
    presentation: &conduit_presentation::Presentation,
    subject: &str,
) -> Option<PresentationRole> {
    presentation
        .subjects
        .iter()
        .find(|candidate| candidate.identity == subject)
        .map(|candidate| candidate.role)
}

fn navigate(
    state: &mut NavigationState,
    presentation: &conduit_presentation::Presentation,
    navigation: &PatchbayNavigationProjection,
    operation: NavigationOperation,
) -> Result<(), String> {
    state
        .navigate(
            presentation,
            &navigation.navigation,
            presentation.revision,
            operation,
        )
        .map(|_| ())
        .map_err(debug("Cord-to-Line navigation"))
}

fn debug<T: core::fmt::Debug>(context: &'static str) -> impl FnOnce(T) -> String {
    move |error| format!("{context}: {error:?}")
}
