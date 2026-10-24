//! Explicit Source replay during preparation, never the legacy metadata install.
use super::*;
use alloc::{collections::BTreeSet, format};
use conduit_plot::{
    check_syntax_document, check_syntax_document_with_native_law_roots, parse_syntax_document,
    CheckedSyntaxDocument,
};

/// Recheck the exact Source used by Speech Native generation with real available
/// Audio and Language identity custody. Requested roots must still have complete
/// private witnesses; unrelated metadata-only legacy contexts remain unsupported.
pub fn checked_document_with_law_custody() -> Result<CheckedSyntaxDocument, String> {
    check_owner(None)
}

/// Retain complete compiler custody for exactly these requested Native roots,
/// before optional registry filling. Real missing/overbound closures refuse the
/// complete call; the exact original Source and generated ABI stay unchanged.
pub fn checked_document_with_law_custody_for(
    names: &[&str],
) -> Result<CheckedSyntaxDocument, String> {
    validate_names(names)?;
    check_owner(Some(names))
}

fn check_owner(names: Option<&[&str]>) -> Result<CheckedSyntaxDocument, String> {
    let mut language_types = conduit_language::identity_types();
    language_types.extend(
        conduit_language::prosody::prosody_types()
            .into_iter()
            .filter(|(name, _)| {
                matches!(
                    *name,
                    "LanguageProsodyChoice"
                        | "LanguageProsodyBoundary"
                        | "LanguageProsodyProminence"
                        | "LanguageProsodyPitch"
                )
            }),
    );
    let audio_names = names
        .map(super::law_dependencies::audio_roots_for)
        .transpose()?;
    let audio_refs = audio_names
        .as_ref()
        .map(|names| names.iter().map(String::as_str).collect::<Vec<_>>());
    let catalog = super::owner_catalog::prepare(&language_types, audio_refs.as_deref())?;
    let syntax = parse_syntax_document(&super::semantic_source::source());
    let document = match names {
        Some(names) => check_syntax_document_with_native_law_roots(&syntax, &catalog, names),
        None => check_syntax_document(&syntax, &catalog),
    }
    .map_err(|why| format!("Speech owner Source: {why:?}"))?;
    let expected = checked_types()?;
    if document.native_types.len() != expected.len()
        || expected
            .iter()
            .any(|expected| !document.native_types.contains(expected))
    {
        return Err("Speech rechecked owner differs from generated Native metadata".into());
    }
    Ok(document)
}

/// Selectively and transactionally retain fully witnessed owner roots. Request
/// bounds precede parsing and catalog cloning; no missing preimage fallback exists.
pub fn install_law_custody_for(startup: &mut StartupCatalog, names: &[&str]) -> Result<(), String> {
    validate_names(names)?;
    if names.is_empty() {
        return Ok(());
    }
    let document = checked_document_with_law_custody_for(names)?;
    let mut witnesses = Vec::with_capacity(names.len());
    for name in names {
        witnesses.push((
            *name,
            document
                .native_law_witness(name)
                .ok_or_else(|| format!("Speech root '{name}' has no complete Source custody"))?,
        ));
    }
    let mut staged = startup.clone();
    for (name, witness) in witnesses {
        if let Some(existing) = staged.native_law_witness(name) {
            if existing != witness {
                return Err(format!("Speech root '{name}' has foreign custody"));
            }
        } else {
            staged.insert_native_law_witness(name, witness)?;
        }
    }
    *startup = staged;
    Ok(())
}

fn validate_names(names: &[&str]) -> Result<(), String> {
    if names.len() > 4096 {
        return Err("Speech requested roots exceed 4096".into());
    }
    let mut unique = BTreeSet::new();
    for name in names {
        if name.is_empty() || name.len() > 4096 || !unique.insert(*name) {
            return Err("Speech requested root is empty, duplicate or overbound".into());
        }
    }
    Ok(())
}
