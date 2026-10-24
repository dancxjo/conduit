//! Explicit Source replay during preparation, never the legacy metadata install.
use super::*;
use alloc::{collections::BTreeSet, format};
use conduit_plot::{check_syntax_document, parse_syntax_document, CheckedSyntaxDocument};

/// Recheck the exact Source used by Speech Native generation with real available
/// Audio and Language identity custody. Requested roots must still have complete
/// private witnesses; unrelated metadata-only legacy contexts remain unsupported.
pub fn checked_document_with_law_custody() -> Result<CheckedSyntaxDocument, String> {
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
    let catalog = super::owner_catalog::prepare(&language_types)?;
    let document = check_syntax_document(
        &parse_syntax_document(&super::semantic_source::source()),
        &catalog,
    )
    .map_err(|why| format!("Speech owner Source: {why:?}"))?;
    let expected = checked_types()?;
    if document.native_types.len() != expected.len()
        || expected.iter().any(|expected| {
            !document
                .native_types
                .iter()
                .any(|actual| actual == expected)
        })
    {
        return Err("Speech rechecked owner differs from generated Native metadata".into());
    }
    Ok(document)
}

/// Selectively and transactionally retain fully witnessed owner roots. Request
/// bounds precede parsing and catalog cloning; no missing preimage fallback exists.
pub fn install_law_custody_for(startup: &mut StartupCatalog, names: &[&str]) -> Result<(), String> {
    if names.len() > 4096 {
        return Err("Speech requested roots exceed 4096".into());
    }
    let mut unique = BTreeSet::new();
    for name in names {
        if name.is_empty() || name.len() > 4096 || !unique.insert(*name) {
            return Err("Speech requested root is empty, duplicate or overbound".into());
        }
    }
    if names.is_empty() {
        return Ok(());
    }
    let document = checked_document_with_law_custody()?;
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
