//! Build-host preparation from an explicit checked document and catalog.
//! These artifact ceilings are not compiler-workspace or runtime admission.
use conduit_plot as p;

#[derive(Clone, Copy)]
pub struct CheckedWorksetBounds {
    pub maximum_entries: usize,
    pub maximum_nodes_per_entry: usize,
    pub maximum_cords_per_entry: usize,
}

/// Preserve the caller's selected entry order and the ordinary compiler's
/// identities. No application catalog, provider, Source text or default entry
/// is installed here. The caller already owns the checked document's backing.
pub fn prepare_checked_workset(
    document: &p::CheckedSyntaxDocument,
    profiles: &p::ProfileCatalog,
    entries: &[&str],
    bounds: CheckedWorksetBounds,
) -> Result<Vec<p::ExpandedAuthoringPlot>, String> {
    if entries.is_empty()
        || entries.len() > bounds.maximum_entries
        || entries
            .iter()
            .enumerate()
            .any(|(i, name)| name.is_empty() || entries[..i].contains(name))
    {
        return Err("checked workset entry bound or duplicate".into());
    }
    let mut prepared = Vec::with_capacity(entries.len());
    for name in entries {
        let plot = p::expand_canonical_plot_for_authoring(document, name, profiles)
            .map_err(|error| format!("checked workset expansion: {error:?}"))?;
        if plot.expanded.gears.len() > bounds.maximum_nodes_per_entry
            || plot.expanded.connections.len() > bounds.maximum_cords_per_entry
        {
            return Err("checked workset expanded artifact bound".into());
        }
        plot.expanded
            .validate_expansion()
            .map_err(|error| format!("checked workset identity: {error:?}"))?;
        prepared.push(plot);
    }
    Ok(prepared)
}
