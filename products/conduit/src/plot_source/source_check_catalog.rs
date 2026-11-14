//! Authoring meaning only; initialized equipment and grants remain independent.
use conduit_plot::{ProfileCatalog, StartupCatalog};

pub(super) fn install(
    startup: &mut StartupCatalog,
    profiles: &mut ProfileCatalog,
) -> Result<(), String> {
    // The product's checked compiler equipment supports at most these envelopes.
    // This declaration does not reserve them or manufacture a Host offer.
    conduit_std_host::source_check::PreparedSourceChecker::install_authoring_declaration(
        startup, profiles, 262_144, 65_536,
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn ordinary_source_can_name_the_checker_without_creating_a_provider() {
        let source = super::super::parse(include_str!(
            "../../../../make/body/tests/source_check_call/check.conduit"
        ))
        .unwrap();
        let authored = source.expand_entry_for_authoring().unwrap();
        assert_eq!(authored.expanded.name, "check-source");
        assert_eq!(authored.expanded.gears.len(), 1);
        assert_eq!(
            authored.expanded.gears[0].kind_id.as_str(),
            conduit_std_host::source_check::SOURCE_CHECK_KIND
        );
        let host = conduit_std_host::StdHost::new();
        assert!(!host.advertisement().capabilities.iter().any(|offer| {
            offer.kind_id.as_str() == conduit_std_host::source_check::SOURCE_CHECK_KIND
        }));
        assert!(conduit_planner::default_expanded_placements(
            &authored.expanded,
            &[host.advertisement().clone()],
        )
        .is_err());
    }
}
