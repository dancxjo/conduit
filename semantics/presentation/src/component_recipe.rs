//! Source-owned structural regions over the original Face reading and references.
//! Look never enters this preparation, so expression changes cannot rearrange
//! semantics, actions, disclosure, or navigation.
use crate::{
    plan_face_utterances, ComponentRecipeInfo, ComponentSubjectSelector, FaceUtterancePlan,
    FaceUtterancePlanError, Presentation, PresentationRole,
};
use alloc::{collections::BTreeSet, vec, vec::Vec};
use conduit_core::StructuredInfoValue;
use conduit_plot::rust_binding::NativeRustBinding;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComponentRecipeRefusal {
    Encoding,
    EmptyRegions,
    DuplicateRegion,
    OverlappingSlots,
    SlotCapacity,
    InvalidFace,
    ReadingCapacity(FaceUtterancePlanError),
    Geometry,
}

/// Finite references into the supplied immutable Face, not another semantic tree.
pub struct PreparedComponentRecipe {
    pub recipe: ComponentRecipeInfo,
    /// Lookup by original subject index, including every unmatched valid subject.
    /// This does not impose a new renderer navigation/reading order.
    pub subject_regions: Vec<Option<usize>>,
    /// Existing aural reader projection, retaining complete original provenance.
    /// Its private ordering policy is not a universal renderer traversal.
    pub reading: FaceUtterancePlan,
}

pub fn prepare_component_recipe(
    info: StructuredInfoValue,
    face: &Presentation,
) -> Result<PreparedComponentRecipe, ComponentRecipeRefusal> {
    let recipe =
        ComponentRecipeInfo::from_structured(info).map_err(|_| ComponentRecipeRefusal::Encoding)?;
    if recipe.regions().is_empty() {
        return Err(ComponentRecipeRefusal::EmptyRegions);
    }
    face.validate()
        .map_err(|_| ComponentRecipeRefusal::InvalidFace)?;
    let reading = plan_face_utterances(face).map_err(ComponentRecipeRefusal::ReadingCapacity)?;
    let mut identities = BTreeSet::new();
    let mut subject_regions = vec![None; face.subjects.len()];
    for (region_index, region) in recipe.regions().iter().enumerate() {
        if !identities.insert(region.identity()) {
            return Err(ComponentRecipeRefusal::DuplicateRegion);
        }
        let mut count = 0usize;
        for (subject_index, subject) in face.subjects.iter().enumerate() {
            let selected = match region.selector() {
                ComponentSubjectSelector::Subject(identity) => identity == &subject.identity,
                ComponentSubjectSelector::Role(role) => {
                    existing_role_name(&subject.role) == Some(role.as_str())
                }
            };
            if selected {
                if subject_regions[subject_index].is_some() {
                    return Err(ComponentRecipeRefusal::OverlappingSlots);
                }
                count += 1;
                if count > usize::from(*region.maximum_subjects()) {
                    return Err(ComponentRecipeRefusal::SlotCapacity);
                }
                subject_regions[subject_index] = Some(region_index);
            }
        }
    }
    Ok(PreparedComponentRecipe {
        recipe,
        subject_regions,
        reading,
    })
}

