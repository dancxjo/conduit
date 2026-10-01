use conduit_form::rust_binding::{generate_rust_bindings_with_codes, RustBindingOptions};
use conduit_form::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("types.conduit")),
        &StartupCatalog::new(),
    )
    .expect("time semantic Types must check");
    let generated = generate_rust_bindings_with_codes(
        &checked.native_types,
        &checked.codes,
        &RustBindingOptions {
            derive_serde_for_variants: true,
            serde_variant_exclusions: ["ReplayCommand".into()].into(),
            serde_record_types: [
                "CandidateConflict".into(),
                "ReminderOccurrence".into(),
                "TemporalInstant".into(),
            ]
            .into(),
            copy_record_types: [
                "CivilResolutionPolicy".into(),
                "PulseObservation".into(),
                "RhythmState".into(),
            ]
            .into(),
            public_record_fields: ["ReminderOccurrence".into(), "RhythmState".into()].into(),
            ..RustBindingOptions::default()
        },
    )
    .expect("time semantic Types and codes must generate exact Rust bindings");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("semantic_types.rs");
    fs::write(output, generated.source).expect("write generated time bindings");
}
