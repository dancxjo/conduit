use std::fs;
use std::path::Path;

#[test]
fn host_fabrication_source_excludes_body_and_spore_orchestration() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for entry in fs::read_dir(&source).expect("read Host fabrication source") {
        let path = entry.expect("read source entry").path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("UTF-8 source filename");
        assert!(
            !name.contains("body") && !name.contains("spore"),
            "Host fabrication owns upper-layer source module {name}"
        );
    }

    let facade = fs::read_to_string(source.join("lib.rs")).expect("read Host fabrication facade");
    for forbidden in [
        "BodyDescription",
        "CheckedBody",
        "SporeManifest",
        "build_body_spores",
        "deployment_receipt",
    ] {
        assert!(
            !facade.contains(forbidden),
            "Host fabrication facade exposes upper-layer contract {forbidden}"
        );
    }
}
