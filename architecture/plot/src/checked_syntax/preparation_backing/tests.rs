use super::*;
fn document() -> CheckedSyntaxDocument {
    let parsed = crate::parse_syntax_document(
        "plot relay (\n >> input: Text\n output: Text >>\n) {\n input >> output\n}\n",
    );
    crate::check_syntax_document(&parsed, &crate::StartupCatalog::new()).unwrap()
}
#[test]
fn checked_document_tracks_spare_owned_identity_storage() {
    let mut value = document();
    let before = value.owned_preparation_backing_bytes().unwrap();
    let original = value.source_document_id.0.capacity();
    value.source_document_id.0.reserve(original + 400);
    let delta = value.source_document_id.0.capacity() - original;
    assert_eq!(
        value.owned_preparation_backing_bytes(),
        Some(before + delta)
    );
}
#[test]
fn unsupported_refinements_refuse_instead_of_omitting_backing() {
    let mut value = document();
    let span = value.plots[0].runtime_ports[0].span;
    value.plots[0].runtime_ports[0]
        .refinements
        .push(crate::ValueRefinement::Finite { span });
    assert_eq!(value.owned_preparation_backing_bytes(), None);
}
