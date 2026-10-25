#![cfg(feature = "plot-catalog")]
//! Additive selector syntax changes exact Type/configuration custody.
use conduit_core::*;
use conduit_face::*;
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};

#[test]
fn collection_variant_changes_enclosing_recipe_profile_and_kind_configuration() {
    let collection = "    | collection {\n        mode: Text <= 16B in [\"information\", \"exclusive\", \"independent\"]\n        maximum_items: U16 in 1..=64\n    }\n";
    let source = include_str!("../types.conduit");
    assert_eq!(source.matches(collection).count(), 1);
    let mut catalog = StartupCatalog::new();
    catalog
        .insert_value_kind_alias("Quantity", kind_id(QUANTITY_INFO_ID))
        .unwrap();
    catalog
        .insert_value_kind_alias("ResourceRef", kind_id(RESOURCE_REFERENCE_INFO_ID))
        .unwrap();
    catalog
        .insert_structured_type(
            "ImageObservationReference",
            conduit_human::ImageObservationReference::semantic_type().unwrap(),
        )
        .unwrap();
    catalog
        .insert_structured_type(
            "TemporalInstant",
            conduit_time::NativeTemporalInstant::semantic_type().unwrap(),
        )
        .unwrap();
    let old_source = [
        source.replace(collection, ""),
        include_str!("../look.conduit").into(),
        include_str!("../component-layout.conduit").into(),
    ]
    .join("\n");
    let old = check_syntax_document(&parse_syntax_document(&old_source), &catalog).unwrap();
    let old_type = &old
        .native_types
        .iter()
        .find(|t| t.name == "ComponentRecipeInfo")
        .unwrap()
        .value_type;
    let current = component_recipe_type();
    assert_ne!(old_type, &current);
    assert_ne!(
        old_type.profile().unwrap().value_kind(),
        current.profile().unwrap().value_kind()
    );
    let kind = component_mask_kind(4096);
    let field = kind
        .configuration
        .iter()
        .find(|f| f.key == "recipe")
        .unwrap();
    assert_eq!(
        field.rule,
        KindConfigurationRule::Structured {
            profile: current.profile().unwrap().value_kind().clone(),
        }
    );
    assert_eq!(
        kind.kind_contract_revision.as_str(),
        "conduit.face/component-mask@4"
    );
}
