//! Portable Patchbay explanation of the exact split Text Lab realization.

use conduit_body::{
    AuthenticatedHostObservation, Body, BodyMembership, CandidateInventory, MembershipProofId,
    PartId,
};
use conduit_core::{HostAdvertisement, SignId};
use conduit_face::{
    Face, FaceAction, FaceActionAvailability, FaceAspect, FaceCursor, FaceDepth,
    FaceDisclosureLevel, FacePlace, FacePropertyValue, FaceRelationshipKind, FaceRole,
    NavigationOperation, NavigationState, MAX_NAVIGATION_HISTORY,
};
use conduit_observatory::{
    build_report, CapabilityAvailability, CapabilityStatusReport, CapabilitySupport, HostReport,
    LineReport, ObservatorySnapshot, OfferFreshness, OperationalState, RetentionReport,
    SNAPSHOT_SCHEMA,
};
use conduit_semantic_catalog::TextLabLineLossReceipt;
use conduit_semantic_catalog::{
    exact_text_lab_split_plan, TEXT_LAB_BROWSER_HOST, TEXT_LAB_RETURN_LINE,
};
use patchbay_application::{PartsView, PatchbayNavigationProjection};
use serde::{Deserialize, Serialize};

use crate::text_lab_explanation_loss::validate_loss;
use crate::{
    portable_content::ContentBuilder, PatchbayFace, PatchbayGraph, PatchbayRequestId, PlanDocument,
    PlotEditor,
};

