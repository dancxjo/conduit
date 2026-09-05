#![cfg(feature = "form-catalog")]

use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_semantic_catalog::*;

#[test]
fn image_text_composition_is_an_ordinary_browser_neutral_form() {
    let source = include_str!("../../../forms/image-text-compose/main.conduit");
    let lower = source.to_ascii_lowercase();
    for forbidden in [
        "browser",
        "dom",
        "canvas",
        "getusermedia",
        "device",
        "permission",
        "indexeddb",
        "socket",
        "transport",
        "host",
    ] {
        assert!(!lower.contains(forbidden), "Form contains {forbidden}");
    }

    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_human_media_catalogs(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "image-text-compose", &profile).unwrap();

    assert_eq!(authored.input_bindings.len(), 2);
    assert_eq!(authored.output_bindings.len(), 1);
    assert_eq!(authored.expanded.gears.len(), 1);
    assert_eq!(
        authored.expanded.gears[0].kind_id.as_str(),
        IMAGE_TEXT_COMPOSE_KIND
    );
    assert_eq!(authored.expanded.connections.len(), 0);
}

#[test]
fn composed_schema_is_finite_and_keeps_image_as_a_resource_reference() {
    use conduit_core::StructuredInfoTypeShape;

    let record = image_text_record_type();
    let StructuredInfoTypeShape::Record { fields, .. } = record.shape() else {
        panic!("image-text result must be a record");
    };
    let image = fields.iter().find(|field| field.name() == "image").unwrap();
    assert!(matches!(
        image.value_type().shape(),
        StructuredInfoTypeShape::Leaf(identity)
            if identity.as_str() == conduit_core::RESOURCE_REFERENCE_INFO_ID
    ));
    let metadata = fields
        .iter()
        .find(|field| field.name() == "metadata")
        .unwrap();
    assert!(matches!(
        metadata.value_type().shape(),
        StructuredInfoTypeShape::Collection { length, .. }
            if usize::from(length) == conduit_human::MAXIMUM_IMAGE_TEXT_METADATA_ENTRIES
    ));
}
