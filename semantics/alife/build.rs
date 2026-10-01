use conduit_form::rust_binding::{generate_rust_bindings_with_codes, RustBindingOptions};
use conduit_form::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("types.conduit")),
        &StartupCatalog::new(),
    )
    .expect("artificial-life semantic Types must check");
    let generated = generate_rust_bindings_with_codes(
        &checked.native_types,
        &checked.codes,
        &RustBindingOptions {
            copy_record_types: [
                "GrayScottParameters".into(),
                "LeniaParameters".into(),
                "ReactionDiffusionRegion".into(),
            ]
            .into(),
            copy_record_value_getters: [
                "GrayScottParameters".into(),
                "LeniaParameters".into(),
                "ReactionDiffusionRegion".into(),
            ]
            .into(),
            direct_checked_record_constructors: ["LeniaParameters".into()].into(),
            public_record_fields: ["GrayScottParameters".into(), "LeniaParameters".into()].into(),
            record_constructor_orders: [
                (
                    "ReactionDiffusionRegion".into(),
                    vec![
                        "region-id".into(),
                        "origin-x".into(),
                        "origin-y".into(),
                        "width".into(),
                        "height".into(),
                    ],
                ),
                (
                    "LeniaParameters".into(),
                    vec![
                        "kernel-radius".into(),
                        "kernel-mu-q16".into(),
                        "kernel-sigma-q16".into(),
                        "growth-mu-q16".into(),
                        "growth-sigma-q16".into(),
                        "dt-q16".into(),
                        "boundary".into(),
                    ],
                ),
                (
                    "GrayScottParameters".into(),
                    vec![
                        "diffusion-u-ppm".into(),
                        "diffusion-v-ppm".into(),
                        "feed-ppm".into(),
                        "kill-ppm".into(),
                        "time-step-ppm".into(),
                    ],
                ),
            ]
            .into(),
            ..RustBindingOptions::default()
        },
    )
    .expect("artificial-life semantic Types and codes must generate exact Rust bindings");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("semantic_types.rs");
    fs::write(output, generated.source).expect("write generated artificial-life bindings");
}
