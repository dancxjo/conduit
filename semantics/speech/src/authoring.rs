//! Speech Types and their exact refinement laws in ordinary `.conduit` authoring.
#[path = "../build_support/semantic_source.rs"]
mod semantic_source;
use alloc::string::String;
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};

/// Domain preparation only. Imports retain the same declarations, bounds and
/// invariants used by generated Native bindings; no renderer or speaking grant.
pub fn install(startup: &mut StartupCatalog) -> Result<(), String> {
    let mut basis = StartupCatalog::new();
    let identities = check_syntax_document(
        &parse_syntax_document(include_str!("../../language/identity.conduit")),
        &StartupCatalog::new(),
    )
    .map_err(|error| alloc::format!("{error:?}"))?;
    for (name, ty) in conduit_language::identity_types() {
        if let Some(checked) = identities.native_types.iter().find(|ty| ty.name == name) {
            if checked.value_type != ty {
                return Err(alloc::format!(
                    "Language Type '{name}' differs from its owner"
                ));
            }
            basis.insert_checked_native_type(name, checked)?;
            // Language's broad catalog may already have installed the bare
            // shape. A qualified import retains the owner's laws without
            // overwriting that registration or copying its declarations.
            startup.insert_checked_native_type(alloc::format!("language/{name}"), checked)?;
        } else {
            basis.insert_structured_type(name, ty.clone())?;
        }
        startup.ensure_structured_type(name, ty)?;
    }
    for (name, ty) in conduit_language::prosody::prosody_types() {
        if matches!(
            name,
            "LanguageProsodyChoice"
                | "LanguageProsodyBoundary"
                | "LanguageProsodyProminence"
                | "LanguageProsodyPitch"
        ) {
            basis.insert_structured_type(name, ty.clone())?;
            startup.ensure_structured_type(name, ty)?;
        }
    }
    let source = semantic_source::source();
    let checked = check_syntax_document(&parse_syntax_document(&source), &basis)
        .map_err(|error| alloc::format!("{error:?}"))?;
    for ty in &checked.native_types {
        startup.insert_checked_native_type(ty.name.clone(), ty)?;
    }
    Ok(())
}
