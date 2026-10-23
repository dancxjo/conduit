//! Versioned ordinary Fore contract. Publication here does not offer a native Back.
use crate::ComponentLayoutInfo;
#[cfg(feature = "plot-catalog")]
use crate::{
    ComponentLayoutAlignment, ComponentLayoutBreak, ComponentLayoutCandidate, ComponentLayoutChild,
    ComponentLayoutNode,
};
#[cfg(feature = "plot-catalog")]
use conduit_plot::rust_binding::BoundedSequence;
use conduit_plot::rust_binding::NativeRustBinding;

pub fn component_layout_type() -> conduit_core::StructuredInfoType {
    ComponentLayoutInfo::semantic_type().expect("checked finite component layout Type")
}

/// The existing Fore identity with an explicit contract revision and required
/// layout configuration. Existing component-mask@2 construction is unchanged.
/// ComponentRecipeInfo supplies existing identity and typed region bindings;
/// ComponentLayoutInfo owns @3 composition. Legacy gutter/reflow/column fields
/// and geometry_for_width remain @2 geometry, not a second @3 layout authority.
#[cfg(feature = "plot-catalog")]
pub fn layout_component_mask_kind(maximum_bytes: u32) -> conduit_core::Kind {
    use conduit_core::*;
    let mut kind = crate::component_mask_kind(maximum_bytes);
    kind.kind_contract_revision = KindIdentity::from("conduit.face/component-mask@3");
    let child = |node| ComponentLayoutChild::new(1, 0, 1, 0, node).unwrap();
    let root = ComponentLayoutNode::column(
        ComponentLayoutAlignment::Stretch,
        ComponentLayoutBreak::AccessibleContinuation,
        BoundedSequence::try_from_iter([child(1), child(2)]).unwrap(),
        8,
        ComponentLayoutAlignment::Stretch,
        0,
        1,
        0,
        8,
    )
    .unwrap();
    let layout = ComponentLayoutInfo::new(
        BoundedSequence::try_from_iter([ComponentLayoutCandidate::new(
            BoundedSequence::try_from_iter([
                root,
                ComponentLayoutNode::region(0).unwrap(),
                ComponentLayoutNode::Fallback,
            ])
            .unwrap(),
            0,
        )
        .unwrap()])
        .unwrap(),
        "conduit.face/component-default-layout@1".into(),
        "conduit.face/component-default@1".into(),
        1,
    )
    .unwrap()
    .into_structured()
    .unwrap();
    let profile = layout.value_type().profile().unwrap().value_kind().clone();
    kind.startup_parameters.push(FrontStartupParameter {
        name: "layout".into(),
        value_type: profile.clone(),
        has_default: false,
    });
    kind.configuration.push(KindConfigurationField {
        key: "layout".into(),
        default_value: ConfigurationValue::Structured(
            StructuredConfigurationValue::new(profile.clone(), layout.canonical_bytes().unwrap())
                .unwrap(),
        ),
        rule: KindConfigurationRule::Structured { profile },
    });
    kind
}

/// Reviewed @3 structural variants. The old @2 sources retain their own identity.
pub const CRECHE_LAYOUT_MASK_SOURCE: &str =
    include_str!("../components/creche-layout-mask.conduit");
pub const TODO_LAYOUT_MASK_SOURCE: &str = include_str!("../components/todo-layout-mask.conduit");
