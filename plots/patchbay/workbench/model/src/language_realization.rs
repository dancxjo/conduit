//! Optional realization details derived from authoritative checked Language work.
use crate::{GearRealizationError, PatchbaySubjectRef};
use conduit_core::HostAdvertisement;
use conduit_planner::{inspect_language_coverage, LanguageCoverageCandidateEvidence};
use conduit_plot::ExpandedCanonicalPlot;

/// Requests and every exact Fore-compatible current Back, including coverage refusals.
/// This is a details projection; selection still passes the mandatory planner gate.
pub fn language_realization_details(
    plot: &ExpandedCanonicalPlot,
    subject: &PatchbaySubjectRef,
    hosts: &[HostAdvertisement],
) -> Result<Vec<LanguageCoverageCandidateEvidence>, GearRealizationError> {
    if subject.expanded_plot_id != plot.expanded_plot_id {
        return Err(GearRealizationError::StaleSubject);
    }
    let graph =
        crate::PatchbayGraph::from_expanded(plot).map_err(|_| GearRealizationError::UnknownGear)?;
    let graphical = graph
        .gears
        .iter()
        .find(|gear| gear.identity == subject.subject_identity)
        .ok_or(GearRealizationError::UnknownGear)?;
    let checked = plot
        .gears
        .iter()
        .find(|gear| gear.gear_id == graphical.gear_id)
        .ok_or(GearRealizationError::UnknownGear)?;
    inspect_language_coverage(checked, hosts).map_err(GearRealizationError::Planning)
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{BootId, HostId, HostProfileId, OfferGeneration, PROTOCOL_VERSION};
    #[test]
    fn details_keep_french_request_and_current_english_candidate_refusal() {
        let source = r#"plot language-details {
            tokens: language/tokenize-four(text = "Bonjour les amis.", language-request = { language: "language/french", variety: none(""), variety_policy: language_sufficient("") })
        }"#;
        let mut startup = conduit_plot::StartupCatalog::new();
        let mut profile = conduit_plot::ProfileCatalog::new();
        conduit_language::install_linguistics_catalogs(&mut startup, &mut profile).unwrap();
        let checked = conduit_plot::check_syntax_document(
            &conduit_plot::parse_syntax_document(source),
            &startup,
        )
        .unwrap();
        let plot = conduit_plot::expand_canonical_plot_for_authoring(
            &checked,
            "language-details",
            &profile,
        )
        .unwrap()
        .expanded;
        let graph = crate::PatchbayGraph::from_expanded(&plot).unwrap();
        let subject = graph.subject_ref(&graph.gears[0].identity).unwrap();
        let host = HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: HostId::from("details/host"),
            boot_id: BootId::from("details/boot"),
            offer_generation: OfferGeneration(1),
            profile: HostProfileId::from("details/profile"),
            bases: vec![],
            resources: vec![],
            planner_capabilities: vec![],
            capabilities: conduit_std_host::hosted_linguistics::linguistics_std_offers(),
        };
        let details = language_realization_details(&plot, &subject, &[host]).unwrap();
        assert_eq!(details.len(), 1);
        assert_eq!(
            details[0].checks[0].requirement.request.language().get(),
            "language/french"
        );
        assert!(details[0].checks[0].result.is_err());
        assert_eq!(
            details[0].checks[0].coverage_revision.as_deref(),
            Some("four-token-rules@1")
        );
        let mut stale = subject;
        stale.expanded_plot_id = "stale".into();
        assert_eq!(
            language_realization_details(&plot, &stale, &[]),
            Err(GearRealizationError::StaleSubject)
        );
    }
}
