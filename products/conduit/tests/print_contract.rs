//! Public checker and real default Host availability are separate proof classes.
use conduit_semantic_catalog::{install_print_catalogs, PRINT_SUBMIT_KIND};

#[test]
fn public_print_check_passes_but_default_host_has_no_implicit_print_provider() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../plots/print-contract/main.conduit");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args(["check", source.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "[]");
    let host = conduit_std_host::StdHost::new();
    assert!(host
        .advertisement()
        .capabilities
        .iter()
        .all(|offer| offer.kind_id.as_str() != PRINT_SUBMIT_KIND));
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    install_print_catalogs(&mut startup, &mut profile).unwrap();
    let checked = conduit_plot::parse_with_startup(
        &std::fs::read_to_string(source).unwrap(),
        &startup,
        &profile,
    )
    .unwrap();
    assert!(matches!(
        conduit_planner::default_placements(&checked, core::slice::from_ref(host.advertisement())),
        Err(conduit_planner::PlannerError::UnknownCapability(_))
    ));
}
