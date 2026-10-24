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
        assert!(rust.contains("AssignedDisplayProfile"));
        if let Some(previous) = &previous {
            assert_eq!(&rust, previous);
        } else {
            previous = Some(rust);
        }
    }
}
