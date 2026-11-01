//! Source/typed outcome proof only; installed/native execution is separate.
use conduit_body_make::{deployment_runtime::*, deployment_runtime_catalog::*};

#[test]
fn ordinary_source_checks_with_exact_creche_fore() {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    install_creche_catalogs(&mut startup, &mut profile).unwrap();
    let syntax =
        conduit_plot::parse_syntax_document(include_str!("../../../plots/creche/main.conduit"));
    let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
    conduit_plot::expand_canonical_plot_for_authoring(&checked, "creche/main", &profile).unwrap();
}

#[test]
fn outcomes_keep_scope_and_evidence_identity_distinct() {
    let request = CrecheRequest([11; 32]);
    for disposition in [
        CrecheDisposition::Prepared,
        CrecheDisposition::Realized,
        CrecheDisposition::Refused,
        CrecheDisposition::Cancelled,
    ] {
        let outcome = CrecheOutcome {
            request,
            disposition,
            evidence_digest: [29; 32],
        };
        assert_eq!(CrecheOutcome::decode(&outcome.encode()), Some(outcome));
    }
    let valid = CrecheOutcome {
        request,
        disposition: CrecheDisposition::Prepared,
        evidence_digest: [29; 32],
    }
    .encode();
    assert!(CrecheOutcome::decode(&valid[..64]).is_none());
    let mut invalid = valid;
    invalid[32] = 4;
    assert!(CrecheOutcome::decode(&invalid).is_none());
    assert!(CrecheOutcome::decode(&[0; 66]).is_none());
}
