//! Preparation conformance, not renderer or measured geometry proof.
use conduit_face::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};

fn face() -> Face {
    Face::new_with_semantics(
        7,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![
            FaceSubject {
                identity: "field".into(),
                role: FaceRole::TextEntry,
                name: "Name".into(),
            },
            FaceSubject {
                identity: "other".into(),
                role: FaceRole::Semantic(conduit_core::kind_id("test/open@1")),
                name: "Unknown".into(),
            },
        ],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
    )
    .unwrap()
}
fn recipe() -> conduit_core::StructuredInfoValue {
    ComponentRecipeInfo::new(
        0,
        "test/recipe@1".into(),
        1,
        280,
        BoundedSequence::try_from_iter([ComponentRegionInfo::new(
            0,
            "field".into(),
            1,
            ComponentSubjectSelector::subject("field".into()).unwrap(),
        )
        .unwrap()])
        .unwrap(),
        1,
    )
    .unwrap()
    .into_structured()
    .unwrap()
}
fn child(node: u8, inline: u8, block: u8, inline_span: u8) -> ComponentLayoutChild {
    ComponentLayoutChild::new(1, block, inline_span, inline, node).unwrap()
}
fn group(children: Vec<ComponentLayoutChild>) -> ComponentLayoutGroup {
    ComponentLayoutGroup::new(
        ComponentLayoutAlignment::Start,
        ComponentLayoutBreak::AccessibleContinuation,
        BoundedSequence::try_from_iter(children).unwrap(),
        8,
        ComponentLayoutAlignment::Stretch,
        1,
        1,
        16,
    )
    .unwrap()
}
fn column(nodes: &[u8]) -> ComponentLayoutNode {
    node_column(group(nodes.iter().map(|n| child(*n, 0, 0, 1)).collect())).unwrap()
}
fn region() -> ComponentLayoutNode {
    ComponentLayoutNode::region(0).unwrap()
}
fn candidate(nodes: Vec<ComponentLayoutNode>) -> ComponentLayoutCandidate {
    ComponentLayoutCandidate::new(BoundedSequence::try_from_iter(nodes).unwrap(), 0).unwrap()
}
fn profile(candidates: Vec<ComponentLayoutCandidate>) -> conduit_core::StructuredInfoValue {
    ComponentLayoutInfo::new(
        BoundedSequence::try_from_iter(candidates).unwrap(),
        "test/layout@1".into(),
        "test/recipe@1".into(),
        1,
    )
    .unwrap()
    .into_structured()
    .unwrap()
}
fn refusal(nodes: Vec<ComponentLayoutNode>) -> ComponentLayoutRefusal {
    prepare_component_layout(profile(vec![candidate(nodes)]), recipe(), &face())
        .err()
        .unwrap()
}

#[test]
fn nested_alternatives_preserve_exact_face_recipe_and_generic_fallback() {
    let face = face();
    let before = face.clone();
    let grid = grid(
        ComponentLayoutGrid::new(2, group(vec![child(3, 0, 0, 2), child(4, 0, 1, 1)]), 2).unwrap(),
    )
    .unwrap();
    let structure = profile(vec![
        candidate(vec![
            column(&[1]),
            row(group(vec![child(2, 0, 0, 1)])).unwrap(),
            grid,
            region(),
            ComponentLayoutNode::Fallback,
        ]),
        candidate(vec![
            column(&[1, 2, 3]),
            region(),
            ComponentLayoutNode::Fallback,
            column(&[]),
        ]),
    ]);
    let prepared = prepare_component_layout(structure, recipe(), &face).unwrap();
    assert_eq!(prepared.face(), &before);
    assert_eq!(prepared.face().revision, 7);
    assert_eq!(prepared.structure().candidates().len(), 2);
    assert_eq!(prepared.recipe().subject_regions, vec![Some(0), None]);
    assert_eq!(prepared.recipe().recipe.identity(), "test/recipe@1");
}

