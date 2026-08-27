//! Portable Patchbay explanation of the exact split Text Lab realization.

use conduit_body::{
    AuthenticatedHostObservation, Body, BodyMembership, CandidateInventory, MembershipProofId,
    PartId,
};
use conduit_core::{HostAdvertisement, SignId};
use conduit_observatory::{
    build_report, CapabilityAvailability, CapabilityStatusReport, CapabilitySupport, HostReport,
    LineReport, ObservatorySnapshot, OfferFreshness, OperationalState, RetentionReport,
    SNAPSHOT_SCHEMA,
};
use conduit_presentation::{
    NavigationOperation, NavigationState, Presentation, PresentationAction,
    PresentationActionAvailability, PresentationAspect, PresentationCursor, PresentationDepth,
    PresentationDisclosureLevel, PresentationPropertyValue, PresentationRelationshipKind,
    PresentationRole, MAX_NAVIGATION_HISTORY,
};
use conduit_std_catalog::TextLabLineLossReceipt;
use conduit_std_catalog::{exact_text_lab_split_plan, TEXT_LAB_BROWSER_HOST, TEXT_LAB_RETURN_LINE};
use serde::{Deserialize, Serialize};

use crate::text_lab_explanation_loss::validate_loss;
use crate::{
    portable_content::ContentBuilder, FormEditor, PartsView, PatchbayGraph,
    PatchbayNavigationProjection, PatchbayPresentation, PatchbayRequestId, PlanDocument,
};

const SOURCE: &str = include_str!("../../../../examples/text-lab.conduit");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextLabSplitExplanation {
    pub ordinary_path: String,
    pub presentation: Presentation,
    pub navigation: PatchbayNavigationProjection,
    pub upper_program_cursor: PresentationCursor,
    pub browser_host_cursor: PresentationCursor,
    pub return_line_cursor: PresentationCursor,
    pub returned_program_cursor: PresentationCursor,
}

pub fn text_lab_split_explanation(base: &str) -> Result<TextLabSplitExplanation, String> {
    text_lab_explanation(base, None)
}

pub fn text_lab_split_loss_explanation(
    base: &str,
    receipt: &TextLabLineLossReceipt,
) -> Result<TextLabSplitExplanation, String> {
    text_lab_explanation(base, Some(receipt))
}

