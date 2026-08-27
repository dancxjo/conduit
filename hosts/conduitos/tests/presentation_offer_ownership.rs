use std::{fs, path::Path};

#[test]
fn conduitos_presentation_offer_construction_stays_with_conduitos() {
    let former_owner =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/conduit-std-catalog/src");
    for entry in fs::read_dir(former_owner).expect("read std catalog sources") {
        let entry = entry.expect("read std catalog entry");
        if entry.path().extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let source = fs::read_to_string(entry.path()).expect("read std catalog source");
        for forbidden in [
            "conduitos_presentation_nucleus_offers",
            "CONDUITOS_PRESENTATION_PROFILE",
            "CONDUITOS_PRESENTATION_ARTIFACT",
        ] {
            assert!(
                !source.contains(forbidden),
                "ConduitOS offer ownership returned to std catalog: {forbidden}"
            );
        }
    }
}
