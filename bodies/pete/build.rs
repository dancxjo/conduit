use conduit_plot::rust_binding::{
    generate_rust_bindings_with_forms_and_external_bindings,
    semantic_core::StructuredInfoTypeShape, ExternalNativeRustBinding, RustBindingOptions,
};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=cognition-types.conduit");
    let mut catalog = StartupCatalog::new();
    let types = [
        conduit_time::NativeTemporalInstant::semantic_type().unwrap(),
        conduit_human::ExperienceTemporalPolicy::semantic_type().unwrap(),
    ];
    let names = ["TemporalInstant", "ExperienceTemporalPolicy"];
    let rust_types = [
        "conduit_time::NativeTemporalInstant",
        "conduit_human::ExperienceTemporalPolicy",
    ];
    let bindings = types
        .iter()
        .zip(names)
        .zip(rust_types)
        .map(|((ty, name), path)| {
            let StructuredInfoTypeShape::Record { schema, .. } = ty.shape() else {
                panic!("Pete cognition imports named records");
            };
            catalog.insert_structured_type(name, ty.clone()).unwrap();
            ExternalNativeRustBinding {
                semantic_identity: schema.as_str(),
                rust_type_path: path,
            }
        })
        .collect::<Vec<_>>();
    let document = parse_syntax_document(include_str!("cognition-types.conduit"));
    let checked = check_syntax_document(&document, &catalog).unwrap();
    let generated = generate_rust_bindings_with_forms_and_external_bindings(
        &checked.native_types,
        &checked.type_forms,
        &types,
        &bindings,
        &RustBindingOptions::default(),
    )
    .unwrap();
    let path = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("cognition_types.rs");
    fs::write(path, generated.source).unwrap();
}
