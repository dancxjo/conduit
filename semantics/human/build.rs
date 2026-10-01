use conduit_form::rust_binding::{generate_rust_bindings_with_codes, RustBindingOptions};
use conduit_form::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{collections::BTreeMap, env, fs, path::PathBuf};

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
    .expect("human semantic Types must check");
    let generated = generate_rust_bindings_with_codes(
        &checked.native_types,
        &checked.codes,
        &RustBindingOptions {
            derive_serde_for_variants: true,
            copy_record_types: ["ExperienceTemporalPolicy".into(), "ImageRegion".into()].into(),
            direct_checked_record_constructors: ["ImageObservationReference".into()].into(),
            public_record_fields: ["ImageObservationReference".into()].into(),
            record_constructor_orders: BTreeMap::from([(
                "ImageObservationReference".into(),
                vec!["content".into(), "width".into(), "height".into()],
            )]),
            record_constructor_names: BTreeMap::from([(
                "ImageObservationReference".into(),
                "new_native".into(),
            )]),
            ..RustBindingOptions::default()
        },
    )
    .expect("human semantic Types and codes must generate exact Rust bindings");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("semantic_types.rs");
    fs::write(output, generated.source).expect("write generated human bindings");
}
