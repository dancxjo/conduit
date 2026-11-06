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
                self.profiles,
            )
            .map_err(|diagnostic| diagnostic.to_string());
        };
        conduit_plot::expand_canonical_plot_for_authoring_with_backs(
            checked,
            entry,
            self.profiles,
            backs,
        )
        .map_err(|diagnostic| diagnostic.to_string())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn source_preparation_reports_current_thread_requested_backing() {
        let probe = crate::allocation_probe::begin();
        let source =
            super::super::parse(include_str!("../../../../plots/clock/main.conduit")).unwrap();
        let checked = source.expand_entry_for_authoring().unwrap();
        let statistics = probe.finish_statistics();
        assert!(statistics.allocations > 0);
        assert!(statistics.peak_live_bytes >= statistics.live_bytes);
        println!("Source current-thread allocation diagnostic: {statistics:?}");
        let warm_probe = crate::allocation_probe::begin();
        let second =
            super::super::parse(include_str!("../../../../plots/clock/main.conduit")).unwrap();
        let second_checked = second.expand_entry_for_authoring().unwrap();
        let warm = warm_probe.finish_statistics();
        println!("Source warm-catalog current-thread allocation diagnostic: {warm:?}");
        let copy_probe = crate::allocation_probe::begin();
        let catalog_copy = super::super::standard_catalogs().unwrap();
        let copied = copy_probe.finish_statistics();
        println!("Explicit immutable catalog copy allocation diagnostic: {copied:?}");
        std::hint::black_box((&source, &checked, &second, &second_checked, &catalog_copy));
    }

    #[test]
    fn owned_capture_keeps_its_original_source_allocation() {
        let captured = String::from(include_str!("../../../../plots/clock/main.conduit"));
        let original = captured.as_ptr();
        let source = super::super::parse_owned(captured).unwrap();
        assert_eq!(source.source.as_ptr(), original);
        assert!(source.expand_entry_for_authoring().is_ok());
    }

    #[test]
    fn canonical_sources_share_immutable_catalogs_without_sharing_syntax() {
        let first =
            super::super::parse(include_str!("../../../../plots/clock/main.conduit")).unwrap();
        let second =
            super::super::parse(include_str!("../../../../plots/clock/main.conduit")).unwrap();
        assert!(core::ptr::eq(first.startup, second.startup));
        assert!(core::ptr::eq(first.profiles, second.profiles));
        assert_ne!(first.source.as_ptr(), second.source.as_ptr());
        assert!(first.expand_entry_for_authoring().is_ok());
        assert!(second.expand_entry_for_authoring().is_ok());
    }

    #[test]
    fn ordinary_i2c_source_checks_without_inventing_a_local_controller() {
        let source = super::super::parse(include_str!(
            "../../../../plots/device-protocols/i2c.conduit"
        ))
        .unwrap();
        let authored = source.expand_entry_for_authoring().unwrap();
        assert_eq!(authored.expanded.gears.len(), 1);
        assert_eq!(
            authored.expanded.gears[0].kind_id.as_str(),
            conduit_i2c_transaction::contract::I2C_KIND
        );
        let host = conduit_std_host::StdHost::new();
        assert!(!host.advertisement().capabilities.iter().any(|offer| {
            offer.kind_id.as_str() == conduit_i2c_transaction::contract::I2C_KIND
        }));
        assert!(conduit_planner::default_expanded_placements(
            &authored.expanded,
            &[host.advertisement().clone()],
        )
        .is_err());
    }

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
