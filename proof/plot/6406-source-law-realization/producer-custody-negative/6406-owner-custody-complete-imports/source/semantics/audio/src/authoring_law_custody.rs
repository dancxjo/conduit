//! Explicit allocating preparation; legacy metadata-only installation stays unchanged.
use super::*;
use alloc::{collections::BTreeSet, format};
use conduit_plot::{check_syntax_document, parse_syntax_document, CheckedSyntaxDocument};

/// Recheck the exact owner Type Source. Only roots with complete private
/// compiler custody have witnesses; unsupported imported clock contexts remain
/// absent. Public inspection metadata is used only for exact ABI comparison.
pub fn checked_document_with_law_custody() -> Result<CheckedSyntaxDocument, String> {
    let mut catalog = StartupCatalog::new();
    for ty in conduit_data::clock_authoring::checked_types()
        .map_err(|why| format!("Data clock owner metadata: {why:?}"))?
    {
        catalog.insert_checked_native_type(ty.name.clone(), &ty)?;
    }
    let document = check_syntax_document(
        &parse_syntax_document(&super::type_source::source()),
        &catalog,
    )
    .map_err(|why| format!("Audio owner Source: {why:?}"))?;
    let expected = checked_types().map_err(|why| format!("Audio generated metadata: {why:?}"))?;
    if document.native_types.len() != expected.len()
        || expected.iter().any(|expected| {
            !document
                .native_types
                .iter()
                .any(|actual| actual == expected)
        })
    {
        return Err("Audio rechecked owner differs from generated Native metadata".into());
    }
    Ok(document)
}

/// Install only explicitly requested, fully witnessed owner roots. All request
/// bounds are checked before Source parsing or cloning the destination catalog.
/// Missing reachable Source custody refuses the whole transaction.
pub fn install_law_custody_for(startup: &mut StartupCatalog, names: &[&str]) -> Result<(), String> {
    if names.len() > 4096 {
        return Err("Audio requested roots exceed 4096".into());
    }
    let mut unique = BTreeSet::new();
    for name in names {
        if name.is_empty() || name.len() > 4096 || !unique.insert(*name) {
            return Err("Audio requested root is empty, duplicate or overbound".into());
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
                .ok_or_else(|| format!("Audio root '{name}' has no complete Source custody"))?,
        ));
    }
    let mut staged = startup.clone();
    for (name, witness) in witnesses {
        if let Some(existing) = staged.native_law_witness(name) {
            if existing != witness {
                return Err(format!("Audio root '{name}' has foreign custody"));
            }
        } else {
            staged.insert_native_law_witness(name, witness)?;
        }
    }
    *startup = staged;
    Ok(())
}
