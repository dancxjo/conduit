use std::{fs, path::Path};

#[test]
fn browser_presentation_offer_construction_stays_with_the_browser_host() {
    let former_owner =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/conduit-std-catalog/src");
    for entry in fs::read_dir(former_owner).expect("read std catalog sources") {
        let entry = entry.expect("read std catalog entry");
        if entry.path().extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let source = fs::read_to_string(entry.path()).expect("read std catalog source");
        for forbidden in [
            "browser_presentation_nucleus_offers",
            "browser_human_io_offers",
            "browser_human_io_advertisement_offers",
            "BROWSER_PRESENTATION_PROFILE",
            "BROWSER_PRESENTATION_ARTIFACT",
        ] {
            assert!(
                !source.contains(forbidden),
                "browser offer ownership returned to std catalog: {forbidden}"
            );
        }
    }
}
