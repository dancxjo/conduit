use std::fs;

#[test]
fn portable_time_meaning_has_one_host_neutral_owner() {
    let manifest = include_str!("../Cargo.toml");
    let dependencies = manifest
        .split_once("[dependencies]")
        .expect("time owner declares dependencies")
        .1;
    for forbidden in ["conduit-std-catalog", "conduit-std-host", "hosts/"] {
        assert!(!dependencies.contains(forbidden));
    }

    let former_tick_owner = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../conduit-std-catalog/src/tick.rs"
    );
    let former_every_owner = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../conduit-std-catalog/src/time_every.rs"
    );
    let former_sources = [former_tick_owner, former_every_owner]
        .map(|path| fs::read_to_string(path).expect("read former time semantic owner"));
    for forbidden in [
        "pub const TICK_VALUE_KIND",
        "pub fn encode_tick",
        "pub fn install_tick_pipeline_catalogs",
        "pub const TIME_EVERY_KIND",
        "pub fn install_time_pipeline_catalogs",
    ] {
        assert!(
            former_sources
                .iter()
                .all(|source| !source.contains(forbidden)),
            "portable time truth returned to std ownership: {forbidden}"
        );
    }
}

#[test]
fn semantic_consumers_do_not_import_tick_meaning_from_std_inventory() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for relative in [
        "crates/conduit-alife/src/distributed_expansion.rs",
        "crates/conduit-alife/src/distributed_catalog.rs",
        "crates/conduit-signal/src/trigger.rs",
        "hosts/conduitos/src/timing_plan.rs",
        "hosts/std/src/installed_std/contract.rs",
    ] {
        let source = fs::read_to_string(workspace.join(relative)).expect("read semantic consumer");
        for forbidden in [
            "conduit_std_catalog::TICK_VALUE_KIND",
            "conduit_std_catalog::TICK_ENCODED_LEN",
            "conduit_std_catalog::encode_tick",
            "conduit_std_catalog::decode_tick",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} imports {forbidden}"
            );
        }
    }
}

#[test]
fn calendar_and_scheduling_domains_do_not_return_to_core() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let core_source = workspace.join("crates/conduit-core/src");
    for former_module in [
        "calendar.rs",
        "calendar_proposal.rs",
        "temporal_recurrence.rs",
        "temporal_recurrence_civil.rs",
        "temporal_schedule.rs",
        "temporal_window.rs",
    ] {
        assert!(
            !core_source.join(former_module).exists(),
            "portable time domain returned to conduit-core: {former_module}"
        );
    }

    let core_manifest = fs::read_to_string(workspace.join("crates/conduit-core/Cargo.toml"))
        .expect("read core manifest");
    assert!(
        !core_manifest.contains("conduit-time"),
        "conduit-core must not depend on its higher-level time domain owner"
    );
}
