//! Seal the exact browser quantity implementation identity without shipping its source.
use std::{env, fs, path::PathBuf};

fn main() {
    let base =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("src/installed_browser");
    let mut source = Vec::new();
    // Same source set and byte order as the former runtime concat! expression.
    for file in [
        "quantity_conversion.rs",
        "../../../../../architecture/plot/src/dimension_operation.rs",
        "../../../../../architecture/core/src/dimension.rs",
        "../../../../../semantics/catalog/src/converted_equals_back.rs",
        "../../../../../architecture/plot/src/quantity_conversion/converted_equals.rs",
        "../../../../../architecture/plot/src/quantity_conversion/converted_equals/validation.rs",
        "../../../../../architecture/plot/src/quantity_conversion.rs",
        "../../../../../architecture/plot/src/quantity_conversion/encoding.rs",
        "../../../../../architecture/plot/src/quantity_conversion/comparison.rs",
        "../../../../../architecture/plot/src/quantity_conversion/comparison/operand.rs",
        "../../../../../architecture/plot/src/quantity_conversion/profile.rs",
        "../../../../../architecture/plot/src/quantity_conversion/temperature_difference.rs",
        "../../../../../architecture/core/src/quantity/temperature_difference.rs",
        "../../../../../architecture/core/src/quantity.rs",
        "../../../../../architecture/core/src/unit.rs",
        "../../../../../architecture/core/definitions/physical.conduit",
        "../../../../../architecture/core/src/physical_definition.rs",
        "../../../../../architecture/core/src/physical_definition/dimension.rs",
        "../../../../../architecture/core/src/physical_definition/family.rs",
        "../../../../../architecture/core/src/physical_definition/generated.rs",
        "../../../../../architecture/core/src/physical_definition/role_kind.rs",
        "../../../../../architecture/core/src/physical_definition/scalar.rs",
        "../../../../../architecture/core/src/physical_definition/unit.rs",
        "../../../../../architecture/core/src/quantity_configuration.rs",
        "../../../../../architecture/core/src/quantity_suffix.rs",
        "../../../../../architecture/core/src/quantity/exact.rs",
        "../../../../../architecture/core/src/quantity/target.rs",
        "../../../../../architecture/core/src/quantity/receipt.rs",
        "../../../../../architecture/core/src/quantity/conversion_law.rs",
        "../../../../../architecture/core/src/quantity/wide_conversion.rs",
        "../../../../../architecture/core/src/quantity/magnitude.rs",
    ] {
        let path = base.join(file);
        println!("cargo:rerun-if-changed={}", path.display());
        source.extend(fs::read(path).expect("read quantity implementation source"));
    }
    let digest = conduit_core::semantic_digest("quantity/compiled-browser-back@1", &source);
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("quantity_compiled_digest.bin"),
        digest,
    )
    .expect("write quantity implementation identity");
}