#[test]
fn refuses_invalid_topology_and_incomplete_binding_sets() {
    use ComponentLayoutRefusal as R;
    assert_eq!(refusal(vec![column(&[0])]), R::Cycle);
    assert_eq!(refusal(vec![column(&[3])]), R::NodeReference);
    assert_eq!(refusal(vec![column(&[1, 1]), region()]), R::DuplicateParent);
    assert_eq!(
        refusal(vec![
            column(&[1, 2]),
            region(),
            ComponentLayoutNode::Fallback,
            column(&[])
        ]),
        R::UnreachableNode
    );
    assert_eq!(
        refusal(vec![column(&[1]), ComponentLayoutNode::Fallback]),
        R::MissingRegion
    );
    assert_eq!(refusal(vec![column(&[1]), region()]), R::Fallback);
    assert_eq!(
        refusal(vec![
            column(&[1, 2, 3]),
            region(),
            region(),
            ComponentLayoutNode::Fallback
        ]),
        R::DuplicateRegion
    );
    assert_eq!(
        refusal(vec![ComponentLayoutNode::region(1).unwrap()]),
        R::RegionReference
    );
    assert_eq!(
        refusal(vec![
            column(&[1]),
            column(&[2]),
            column(&[3]),
            column(&[4, 5]),
            region(),
            ComponentLayoutNode::Fallback
        ]),
        R::Depth
    );
}

#[test]
fn refuses_grid_overlap_and_track_span_overrun() {
    use ComponentLayoutRefusal as R;
    for (children, expected) in [
        (vec![child(1, 0, 0, 2), child(2, 1, 0, 1)], R::GridOverlap),
        (vec![child(1, 1, 0, 2), child(2, 0, 1, 1)], R::Placement),
        (vec![child(1, 0, 3, 1), child(2, 0, 1, 1)], R::Placement),
    ] {
        let grid = grid(ComponentLayoutGrid::new(2, group(children), 2).unwrap()).unwrap();
        assert_eq!(
            refusal(vec![grid, region(), ComponentLayoutNode::Fallback]),
            expected
        );
    }
    assert_eq!(
        refusal(vec![
            row(group(vec![child(1, 1, 0, 1), child(2, 0, 0, 1)])).unwrap(),
            region(),
            ComponentLayoutNode::Fallback
        ]),
        R::Placement
    );
}

#[test]
fn profile_bounds_are_checked_before_preparation() {
    assert!(ComponentLayoutChild::new(1, 0, 3, 0, 0).is_err());
    assert!(ComponentLayoutGrid::new(1, group(vec![]), 5).is_err());
    assert!(ComponentLayoutInfo::new(
        BoundedSequence::try_from_iter(Vec::<ComponentLayoutCandidate>::new()).unwrap(),
        "".into(),
        "recipe".into(),
        1
    )
    .is_err());
    let empty = profile(vec![]);
    assert_eq!(
        prepare_component_layout(empty, recipe(), &face()).err(),
        Some(ComponentLayoutRefusal::Capacity)
    );
}

#[test]
fn every_alternative_must_preserve_bindings_and_total_capacity() {
    let face = face();
    let complete = candidate(vec![
        column(&[1, 2]),
        region(),
        ComponentLayoutNode::Fallback,
    ]);
    let incomplete = candidate(vec![column(&[1]), ComponentLayoutNode::Fallback]);
    assert_eq!(
        prepare_component_layout(profile(vec![complete, incomplete]), recipe(), &face).err(),
        Some(ComponentLayoutRefusal::MissingRegion)
    );
    let wide = || {
        candidate({
            let mut nodes = vec![
                column(&(1..17).collect::<Vec<_>>()),
                region(),
                ComponentLayoutNode::Fallback,
            ];
            nodes.extend((3..17).map(|_| column(&[])));
            nodes
        })
    };
    assert_eq!(
        prepare_component_layout(profile(vec![wide(), wide()]), recipe(), &face).err(),
        Some(ComponentLayoutRefusal::Capacity)
    );
    let wrong_recipe = ComponentLayoutInfo::new(
        BoundedSequence::try_from_iter([candidate(vec![
            column(&[1, 2]),
            region(),
            ComponentLayoutNode::Fallback,
        ])])
        .unwrap(),
        "test/layout@1".into(),
        "another/recipe@1".into(),
        1,
    )
    .unwrap()
    .into_structured()
    .unwrap();
    assert_eq!(
        prepare_component_layout(wrong_recipe, recipe(), &face).err(),
        Some(ComponentLayoutRefusal::RecipeIdentity)
    );
    assert_eq!(
        refusal(vec![
            column(&[1, 2, 3]),
            region(),
            ComponentLayoutNode::Fallback,
            ComponentLayoutNode::Fallback
        ]),
        ComponentLayoutRefusal::Fallback
    );
}