fn text_lab_explanation(
    base: &str,
    loss: Option<&TextLabLineLossReceipt>,
) -> Result<TextLabSplitExplanation, String> {
    let exact = exact_text_lab_split_plan(
        base,
        &conduit_browser_runtime::presentation_nucleus::browser_text_upper_offer(),
    )?;
    if let Some(receipt) = loss {
        validate_loss(base, &exact.plan, receipt)?;
    }
    let editor = FormEditor::from_source("examples/text-lab.conduit".into(), SOURCE.into())
        .map_err(|error| error.to_string())?;
    let expanded = editor
        .expand_form("text-lab")
        .map_err(|error| error.to_string())?;
    if expanded.source_document_id != exact.plan.source_document_id
        || expanded.checked_form_id != exact.plan.checked_form_id
        || expanded.expanded_form_id != exact.plan.expanded_form_id
    {
        return Err("Patchbay Text Lab explanation changed canonical Form identity".into());
    }

    let body = Body::born(
        exact.plan.source_document_id.clone(),
        exact.plan.checked_form_id.clone(),
        1,
        SignId::from("patchbay/text-lab/born"),
    )
    .map_err(|error| error.to_string())?;
    let (body, wake) = body
        .wake(1, SignId::from("patchbay/text-lab/woke"))
        .map_err(|error| error.to_string())?;
    let wake = wake
        .plan_ready(&exact.plan, SignId::from("patchbay/text-lab/planned"))
        .map_err(|error| error.to_string())?;
    let mut membership =
        BodyMembership::new(body.body_id.clone()).map_err(|error| format!("{error:?}"))?;
    let here = admit(&mut membership, &body, "here", &exact.native, 0)?;
    admit(&mut membership, &body, "browser", &exact.browser, 1)?;
    let candidates =
        CandidateInventory::new(body.body_id.clone()).map_err(|error| format!("{error:?}"))?;
    let parts = PartsView::project(
        &body,
        &membership,
        &candidates,
        &here,
        Some(&exact.plan),
        None,
        true,
    )
    .map_err(|error| format!("{error:?}"))?;

    let plan = PlanDocument::from_plan(
        PatchbayRequestId::new("patchbay/text-lab/split-plan")
            .map_err(|error| format!("{error:?}"))?,
        &exact.plan,
    )
    .map_err(|error| format!("{error:?}"))?;
    let snapshot = ObservatorySnapshot {
        schema: SNAPSHOT_SCHEMA.into(),
        hosts: vec![
            available_host(&exact.native),
            available_host(&exact.browser),
        ],
        bases: vec![],
        lines: vec![
            LineReport {
                offer: exact.forward_line,
                state: OperationalState::Available,
            },
            LineReport {
                offer: exact.return_line,
                state: if loss.is_some() {
                    OperationalState::Unreachable
                } else {
                    OperationalState::Available
                },
            },
        ],
        plans: vec![exact.plan.clone()],
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
        Some(plan),
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
    let presentation = append_ordinary_path(presentation, loss)?;
    let navigation = PatchbayNavigationProjection::for_embodied(&presentation)?;
    let upper = subject_with_identity_property(
        &presentation,
        PresentationRole::Gear,
        "host-id",
        TEXT_LAB_BROWSER_HOST,
    )?;
    let browser = realized_target(&presentation, &upper, PresentationRole::Host)?;
    let return_cord = subject_with_identity_property(
        &presentation,
        PresentationRole::Cord,
        "line-id",
        TEXT_LAB_RETURN_LINE,
    )?;
    let return_line = realized_target(&presentation, &return_cord, PresentationRole::Line)?;

    let mut state = NavigationState::new(
        &navigation.navigation,
        navigation.cursor.clone(),
        MAX_NAVIGATION_HISTORY,
    )
    .map_err(|error| format!("{error:?}"))?;
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
        NavigationOperation::Focus(upper.clone()),
    )?;
    let upper_program_cursor = state.cursor().clone();
    follow_to(&mut state, &presentation, &navigation, &upper, &browser)?;
    navigate(
        &mut state,
        &presentation,
        &navigation,
        NavigationOperation::Disclose(PresentationDepth::Exact),
    )?;
    let browser_host_cursor = state.cursor().clone();
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
    navigate(
        &mut state,
        &presentation,
        &navigation,
        NavigationOperation::Focus(return_cord.clone()),
    )?;
    follow_to(
        &mut state,
        &presentation,
        &navigation,
        &return_cord,
        &return_line,
    )?;
    navigate(
        &mut state,
        &presentation,
        &navigation,
        NavigationOperation::Disclose(PresentationDepth::Exact),
    )?;
    let return_line_cursor = state.cursor().clone();
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
    let returned_program_cursor = state.cursor().clone();

    Ok(TextLabSplitExplanation {
        ordinary_path: loss.map_or_else(
            || "keyboard here -> uppercase there -> presentation here".into(),
            |_| "browser Part unavailable -> unchanged Form currently unrealizable".into(),
        ),
        presentation,
        navigation,
        upper_program_cursor,
        browser_host_cursor,
        return_line_cursor,
        returned_program_cursor,
    })
}

