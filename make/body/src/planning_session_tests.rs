//! Body-owned planning-session proof using ordinary checked and expanded Forms.

mod execution;
mod proposal;

fn hello_form() -> conduit_form::ExpandedCanonicalForm {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile).unwrap();
    let checked = conduit_form::check_syntax_document(
        &conduit_form::parse_syntax_document(include_str!("../../../forms/hello/main.conduit")),
        &startup,
    )
    .unwrap();
    conduit_form::expand_canonical_form(&checked, "hello", &profile).unwrap()
}
