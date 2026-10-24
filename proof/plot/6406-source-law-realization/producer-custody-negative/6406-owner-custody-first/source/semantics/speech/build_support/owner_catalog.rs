//! Exact imported owner contracts, upgraded only by real opaque Source custody.
extern crate alloc;
use alloc::{format, string::String};
use conduit_core::StructuredInfoType;
use conduit_plot::StartupCatalog;

pub fn prepare(language_types: &[(&str, StructuredInfoType)]) -> Result<StartupCatalog, String> {
    let audio = conduit_audio::authoring::checked_types()
        .map_err(|why| format!("Audio owner metadata: {why:?}"))?;
    let audio_source = conduit_audio::authoring::checked_document_with_law_custody()?;
    let mut catalog = StartupCatalog::new();
    // Keep the same finite Audio import inventory as existing Speech generation.
    // Unwitnessed metadata is retained only to check unrelated legacy declarations;
    // it cannot grant a requested root a complete compiler witness.
    for &(name, _, _) in super::audio_imports::PATHS {
        let expected = audio
            .iter()
            .find(|ty| ty.name == name)
            .ok_or_else(|| format!("missing Audio owner Type '{name}'"))?;
        if let Some(witness) = audio_source.native_law_witness(name) {
            if witness.checked_type() != expected {
                return Err(format!("Audio custody differs for '{name}'"));
            }
            catalog.insert_native_law_witness(name, witness)?;
        } else {
            catalog.insert_checked_native_type(name, expected)?;
        }
    }
    let identities = super::semantic_source::language_identities()?;
    for (name, ty) in language_types {
        if let Some(checked) = identities
            .native_types
            .iter()
            .find(|native| native.name == *name)
        {
            if &checked.value_type != ty {
                return Err(format!("Language owner Type differs for '{name}'"));
            }
            if let Some(witness) = identities.native_law_witness(name) {
                if witness.checked_type() != checked {
                    return Err(format!("Language custody differs for '{name}'"));
                }
                catalog.insert_native_law_witness(*name, witness)?;
            } else {
                catalog.insert_checked_native_type(*name, checked)?;
            }
        } else {
            catalog.insert_structured_type(*name, ty.clone())?;
        }
    }
    Ok(catalog)
}