#[test]
fn associated_bound_form_fields_compose_without_new_widget_semantics() {
    let face = Face::new_with_semantics(
        8,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![
            FaceSubject {
                identity: "label".into(),
                role: FaceRole::Info,
                name: "Account name".into(),
            },
            FaceSubject {
                identity: "field".into(),
                role: FaceRole::TextEntry,
                name: "Name".into(),
            },
            FaceSubject {
                identity: "help".into(),
                role: FaceRole::Info,
                name: "Use your public name".into(),
            },
        ],
        vec![FaceRelationship {
            source: "label".into(),
            target: "field".into(),
            kind: FaceRelationshipKind::Describes,
        }],
        vec![],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let regions =
        [("label", "Info"), ("field", "TextEntry")]
            .into_iter()
            .map(|(identity, role)| {
                let binding = ComponentSubjectBinding::new(
                    BoundedSequence::try_from_iter(Vec::<ComponentActionBinding>::new()).unwrap(),
                    BoundedSequence::try_from_iter(Vec::<ComponentPropertyBinding>::new()).unwrap(),
                    ComponentBindingRole::role(role.into()).unwrap(),
                    identity.into(),
                )
                .unwrap();
                ComponentRegionInfo::new(
                    0,
                    identity.into(),
                    1,
                    ComponentSubjectSelector::bound(
                        binding.actions().clone(),
                        binding.properties().clone(),
                        binding.role().clone(),
                        binding.subject().clone(),
                    )
                    .unwrap(),
                )
                .unwrap()
            });
    let recipe = ComponentRecipeInfo::new(
        0,
        "test/recipe@1".into(),
        1,
        280,
        BoundedSequence::try_from_iter(regions).unwrap(),
        1,
    )
    .unwrap()
    .into_structured()
    .unwrap();
    let grid = grid(
        ComponentLayoutGrid::new(
            2,
            group(vec![
                child(1, 0, 0, 1),
                child(2, 1, 0, 1),
                child(3, 0, 1, 2),
            ]),
            2,
        )
        .unwrap(),
    )
    .unwrap();
    let wide = candidate(vec![
        grid,
        region(),
        ComponentLayoutNode::region(1).unwrap(),
        ComponentLayoutNode::Fallback,
    ]);
    let narrow = candidate(vec![
        column(&[1, 2, 3]),
        region(),
        ComponentLayoutNode::region(1).unwrap(),
        ComponentLayoutNode::Fallback,
    ]);
    let prepared = prepare_component_layout(profile(vec![wide, narrow]), recipe, &face).unwrap();
    assert_eq!(
        prepared.recipe().subject_regions,
        vec![Some(0), Some(1), None]
    );
    assert_eq!(prepared.face().relationships, face.relationships);
}

fn node_column(
    g: ComponentLayoutGroup,
) -> Result<ComponentLayoutNode, conduit_plot::rust_binding::NativeBindingRefusal> {
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
}
fn row(
    g: ComponentLayoutGroup,
) -> Result<ComponentLayoutNode, conduit_plot::rust_binding::NativeBindingRefusal> {
    ComponentLayoutNode::row(
        *g.block_alignment(),
        *g.break_policy(),
        g.children().clone(),
        *g.gap_em_sixteenths(),
        *g.inline_alignment(),
        *g.inline_grow(),
        *g.inline_shrink(),
        *g.padding_em_sixteenths(),
    )
}
fn grid(
    g: ComponentLayoutGrid,
) -> Result<ComponentLayoutNode, conduit_plot::rust_binding::NativeBindingRefusal> {
    ComponentLayoutNode::grid(*g.block_tracks(), g.group().clone(), *g.inline_tracks())
}
