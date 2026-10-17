//! Consumer installation order reuses one exact Language-owned request schema.
use conduit_language::*;
use conduit_plot::{ProfileCatalog, StartupCatalog};

#[test]
fn shared_request_registration_is_idempotent_before_the_language_catalog() {
    let mut startup = StartupCatalog::new();
    install_language_request_type(&mut startup).unwrap();
    install_language_request_type(&mut startup).unwrap();
    install_linguistics_catalogs(&mut startup, &mut ProfileCatalog::new()).unwrap();
    assert_eq!(
        startup.structured_type_name(&language_request_profile()),
        Some("LanguageRequest")
    );
}

#[test]
fn conflicting_namesake_and_value_alias_cannot_replace_the_owner_schema() {
    let mut startup = StartupCatalog::new();
    startup
        .insert_structured_type("LanguageRequest", LanguageId::semantic_type().unwrap())
        .unwrap();
    assert!(install_language_request_type(&mut startup).is_err());
    let mut startup = StartupCatalog::new();
    startup
        .insert_value_kind_alias("LanguageRequest", conduit_core::kind_id("value/text"))
        .unwrap();
    assert!(install_language_request_type(&mut startup).is_err());
}