const SOURCE: &str = include_str!("../../../../../plots/text-lab/main.conduit");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextLabSplitExplanation {
    pub ordinary_path: String,
    pub face: Face,
    pub navigation: PatchbayNavigationProjection,
    pub upper_program_cursor: FaceCursor,
    pub browser_host_cursor: FaceCursor,
    pub return_line_cursor: FaceCursor,
    pub returned_program_cursor: FaceCursor,
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
        &conduit_browser_runtime::face_nucleus::browser_text_upper_offer(),
    )?;
    if let Some(receipt) = loss {
        validate_loss(base, &exact.plan, receipt)?;
    }
    let editor = PlotEditor::from_source("plots/text-lab/main.conduit".into(), SOURCE.into())
        .map_err(|error| error.to_string())?;
    let expanded = editor
        .expand_plot("text-lab")
        .map_err(|error| error.to_string())?;
    if expanded.source_document_id != exact.plan.source_document_id
        || expanded.checked_plot_id != exact.plan.checked_plot_id
        || expanded.expanded_plot_id != exact.plan.expanded_plot_id
    {
        return Err("Patchbay Text Lab explanation changed canonical Plot identity".into());
    }

    let body = Body::born(
        exact.plan.source_document_id.clone(),
        exact.plan.checked_plot_id.clone(),
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
    let projection = PatchbayFace::new(
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
    let face = projection
        .to_portable_front_door(&body, &wake, &parts)
        .map_err(|error| error.to_string())?;
    let face = append_ordinary_path(face, loss)?;
    let navigation = PatchbayNavigationProjection::for_embodied(&face)?;
    let upper =
        subject_with_identity_property(&face, FaceRole::Gear, "host-id", TEXT_LAB_BROWSER_HOST)?;
    let browser = realized_target(&face, &upper, FaceRole::Host)?;
    let return_cord =
        subject_with_identity_property(&face, FaceRole::Cord, "line-id", TEXT_LAB_RETURN_LINE)?;
    let return_line = realized_target(&face, &return_cord, FaceRole::Line)?;

    let mut state = NavigationState::new(
        &navigation.navigation,
        navigation.cursor.clone(),
        MAX_NAVIGATION_HISTORY,
    )
    .map_err(|error| format!("{error:?}"))?;
    navigate(
        &mut state,
        &face,
        &navigation,
        NavigationOperation::Enter(FacePlace::Program),
    )?;
    navigate(
        &mut state,
        &face,
        &navigation,
        NavigationOperation::Show(FaceAspect::Plan),
    )?;
    navigate(
        &mut state,
        &face,
        &navigation,
        NavigationOperation::Focus(upper.clone()),
    )?;
    let upper_program_cursor = state.cursor().clone();
    follow_to(&mut state, &face, &navigation, &upper, &browser)?;
    navigate(
        &mut state,
        &face,
        &navigation,
        NavigationOperation::Disclose(FaceDepth::Exact),
    )?;
    let browser_host_cursor = state.cursor().clone();
    navigate(&mut state, &face, &navigation, NavigationOperation::Back)?;
    navigate(&mut state, &face, &navigation, NavigationOperation::Back)?;
    navigate(
        &mut state,
        &face,
        &navigation,
        NavigationOperation::Focus(return_cord.clone()),
    )?;
    follow_to(&mut state, &face, &navigation, &return_cord, &return_line)?;
    navigate(
        &mut state,
        &face,
        &navigation,
        NavigationOperation::Disclose(FaceDepth::Exact),
    )?;
    let return_line_cursor = state.cursor().clone();
    navigate(&mut state, &face, &navigation, NavigationOperation::Back)?;
    navigate(&mut state, &face, &navigation, NavigationOperation::Back)?;
    let returned_program_cursor = state.cursor().clone();

    Ok(TextLabSplitExplanation {
        ordinary_path: loss.map_or_else(
            || "keyboard here -> uppercase there -> face here".into(),
            |_| "browser Part unavailable -> unchanged Plot currently unrealizable".into(),
        ),
        face,
        navigation,
        upper_program_cursor,
        browser_host_cursor,
        return_line_cursor,
        returned_program_cursor,
    })
}

fn append_ordinary_path(
    mut face: Face,
    loss: Option<&TextLabLineLossReceipt>,
) -> Result<Face, String> {
    let mut content = ContentBuilder::from_parts(
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
    );
    let path = content.subject_with_identity(
        "text-lab/ordinary-path",
        FaceRole::Info,
        "Text Lab placement: keyboard here, uppercase there, face here",
    );
    content.line(&path, "keyboard here -> uppercase there -> face here");
    let plot = content
        .subjects
        .iter()
        .find(|subject| subject.role == FaceRole::Plot)
        .map(|subject| subject.identity.clone())
        .ok_or("Text Lab explanation lacks its Program Plot")?;
    content.contains(&plot, &path);
    if let Some(receipt) = loss {
        let unavailable = content.subject_with_identity(
            "text-lab/unavailable",
            FaceRole::Info,
            "Text Lab unavailable: browser Part lost; unchanged Plot currently unrealizable",
        );
        content.line(
            &unavailable,
            "browser Part unavailable -> unchanged Plot currently unrealizable",
        );
        content.contains(&plot, &unavailable);
        let sign = content.subject_with_identity(
            format!("sign/{}", receipt.sign_id),
            FaceRole::Sign,
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
        face.basis
            .sign_ids
            .push(conduit_core::SignId::from(receipt.sign_id.clone()));
    }
    let mut actions = face.actions;
    actions.push(FaceAction {
        identity: "action/text-lab/observe-return-line-loss".into(),
        intent: "conduit.intent/observe-line-loss@1".into(),
        target: path,
        name: "Observe browser loss".into(),
        arguments: vec![],
        disclosure: FaceDisclosureLevel::CurrentAction,
        availability: if loss.is_some() {
            FaceActionAvailability::Unavailable {
                reason_code: "line/already-unavailable".into(),
                explanation: "The exact return Line is already unavailable.".into(),
            }
        } else {
            FaceActionAvailability::Available
        },
    });
    Face::new_with_semantics_and_temporal(
        face.revision + u64::from(loss.is_some()),
        face.basis,
        content.subjects,
        content.relationships,
        content.properties,
        content.text,
        actions,
        face.disclosures,
        face.temporal_references,
        face.temporal_facts,
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

fn subject_with_identity_property(
    face: &Face,
    role: FaceRole,
    name: &str,
    value: &str,
) -> Result<String, String> {
    face.subjects
        .iter()
        .find(|subject| {
            subject.role == role
                && face.properties.iter().any(|property| {
                    property.subject == subject.identity
                        && property.name == name
                        && property.value == FacePropertyValue::Identity(value.into())
                })
        })
        .map(|subject| subject.identity.clone())
        .ok_or_else(|| format!("Text Lab explanation lacks {role:?} with {name}={value}"))
}

fn realized_target(face: &Face, source: &str, role: FaceRole) -> Result<String, String> {
    face.relationships
        .iter()
        .find(|relationship| {
            relationship.source == source
                && relationship.kind == FaceRelationshipKind::Realizes
                && face
                    .subjects
                    .iter()
                    .any(|subject| subject.identity == relationship.target && subject.role == role)
        })
        .map(|relationship| relationship.target.clone())
        .ok_or_else(|| format!("Text Lab explanation lacks {source} realization"))
}

fn follow_to(
    state: &mut NavigationState,
    face: &Face,
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
        face,
        navigation,
        NavigationOperation::Follow(follow.identity.clone()),
    )
}

fn navigate(
    state: &mut NavigationState,
    face: &Face,
    navigation: &PatchbayNavigationProjection,
    operation: NavigationOperation,
) -> Result<(), String> {
    state
        .navigate(face, &navigation.navigation, face.revision, operation)
        .map(|_| ())
        .map_err(|error| format!("Text Lab navigation refused: {error:?}"))
}
