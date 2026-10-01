use conduit_form::rust_binding::{generate_rust_bindings_with_codes, RustBindingOptions};
use conduit_form::{
    check_syntax_document, generate_ecmascript_codes, parse_syntax_document, StartupCatalog,
};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    let mut catalog = StartupCatalog::new();
    catalog
        .insert_value_kind_alias(
            "ResourceRef",
            conduit_form::rust_binding::semantic_core::kind_id(
                conduit_form::rust_binding::semantic_core::RESOURCE_REFERENCE_INFO_ID,
            ),
        )
        .expect("resource references are one exact portable leaf");
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("types.conduit")),
        &catalog,
    )
    .expect("data semantic Types must check");
    let generated = generate_rust_bindings_with_codes(
        &checked.native_types,
        &checked.codes,
        &RustBindingOptions {
            boxed_variant_payloads: ["TabularQueryOutcomeFour.inline".into()].into(),
            copy_record_types: [
                "DataGenerationNamespace".into(),
                "MeasurementPlotPoint".into(),
                "MeasurementThresholdPolicy".into(),
            ]
            .into(),
            copy_nominal_types: ["DataGenerationDigest".into()].into(),
            hash_nominal_types: ["DataGenerationDigest".into()].into(),
            copy_record_value_getters: ["DataGenerationNamespace".into()].into(),
            record_constructor_names: [("DataGenerationNamespace".into(), "from_digest".into())]
                .into(),
            ..RustBindingOptions::default()
        },
    )
    .expect("data semantic Types and codes must generate exact Rust bindings");
    let output_directory = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"));
    let output = output_directory.join("semantic_types.rs");
    fs::write(output, generated.source).expect("write generated data bindings");
    let ecmascript = output_directory.join("codes.mjs");
    fs::write(&ecmascript, generate_ecmascript_codes(&checked.codes))
        .expect("write generated ECMAScript code bindings");
    println!(
        "cargo:rustc-env=CONDUIT_DATA_CODES_MJS={}",
        ecmascript.display()
    );
}
