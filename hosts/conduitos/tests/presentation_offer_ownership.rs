use std::{fs, path::Path};

#[test]
fn conduitos_presentation_offer_construction_stays_with_conduitos() {
    let former_owner =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/conduit-semantic-catalog/src");
    for entry in fs::read_dir(former_owner).expect("read semantic catalog sources") {
        let entry = entry.expect("read semantic catalog entry");
        if entry.path().extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let source = fs::read_to_string(entry.path()).expect("read semantic catalog source");
        for forbidden in [
            "conduitos_presentation_nucleus_offers",
            "CONDUITOS_PRESENTATION_PROFILE",
            "CONDUITOS_PRESENTATION_ARTIFACT",
        ] {
            assert!(
                !source.contains(forbidden),
                "ConduitOS offer ownership entered the semantic catalog: {forbidden}"
            );
        }
    }
}
