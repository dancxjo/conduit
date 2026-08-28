use std::fs;
use std::path::Path;

#[test]
fn portable_network_source_excludes_target_and_r1_realization() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let forbidden_names = ["pico", "r1", "dhcp", "usb"];
    let forbidden_protocol = ["CONDUIT_R1_", "PicoAppliance"];

    for entry in fs::read_dir(&source).expect("read conduit-net source directory") {
        let path = entry.expect("read source entry").path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("UTF-8 source filename");
        assert!(
            forbidden_names.iter().all(|token| !name.contains(token)),
            "portable conduit-net source owns target/proof module {name}"
        );
        let contents = fs::read_to_string(&path).expect("read portable network source");
        for token in forbidden_protocol {
            assert!(
                !contents.contains(token),
                "portable conduit-net source {} contains target/proof token {token}",
                path.display()
            );
        }
    }
}
