//! Explicit checked entry selection shares one canonical expansion path.
use super::{dsp_source_backs_for, CanonicalSource};

impl CanonicalSource {
    // The library and binary compile this source boundary independently.
    #[allow(dead_code)]
    pub(crate) fn expand_entry_for_authoring(
        &self,
    ) -> Result<conduit_plot::ExpandedAuthoringPlot, String> {
        let checked = self.check()?;
        self.expand_checked_for_authoring(&checked, self.authored_entry_name()?)
    }

    #[allow(dead_code)] // Used by the binary owner; the library compiles this boundary independently.
    pub(crate) fn expand_named_for_authoring(
        &self,
        entry: &str,
    ) -> Result<conduit_plot::ExpandedAuthoringPlot, String> {
        let checked = self.check()?;
        self.expand_checked_for_authoring(&checked, entry)
    }

    fn expand_checked_for_authoring(
        &self,
        checked: &conduit_plot::CheckedSyntaxDocument,
        entry: &str,
    ) -> Result<conduit_plot::ExpandedAuthoringPlot, String> {
        let Some(backs) = dsp_source_backs_for(checked)? else {
            return conduit_plot::expand_canonical_plot_for_authoring(
                checked,
                entry,
                &self.profiles,
            )
            .map_err(|diagnostic| diagnostic.to_string());
        };
        conduit_plot::expand_canonical_plot_for_authoring_with_backs(
            checked,
            entry,
            &self.profiles,
            backs,
        )
        .map_err(|diagnostic| diagnostic.to_string())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn malformed_source_preserves_checker_diagnostic_before_entry_selection() {
        let source = super::super::parse("plot unfinished (").unwrap();
        let diagnostic = source.check().unwrap_err();
        assert_eq!(source.expand_entry_for_authoring().unwrap_err(), diagnostic);
        assert_eq!(
            source.expand_named_for_authoring("absent").unwrap_err(),
            diagnostic
        );
    }
}
