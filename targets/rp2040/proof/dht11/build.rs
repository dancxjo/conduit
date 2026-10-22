#[path = "kernel_image.rs"]
mod kernel_image;
fn main() {
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::write(out.join("memory.x"), include_bytes!("memory.x")).unwrap();
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rustc-link-arg=-Tlink.x");
    println!("cargo:rustc-link-arg=--nmagic");
    println!("cargo:rerun-if-changed=memory.x");
    prepare_source(&out);
    if std::env::var_os("CARGO_FEATURE_KERNEL_EVALUATION").is_some() {
        kernel_image::prepare(&out);
    }
}

fn prepare_source(out: &std::path::Path) {
    use conduit_core::ConfigurationValue;
    use conduit_plot::{
        check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
        PortableExpressionProgram, ProfileCatalog, StartupCatalog,
    };
    use sha2::{Digest, Sha256};
    let path = "../../../../plots/device-protocols/dht11.conduit";
    println!("cargo:rerun-if-changed={path}");
    let source = std::fs::read(path).unwrap();
    let digest = format!("{:x}", Sha256::digest(&source));
    println!("cargo:rustc-env=CONDUIT_DHT11_SOURCE_SHA256={digest}");
    // Bind lifecycle requests to this diagnostic's source inputs and feature.
    let mut build_hash = Sha256::new();
    build_hash.update(&source);
    for file in [
        "Cargo.toml",
        "Cargo.lock",
        "build.rs",
        "memory.x",
        "src/main.rs",
        "src/display_probe.rs",
        "src/http_ap.rs",
        "../../../../bodies/environment/http/src/lib.rs",
        "../../../../bodies/environment/http/src/mask.rs",
        "src/http_radio.rs",
        "../../tm1637/src/lib.rs",
        "../../../../mechanisms/io/tm1637/src/lib.rs",
        "src/source.rs",
        "src/probe_heap.rs",
        "src/usb_link.rs",
        "src/kernel_probe.rs",
        "src/kernel_preparation.rs",
        "src/kernel_execution.rs",
        "src/kernel_back.rs",
        "src/kernel_authority.rs",
        "src/kernel_report.rs",
        "src/kernel_manifest.rs",
        "../../../../bodies/environment/profiles/sampling.conduit",
        "../../../../semantics/time/src/tick_back.rs",
        "../../../../semantics/time/src/tick_trigger.rs",
        "../../../../bodies/environment/face/src/lib.rs",
        "../../../../bodies/environment/face/src/prepared.rs",
        "../../../../bodies/environment/face/src/result.rs",
        "../../../../semantics/face/src/identity.rs",
        "../../../../semantics/face/src/face.rs",
        "../../../../semantics/face/src/semantics.rs",
        "../../../../architecture/core/src/value_constraint/identity.rs",
        "../../../../plots/seven-segment-mask/types.conduit",
        "../../../../plots/seven-segment-mask/src/format.rs",
        "../../../../plots/seven-segment-mask/src/projection.rs",
        "src/kernel_fault.rs",
        "kernel_image.rs",
        "../../../../architecture/core/src/plan_fingerprint.rs",
        "../../../../architecture/core/src/plan_fingerprint/fragment.rs",
        "../../../../plots/device-protocols/dht11-read.conduit",
        "../../../../plots/device-protocols/dht11-observe.conduit",
        "../../../../bodies/environment/runtime/src/observation.rs",
        "../../../../bodies/environment/runtime/src/observation/back.rs",
        "../../../../bodies/environment/runtime/tests/support/plan.rs",
        "../../pulse-capture/src/lib.rs",
        "../../../../mechanisms/io/pulse-capture/src/lib.rs",
    ] {
        println!("cargo:rerun-if-changed={file}");
        build_hash.update(std::fs::read(file).unwrap());
    }
    build_hash.update(std::env::var("CARGO_FEATURE_HTTP_AP").unwrap_or_default());
    build_hash.update(std::env::var("CARGO_FEATURE_DISPLAY_WIRE").unwrap_or_default());
    build_hash.update(std::env::var("CARGO_FEATURE_SOURCE_EVALUATION").unwrap_or_default());
    build_hash.update(std::env::var("CARGO_FEATURE_PIO_CAPTURE").unwrap_or_default());
    build_hash.update(std::env::var("CARGO_FEATURE_KERNEL_EVALUATION").unwrap_or_default());
    let mut capture_source = String::new();
    for file in ["pulse-types.conduit", "dht11-capture.conduit"] {
        let path = format!("../../../../plots/device-protocols/{file}");
        println!("cargo:rerun-if-changed={path}");
        let text = std::fs::read_to_string(path).unwrap();
        capture_source.push_str(&text);
        capture_source.push('\n');
        build_hash.update(text.as_bytes());
    }
    println!(
        "cargo:rustc-env=CONDUIT_DHT11_CAPTURE_SOURCE_SHA256={:x}",
        Sha256::digest(capture_source.as_bytes())
    );
    let syntax = parse_syntax_document(&capture_source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked_capture = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded_capture = expand_canonical_plot_for_authoring(
        &checked_capture,
        "dht11-capture-request",
        &ProfileCatalog::new(),
    )
    .unwrap()
    .expanded;
    assert_eq!(expanded_capture.gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &expanded_capture.gears[0].configuration[0].value
    else {
        panic!("capture Source");
    };
    std::fs::write(
        out.join("capture.program"),
        PortableExpressionProgram::from_canonical_hex(encoded)
            .unwrap()
            .canonical_bytes()
            .unwrap(),
    )
    .unwrap();
    println!(
        "cargo:rustc-env=CONDUIT_DHT11_BUILD_ID={:x}",
        build_hash.finalize()
    );
    let syntax = parse_syntax_document(std::str::from_utf8(&source).unwrap());
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "dht11-decode", &ProfileCatalog::new())
            .unwrap()
            .expanded;
    assert_eq!(expanded.gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("Source expression");
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    std::fs::write(
        out.join("dht11.program"),
        program.canonical_bytes().unwrap(),
    )
    .unwrap();
    let forms = conduit_plot::rust_binding::generate_rust_bindings_with_forms(
        &[],
        &checked.type_forms,
        &Default::default(),
    )
    .unwrap();
    std::fs::write(out.join("forms.rs"), forms.source).unwrap();
}
