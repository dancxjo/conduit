//! Unique shipped member names across checked source categories.
use super::*;

pub(super) fn exported_plot_name<'a>(
    members: &'a [CheckedPackageMember],
    export: &str,
) -> Result<&'a str, PackageBundleError> {
    match unique_plot_leaf(members.iter().flat_map(|member| &member.plots), export) {
        Ok(plot) => Ok(plot),
        Err(PlotLeafError::Missing) => Err(PackageBundleError::MissingExport(export.into())),
        Err(PlotLeafError::Ambiguous) => Err(PackageBundleError::AmbiguousExport(export.into())),
    }
}

pub(super) fn exported_type_name<'a>(
    members: &'a [CheckedPackageMember],
    export: &str,
) -> Result<&'a str, PackageBundleError> {
    match unique_plot_leaf(members.iter().flat_map(|member| &member.types), export) {
        Ok(value_type) => Ok(value_type),
        Err(PlotLeafError::Missing) => Err(PackageBundleError::MissingExport(export.into())),
        Err(PlotLeafError::Ambiguous) => Err(PackageBundleError::AmbiguousExport(export.into())),
    }
}

pub(super) fn exported_glyph_notation_name<'a>(
    members: &'a [CheckedPackageMember],
    export: &str,
) -> Result<&'a str, PackageBundleError> {
    match unique_plot_leaf(
        members.iter().flat_map(|member| &member.glyph_notations),
        export,
    ) {
        Ok(name) => Ok(name),
        Err(PlotLeafError::Missing) => Err(PackageBundleError::MissingExport(export.into())),
        Err(PlotLeafError::Ambiguous) => Err(PackageBundleError::AmbiguousExport(export.into())),
    }
}

pub(super) fn exported_member_name<'a>(
    members: &'a [CheckedPackageMember],
    export: &str,
) -> Result<&'a str, PackageBundleError> {
    let mut selected = None;
    for result in [
        exported_plot_name(members, export),
        exported_type_name(members, export),
        exported_glyph_notation_name(members, export),
    ] {
        match result {
            Ok(name) if selected.is_none() => selected = Some(name),
            Err(PackageBundleError::MissingExport(_)) => {}
            _ => return Err(PackageBundleError::AmbiguousExport(export.into())),
        }
    }
    selected.ok_or_else(|| PackageBundleError::MissingExport(export.into()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PlotLeafError {
    Missing,
    Ambiguous,
}

pub(super) fn unique_plot_leaf<'a, I>(plots: I, leaf: &str) -> Result<&'a str, PlotLeafError>
where
    I: IntoIterator<Item = &'a String>,
{
    let mut matches = plots
        .into_iter()
        .filter(|plot| plot.rsplit('/').next().is_some_and(|name| name == leaf));
    let found = matches.next().ok_or(PlotLeafError::Missing)?;
    if matches.next().is_some() {
        return Err(PlotLeafError::Ambiguous);
    }
    Ok(found)
}