/// The broad Face role vocabulary already owned by Presentation. Open semantic
/// roles deliberately remain unmatched and retain complete generic reading.
fn existing_role_name(role: &PresentationRole) -> Option<&'static str> {
    Some(match role {
        PresentationRole::Document => "Document",
        PresentationRole::Body => "Body",
        PresentationRole::Part => "Part",
        PresentationRole::Candidate => "Candidate",
        PresentationRole::Plot => "Plot",
        PresentationRole::Gear => "Gear",
        PresentationRole::Port => "Port",
        PresentationRole::Cord => "Cord",
        PresentationRole::Plan => "Plan",
        PresentationRole::Play => "Play",
        PresentationRole::Host => "Host",
        PresentationRole::Capability => "Capability",
        PresentationRole::Line => "Line",
        PresentationRole::Manifestation => "Manifestation",
        PresentationRole::Route => "Route",
        PresentationRole::Diagnostic => "Diagnostic",
        PresentationRole::Sign => "Sign",
        PresentationRole::Info => "Info",
        PresentationRole::Region => "Region",
        PresentationRole::Collection => "Collection",
        PresentationRole::Item => "Item",
        PresentationRole::TextEntry => "TextEntry",
        PresentationRole::Status => "Status",
        PresentationRole::Action => "Action",
        PresentationRole::Semantic(_) => return None,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComponentGeometry {
    pub columns: u16,
    pub content_width: u16,
    pub column_width: u16,
    pub gutter: u16,
}
impl PreparedComponentRecipe {
    /// Host units are already admitted by the selected renderer. Every unsigned
    /// subtraction is checked before a font/layout engine sees available width.
    pub fn geometry_for_width(
        &self,
        width: u16,
    ) -> Result<ComponentGeometry, ComponentRecipeRefusal> {
        let gutter = *self.recipe.gutter();
        let content_width = width
            .checked_sub(
                gutter
                    .checked_mul(2)
                    .ok_or(ComponentRecipeRefusal::Geometry)?,
            )
            .filter(|value| *value > 0)
            .ok_or(ComponentRecipeRefusal::Geometry)?;
        let columns = if content_width >= *self.recipe.reflow_width() {
            *self.recipe.maximum_columns()
        } else {
            1
        };
        let inter_column_gutters = gutter
            .checked_mul(columns - 1)
            .ok_or(ComponentRecipeRefusal::Geometry)?;
        let column_width = content_width
            .checked_sub(inter_column_gutters)
            .and_then(|value| value.checked_div(columns))
            .filter(|value| *value > 0)
            .ok_or(ComponentRecipeRefusal::Geometry)?;
        Ok(ComponentGeometry {
            columns,
            content_width,
            column_width,
            gutter,
        })
    }
}

/// Exact source-checked schema for the structural startup configuration.
pub fn component_recipe_type() -> conduit_core::StructuredInfoType {
    ComponentRecipeInfo::semantic_type().expect("checked finite component recipe Type")
}

/// Portable configured renderer contract. Both Host Back selections retain this
/// Fore; concrete resource preparation and acknowledgement remain Host-owned.
#[cfg(feature = "plot-catalog")]
pub fn component_mask_kind(maximum_bytes: u32) -> conduit_core::Kind {
    use crate::ComponentRegionInfo;
    use conduit_core::*;
    use conduit_plot::rust_binding::BoundedSequence;
    let recipe = ComponentRecipeInfo::new(
        12,
        "conduit.presentation/component-default@1".into(),
        1,
        720,
        BoundedSequence::try_from_iter([ComponentRegionInfo::new(
            0,
            "collection".into(),
            20,
            ComponentSubjectSelector::role("Collection".into()).unwrap(),
        )
        .unwrap()])
        .unwrap(),
        1,
    )
    .unwrap()
    .into_structured()
    .unwrap();
    let expression = crate::visual_expression_info(&crate::VisualExpression::phosphor()).unwrap();
    let fields = [("recipe", recipe), ("expression", expression)];
    let port = |name: &str, value_kind: &str, direction, temporal| PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal,
        abnormal_kind: None,
    };
    Kind {
        kind_id: kind_id(COMPONENT_MASK_KIND),
        kind_contract_revision: KindIdentity::from("conduit.presentation/component-mask@1"),
        startup_parameters: fields
            .iter()
            .map(|(name, value)| FrontStartupParameter {
                name: (*name).into(),
                value_type: value.value_type().profile().unwrap().value_kind().clone(),
                has_default: false,
            })
            .collect(),
        shorthand: None,
        inputs: vec![port(
            "presentation",
            crate::PRESENTATION_VALUE_KIND,
            PortDirection::Input,
            PortTemporal::Value,
        )],
        outputs: vec![
            port(
                "interaction",
                crate::FACE_INTERACTION_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Flow { closes: true },
            ),
            port(
                "show",
                crate::SHOW_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Value,
            ),
        ],
        configuration: fields
            .into_iter()
            .map(|(name, value)| {
                let profile = value.value_type().profile().unwrap().value_kind().clone();
                KindConfigurationField {
                    key: name.into(),
                    default_value: ConfigurationValue::Structured(
                        StructuredConfigurationValue::new(
                            profile.clone(),
                            value.canonical_bytes().unwrap(),
                        )
                        .unwrap(),
                    ),
                    rule: KindConfigurationRule::Structured { profile },
                }
            })
            .collect(),
        semantic_laws: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 4,
            max_queue_bytes: maximum_bytes,
        },
    }
}

pub const COMPONENT_MASK_KIND: &str = "presentation/component-mask";

/// One checked document owns both expression roots and both independent recipes.
pub const TODO_COMPONENT_MASK_SOURCE: &str = include_str!("../components/todo-mask.conduit");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentMaskLook {
    Compact,
    Spacious,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TodoComponentRecipe {
    Flat,
    Grouped,
}

pub fn todo_component_mask_root(
    look: ComponentMaskLook,
    recipe: TodoComponentRecipe,
) -> &'static str {
    match (recipe, look) {
        (TodoComponentRecipe::Flat, ComponentMaskLook::Compact) => "todo-mask-flat-compact",
        (TodoComponentRecipe::Flat, ComponentMaskLook::Spacious) => "todo-mask-flat-spacious",
        (TodoComponentRecipe::Grouped, ComponentMaskLook::Compact) => "todo-mask-grouped-compact",
        (TodoComponentRecipe::Grouped, ComponentMaskLook::Spacious) => "todo-mask-grouped-spacious",
    }
}
