//! An independent account form built from existing Face subjects/associations/actions.
use super::*;
use alloc::vec;
use conduit_face::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};

pub(super) fn face() -> Face {
    let subject = |identity: &str, role, name: &str| FaceSubject {
        identity: identity.into(),
        role,
        name: name.into(),
    };
    let edit = |identity: &str, target: &str, name: &str, available| FaceAction {
        identity: identity.into(),
        intent: "account/set-text@1".into(),
        target: target.into(),
        name: name.into(),
        arguments: vec![
            FaceActionArgument::text("value".into(), "New value".into(), 0, 128).unwrap(),
        ],
        disclosure: FaceDisclosureLevel::CurrentAction,
        availability: if available {
            FaceActionAvailability::Available
        } else {
            FaceActionAvailability::Unavailable {
                reason_code: "account/locked".into(),
                explanation: "Contact updates are temporarily unavailable".into(),
            }
        },
    };
    Face::new_with_semantics(
        9,
        crate::native_face_scene::tests::face(9).basis,
        vec![
            subject("form", FaceRole::Document, "Account details"),
            subject("email", FaceRole::TextEntry, "Email address"),
            subject("name-label", FaceRole::Info, "Public name"),
            subject("name", FaceRole::TextEntry, "Name"),
            subject(
                "name-help",
                FaceRole::Info,
                "Use the name others know you by",
            ),
            subject("email-label", FaceRole::Info, "Contact"),
            subject(
                "status",
                FaceRole::Status,
                "All original meanings remain available",
            ),
            subject(
                "other",
                FaceRole::Semantic(conduit_core::kind_id("account/open-note@1")),
                "An open semantic note",
            ),
        ],
        vec![
            FaceRelationship {
                source: "name-label".into(),
                target: "name".into(),
                kind: FaceRelationshipKind::Describes,
            },
            FaceRelationship {
                source: "name-help".into(),
                target: "name".into(),
                kind: FaceRelationshipKind::Describes,
            },
            FaceRelationship {
                source: "email-label".into(),
                target: "email".into(),
                kind: FaceRelationshipKind::Describes,
            },
        ],
        vec![],
        vec![
            FaceText {
                subject: "name".into(),
                text: "Élan e\u{301}".into(),
            },
            FaceText {
                subject: "email".into(),
                text: "person@example.test".into(),
            },
        ],
        vec![
            edit("set-name", "name", "Change name", true),
            edit("set-email", "email", "Change email", false),
        ],
        vec![],
    )
    .unwrap()
}
fn child(node: u8, inline: u8, block: u8, inline_span: u8, block_span: u8) -> ComponentLayoutChild {
    ComponentLayoutChild::new(block_span, block, inline_span, inline, node).unwrap()
}
fn group(children: Vec<ComponentLayoutChild>) -> ComponentLayoutGroup {
    ComponentLayoutGroup::new(
        Alignment::Start,
        Break::AccessibleContinuation,
        BoundedSequence::try_from_iter(children).unwrap(),
        8,
        Alignment::Stretch,
        1,
        1,
        8,
    )
    .unwrap()
}
fn column(nodes: &[u8]) -> ComponentLayoutNode {
    let g = group(nodes.iter().map(|node| child(*node, 0, 0, 1, 1)).collect());
    ComponentLayoutNode::column(
        *g.block_alignment(),
        *g.break_policy(),
        g.children().clone(),
        *g.gap_em_sixteenths(),
        *g.inline_alignment(),
        *g.inline_grow(),
        *g.inline_shrink(),
        *g.padding_em_sixteenths(),
    )
    .unwrap()
}
pub(super) fn profile(face: &Face) -> PreparedComponentLayout<'_> {
    let definitions = [
        ("name-label", "Info"),
        ("name", "TextEntry"),
        ("name-help", "Info"),
        ("email-label", "Info"),
        ("email", "TextEntry"),
        ("form", "Document"),
    ];
    let regions = definitions.into_iter().map(|(subject, role)| {
        ComponentRegionInfo::new(
            0,
            subject.into(),
            1,
            ComponentSubjectSelector::bound(
                BoundedSequence::new(),
                BoundedSequence::new(),
                ComponentBindingRole::role(role.into()).unwrap(),
                subject.into(),
            )
            .unwrap(),
        )
        .unwrap()
    });
    let recipe = ComponentRecipeInfo::new(
        0,
        "account/form-regions@1".into(),
        1,
        280,
        BoundedSequence::try_from_iter(regions).unwrap(),
        1,
    )
    .unwrap()
    .into_structured()
    .unwrap();
    let grid = ComponentLayoutNode::grid(
        4,
        group(vec![
            child(7, 1, 2, 1, 1),
            child(3, 0, 0, 1, 1),
            child(5, 0, 1, 2, 1),
            child(4, 1, 0, 1, 1),
            child(6, 0, 2, 1, 2),
            child(8, 1, 3, 1, 1),
        ]),
        2,
    )
    .unwrap();
    let empty = ComponentLayoutNode::grid(
        4,
        ComponentLayoutGroup::new(
            Alignment::Start,
            Break::AccessibleContinuation,
            BoundedSequence::new(),
            0,
            Alignment::Stretch,
            0,
            1,
            0,
        )
        .unwrap(),
        4,
    )
    .unwrap();
    let nodes = vec![
        column(&[1, 2, 9]),
        ComponentLayoutNode::region(5).unwrap(),
        grid,
        ComponentLayoutNode::region(0).unwrap(),
        ComponentLayoutNode::region(1).unwrap(),
        ComponentLayoutNode::region(2).unwrap(),
        ComponentLayoutNode::region(3).unwrap(),
        ComponentLayoutNode::region(4).unwrap(),
        empty,
        ComponentLayoutNode::Fallback,
    ];
    let candidate =
        ComponentLayoutCandidate::new(BoundedSequence::try_from_iter(nodes).unwrap(), 0).unwrap();
    let layout = ComponentLayoutInfo::new(
        BoundedSequence::try_from_iter([candidate]).unwrap(),
        "account/grid-form@1".into(),
        "account/form-regions@1".into(),
        1,
    )
    .unwrap()
    .into_structured()
    .unwrap();
    prepare_component_layout(layout, recipe, face).unwrap()
}
pub(super) fn expression(scale: u16) -> AdmittedVisualExpression {
    admit_visual_expression(
        Ok(VisualExpression::phosphor()),
        VisualUserConstraints {
            forced_colors: false,
            reduced_motion: true,
            minimum_text_scale_percent: scale,
            maximum_supported_text_scale_percent: 200,
        },
    )
    .unwrap()
}
