//! Provisioned layout remains independent of a particular live Boot/Plan.
use super::planning::fixture_with_boot;
use conduit_environment_make::{DisplayOutputSource, PreparedDisplayOutput};
#[test]
fn trusted_display_layout_is_stable_across_boots_and_omits_live_identity() {
    let source = DisplayOutputSource::compile().unwrap();
    let mut previous = None;
    for boot in ["fixture/boot", "fixture/next-boot"] {
        let (host, grant, _software) = fixture_with_boot(&source, boot);
        let prepared = PreparedDisplayOutput::prepare(
            &source,
            &host,
            &["fixture/base".into(), "conduit.base/local@1".into()],
            &[grant],
        )
        .unwrap();
        let material = prepared.display_layout().unwrap();
        assert_eq!(material.fore().len(), 2);
        assert!(material.records().len() > 20);
        assert_eq!(
            material.source().expanded_plot_id,
            source.plot.expanded.expanded_plot_id
        );
        let mut rust = String::new();
        material.write_rust(&mut rust).unwrap();
        for live in [
            host.host_id.as_str(),
            host.boot_id.as_str(),
            prepared.plan().plan_id.as_str(),
        ] {
            assert!(!rust.contains(live));
        }
        assert!(rust.contains("CONFIGURATIONS"));
        assert!(rust.contains(&format!(
            "MAXIMUM_VALUE_BYTES: u32 = {};",
            prepared.maximum_value_bytes()
        )));
        assert!(rust.contains("AssignedDisplayProfile"));
        if let Some(previous) = &previous {
            assert_eq!(&rust, previous);
        } else {
            previous = Some(rust);
        }
    }
}

#[test]
fn value_extent_covers_every_checked_port_and_refuses_unknown_payload_types() {
    let mut source = DisplayOutputSource::compile().unwrap();
    let extent = source.maximum_value_bytes().unwrap();
    assert!(extent >= conduit_environment_runtime::display_packets::DISPLAY_FRAME_BYTES as u32);
    assert!(extent < conduit_tm1637_wire::MAXIMUM_BYTES);
    assert!(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../targets/esp32/proof/tm1637/src/display_layout.rs"
    ))
    .starts_with(&format!("pub const MAXIMUM_VALUE_BYTES: u32 = {extent};\n")));
    println!(
        "output_type_extent_bytes={extent}, twelve_slot_payload_bytes={}",
        12 * extent
    );
    source.plot.expanded.gears[0].inputs[0].value_kind = "fixture/unknown-type".into();
    assert!(source.maximum_value_bytes().is_err());
}
