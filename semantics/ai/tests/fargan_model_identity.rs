use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
#[test]
fn bounded_model_frame_anchor_has_a_checked_transport_budget() {
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("../../speech/fargan_model_identity.conduit")),
        &StartupCatalog::new(),
    )
    .unwrap();
    let ty = &checked
        .native_types
        .iter()
        .find(|ty| ty.name == "FarganModelFrameAnchor")
        .unwrap()
        .value_type;
    let bound = conduit_plot::maximum_prepared_transport_value_bytes(ty).unwrap();
    eprintln!("FarganModelFrameAnchor maximum canonical transport bytes={bound}");
    assert!(
        bound <= 2048,
        "retain room for explicit numerical phase payloads"
    );
}
