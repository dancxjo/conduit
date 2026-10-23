//! Canonical value aliases and checking definitions for ordinary Mask Plots.
//! Semantic owners pin exact types and kind fronts here. Host offers, selected
//! implementations and execution authority remain separate admission facts.

use conduit_core::kind_id;
use conduit_plot::StartupCatalog;

use crate::{FACE_INTERACTION_VALUE_KIND, FACE_VALUE_KIND, SHOW_VALUE_KIND};

pub fn install_mask_plot_value_aliases(
    startup: &mut StartupCatalog,
) -> Result<(), alloc::string::String> {
    startup.insert_value_kind_alias("Face", kind_id(FACE_VALUE_KIND))?;
    startup.insert_value_kind_alias("FaceInteraction", kind_id(FACE_INTERACTION_VALUE_KIND))?;
    startup.insert_value_kind_alias("Show", kind_id(SHOW_VALUE_KIND))
}

/// Checking definitions only: availability still requires an exact Host Back.
#[cfg(feature = "plot-catalog")]
pub(crate) fn install_component_mask_catalog(
    startup: &mut StartupCatalog,
    profiles: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    for (alias, schema) in [
        ("ComponentRecipeInfo", crate::component_recipe_type()),
        ("VisualExpressionInfo", crate::visual_expression_type()),
    ] {
        let profile = schema
            .profile()
            .map_err(|error| alloc::format!("{error:?}"))?;
        startup.ensure_structured_type(alias, schema.clone())?;
        startup.ensure_structured_type(profile.value_kind().as_str(), schema)?;
    }
    let kind = crate::component_mask_kind(crate::MAX_FACE_TOTAL_BYTES as u32);
    startup.insert(conduit_plot::KindSignature {
        kind: kind.kind_id.as_str().into(),
        startup_parameters: kind
            .startup_parameters
            .iter()
            .map(|parameter| conduit_plot::StartupParameterSignature {
                name: parameter.name.clone(),
                value_type: parameter.value_type.as_str().into(),
                default: None,
            })
            .collect(),
    })?;
    profiles
        .insert_kind(kind)
        .map_err(|error| alloc::format!("install component Mask checking profile: {error:?}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_role_names_resolve_to_canonical_semantic_values() {
        let mut startup = StartupCatalog::new();
        install_mask_plot_value_aliases(&mut startup).unwrap();
        let source = conduit_plot::parse_syntax_document(
            "plot role (\n    >> face: Face\n    interaction: FaceInteraction...| >>\n    show: Show >>\n) {\n}\n",
        );
        let checked = conduit_plot::check_syntax_document(&source, &startup).unwrap();
        let front = &checked.plots[0].runtime_front;
        assert_eq!(front.inputs()[0].value_kind.as_str(), FACE_VALUE_KIND);
        assert_eq!(
            front.outputs()[0].value_kind.as_str(),
            FACE_INTERACTION_VALUE_KIND
        );
        assert_eq!(front.outputs()[1].value_kind.as_str(), SHOW_VALUE_KIND);
    }

    #[cfg(feature = "plot-catalog")]
    #[test]
    fn ordinary_mask_catalog_checks_the_reviewed_component_document() {
        let mut startup = StartupCatalog::new();
        let mut profiles = conduit_plot::ProfileCatalog::new();
        install_mask_plot_value_aliases(&mut startup).unwrap();
        crate::install_mask_mechanism_catalog(&mut startup, &mut profiles).unwrap();
        let syntax = conduit_plot::parse_syntax_document(crate::TODO_COMPONENT_MASK_SOURCE);
        let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
        assert!(checked
            .plots
            .iter()
            .any(|plot| plot.name == "todo-mask-required-item-spacious"));
    }
}