fn append_ordinary_path(
    mut presentation: Presentation,
    loss: Option<&TextLabLineLossReceipt>,
) -> Result<Presentation, String> {
    let mut content = ContentBuilder::from_parts(
        presentation.subjects,
        presentation.relationships,
        presentation.properties,
        presentation.text,
    );
    let path = content.subject_with_identity(
        "text-lab/ordinary-path",
        PresentationRole::Info,
        "keyboard here -> uppercase there -> presentation here",
        "Text Lab placement: keyboard here, uppercase there, presentation here",
    );
    content.line(
        &path,
        "keyboard here -> uppercase there -> presentation here",
    );
    let form = content
        .subjects
        .iter()
        .find(|subject| subject.role == PresentationRole::Form)
        .map(|subject| subject.identity.clone())
        .ok_or("Text Lab explanation lacks its Program Form")?;
    content.contains(&form, &path);
    if let Some(receipt) = loss {
        let unavailable = content.subject_with_identity(
            "text-lab/unavailable",
            PresentationRole::Info,
            "browser Part unavailable -> unchanged Form currently unrealizable",
            "Text Lab unavailable: browser Part lost; unchanged Form currently unrealizable",
        );
        content.line(
            &unavailable,
            "browser Part unavailable -> unchanged Form currently unrealizable",
        );
        content.contains(&form, &unavailable);
        let sign = content.subject_with_identity(
            format!("sign/{}", receipt.sign_id),
            PresentationRole::Sign,
            receipt.code.clone(),
            format!("Causal Text Lab Line-loss Sign {}", receipt.sign_id),
        );
        content.line(
            &sign,
            format!(
                "code={} line={} plan={} active-play={} sequence={} old-plan={} fresh-planning={} refusal={}",
                receipt.code,
                receipt.line_id,
                receipt.plan_id,
                receipt.active_play_id,
                receipt.sequence,
                receipt.old_plan_disposition,
                receipt.fresh_planning,
                receipt.refusal
            ),
        );
        presentation
            .basis
            .sign_ids
            .push(conduit_core::SignId::from(receipt.sign_id.clone()));
    }
    let mut actions = presentation.actions;
    actions.push(PresentationAction {
        identity: "action/text-lab/observe-return-line-loss".into(),
        intent: "conduit.intent/observe-line-loss@1".into(),
        target: path,
        label: "Observe browser loss".into(),
        disclosure: PresentationDisclosureLevel::CurrentAction,
        availability: if loss.is_some() {
            PresentationActionAvailability::Unavailable {
                reason_code: "line/already-unavailable".into(),
                explanation: "The exact return Line is already unavailable.".into(),
            }
        } else {
            PresentationActionAvailability::Available
        },
    });
    Presentation::new_with_semantics_and_temporal(
        presentation.revision + u64::from(loss.is_some()),
        presentation.basis,
        content.subjects,
        content.relationships,
        content.properties,
        content.text,
        actions,
        presentation.disclosures,
        presentation.temporal_references,
        presentation.temporal_facts,
    )
    .map_err(|error| format!("invalid Text Lab ordinary explanation: {error:?}"))
}

fn admit(
    membership: &mut BodyMembership,
    body: &Body,
    name: &str,
    host: &HostAdvertisement,
    index: u64,
) -> Result<PartId, String> {
    let part = PartId::bind(&body.body_id, name, index).map_err(|error| format!("{error:?}"))?;
    let proof = MembershipProofId::bind(&format!("patchbay/text-lab/{name}"))
        .map_err(|error| format!("{error:?}"))?;
    membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            SignId::from(format!("patchbay/text-lab/{name}/admitted")),
        )
        .map_err(|error| format!("{error:?}"))?;
    membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                offer_generation: host.offer_generation,
                proof_id: proof,
                sequence: index + 1,
            },
            SignId::from(format!("patchbay/text-lab/{name}/present")),
        )
        .map_err(|error| format!("{error:?}"))?;
    Ok(part)
}

fn available_host(advertisement: &HostAdvertisement) -> HostReport {
    HostReport {
        advertisement: advertisement.clone(),
        state: OperationalState::Available,
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

fn subject_with_identity_property(
    presentation: &Presentation,
    role: PresentationRole,
    name: &str,
    value: &str,
) -> Result<String, String> {
    presentation
        .subjects
        .iter()
        .find(|subject| {
            subject.role == role
                && presentation.properties.iter().any(|property| {
                    property.subject == subject.identity
                        && property.name == name
                        && property.value == PresentationPropertyValue::Identity(value.into())
                })
        })
        .map(|subject| subject.identity.clone())
        .ok_or_else(|| format!("Text Lab explanation lacks {role:?} with {name}={value}"))
}

fn realized_target(
    presentation: &Presentation,
    source: &str,
    role: PresentationRole,
) -> Result<String, String> {
    presentation
        .relationships
        .iter()
        .find(|relationship| {
            relationship.source == source
                && relationship.kind == PresentationRelationshipKind::Realizes
                && presentation
                    .subjects
                    .iter()
                    .any(|subject| subject.identity == relationship.target && subject.role == role)
        })
        .map(|relationship| relationship.target.clone())
        .ok_or_else(|| format!("Text Lab explanation lacks {source} realization"))
}

fn follow_to(
    state: &mut NavigationState,
    presentation: &Presentation,
    navigation: &PatchbayNavigationProjection,
    source: &str,
    target: &str,
) -> Result<(), String> {
    let follow = navigation
        .navigation
        .follows
        .iter()
        .find(|follow| follow.source_subject == source && follow.target_subject == target)
        .ok_or("Text Lab explanation lacks exact cross-domain FOLLOW")?;
    navigate(
        state,
        presentation,
        navigation,
        NavigationOperation::Follow(follow.identity.clone()),
    )
}

fn navigate(
    state: &mut NavigationState,
    presentation: &Presentation,
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
        .map_err(|error| format!("Text Lab navigation refused: {error:?}"))
}
