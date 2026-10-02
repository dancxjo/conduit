use conduit_plot::rust_binding::{generate_rust_bindings_with_forms, RustBindingOptions};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("types.conduit")),
        &StartupCatalog::new(),
    )
    .expect("artificial-life semantic Types must check");
    let generated = generate_rust_bindings_with_forms(
        &checked.native_types,
        &checked.type_forms,
        &RustBindingOptions {
            copy_record_types: [
                "GardenClockObservation".into(),
                "GardenContactObservation".into(),
                "GardenEnrichedObservation".into(),
                "GardenState".into(),
                "GrayScottParameters".into(),
                "LeniaParameters".into(),
                "ReactionDiffusionCell".into(),
                "ReactionDiffusionEvolveRequest".into(),
                "ReactionDiffusionRegion".into(),
            ]
            .into(),
            copy_record_value_getters: [
                "GrayScottParameters".into(),
                "LeniaParameters".into(),
                "ReactionDiffusionCell".into(),
                "ReactionDiffusionEvolveRequest".into(),
                "ReactionDiffusionRegion".into(),
            ]
            .into(),
            direct_checked_record_constructors: ["LeniaParameters".into()].into(),
            public_record_fields: [
                "GardenClockObservation".into(),
                "GardenContactObservation".into(),
                "GardenEnrichedObservation".into(),
                "GardenState".into(),
                "GrayScottParameters".into(),
                "LeniaParameters".into(),
                "ReactionDiffusionCell".into(),
                "ReactionDiffusionEvolveRequest".into(),
            ]
            .into(),
            copy_nominal_types: ["LeniaFieldId".into(), "ReactionDiffusionFieldId".into()].into(),
            hash_nominal_types: ["LeniaFieldId".into(), "ReactionDiffusionFieldId".into()].into(),
            record_constructor_orders: [
                (
                    "ReactionDiffusionEvolveRequest".into(),
                    vec![
                        "field-id".into(),
                        "expected-generation".into(),
                        "generations".into(),
                        "admitted-cell-generations".into(),
                    ],
                ),
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
            record_constructor_names: [("ReactionDiffusionCell".into(), "new_native".into())]
                .into(),
            ..RustBindingOptions::default()
        },
    )
    .expect("artificial-life semantic Types and Forms must generate exact Rust bindings");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("semantic_types.rs");
    fs::write(output, generated.source).expect("write generated artificial-life bindings");
}
