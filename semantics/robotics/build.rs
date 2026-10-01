use conduit_form::rust_binding::{generate_rust_bindings_with_codes, RustBindingOptions};
use conduit_form::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("types.conduit")),
        &StartupCatalog::new(),
    )
    .expect("robotics semantic Types must check");
    let generated = generate_rust_bindings_with_codes(
        &checked.native_types,
        &checked.codes,
        &RustBindingOptions {
            copy_record_types: [
                "AccelerationObservation".into(),
                "BatteryObservation".into(),
                "ButtonSetObservation".into(),
                "ContactObservation".into(),
                "OdometryObservation".into(),
                "OrientationObservation".into(),
                "ProximityObservation".into(),
                "RangeObservation".into(),
                "WheelDropObservation".into(),
            ]
            .into(),
            copy_record_value_getters: [
                "BatteryObservation".into(),
                "OdometryObservation".into(),
                "OrientationObservation".into(),
                "RangeObservation".into(),
            ]
            .into(),
            record_constructor_orders: [
                (
                    "OrientationObservation".into(),
                    vec![
                        "roll-microradians".into(),
                        "pitch-microradians".into(),
                        "yaw-microradians".into(),
                    ],
                ),
                (
                    "RangeObservation".into(),
                    vec!["distance-mm".into(), "age-ms".into()],
                ),
            ]
            .into(),
            ..RustBindingOptions::default()
        },
    )
    .expect("robotics semantic Types and codes must generate exact Rust bindings");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("semantic_types.rs");
    fs::write(output, generated.source).expect("write generated robotics bindings");
}
