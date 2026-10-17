//! Speech Types and their exact refinement laws in ordinary `.conduit` authoring.
#[path = "../build_support/authoring_types.rs"]
mod authoring_types;
#[path = "../build_support/semantic_source.rs"]
// The runtime imports Language identities; complete Speech source is build-only.
#[allow(dead_code)]
mod semantic_source;
use alloc::string::String;
use conduit_plot::StartupCatalog;

/// Domain preparation only. Imports retain the same declarations, bounds and
/// invariants used by generated Native bindings; no renderer or speaking grant.
pub fn install(startup: &mut StartupCatalog) -> Result<(), String> {
    let identities = semantic_source::language_identities()?;
    for (name, ty) in conduit_language::identity_types() {
        if let Some(checked) = identities.native_types.iter().find(|ty| ty.name == name) {
            if checked.value_type != ty {
                return Err(alloc::format!(
                    "Language Type '{name}' differs from its owner"
                ));
            }
            // Language's broad catalog may already have installed the bare
            // shape. A qualified import retains the owner's laws without
            // overwriting that registration or copying its declarations.
            startup.insert_checked_native_type(alloc::format!("language/{name}"), checked)?;
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
            startup.ensure_structured_type(name, ty)?;
        }
    }
    let types = authoring_types::decode(
        include_bytes!(concat!(env!("OUT_DIR"), "/authoring_types.bin")),
        crate::semantic::IPA_CONSTRUCTOR_SOURCE_ID,
    )?;
    for ty in &types {
        startup.insert_checked_native_type(ty.name.clone(), ty)?;
    }
    Ok(())
}
