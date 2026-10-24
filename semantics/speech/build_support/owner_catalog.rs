//! Exact imported owner contracts, upgraded only by real opaque Source custody.
extern crate alloc;
use alloc::{format, string::String};
use conduit_core::StructuredInfoType;
use conduit_plot::StartupCatalog;

pub fn prepare(
    language_types: &[(&str, StructuredInfoType)],
    audio_roots: Option<&[&str]>,
) -> Result<StartupCatalog, String> {
    let audio = conduit_audio::authoring::checked_types()
        .map_err(|why| format!("Audio owner metadata: {why:?}"))?;
    let audio_source = match audio_roots {
        Some(names) => conduit_audio::authoring::checked_document_with_law_custody_for(names)?,
        None => conduit_audio::authoring::checked_document_with_law_custody()?,
    };
    let mut catalog = StartupCatalog::new();
    // Speech generation installs every Audio owner Type, not merely its smaller
    // Rust external-binding PATHS inventory (which omits trajectory outputs).
    // Unwitnessed metadata checks unrelated legacy declarations but cannot issue
    // a requested root's missing complete Source custody.
    for expected in &audio {
        let name = expected.name.as_str();
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
