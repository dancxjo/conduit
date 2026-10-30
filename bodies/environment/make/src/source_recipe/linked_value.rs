//! Make-only expressions for exact checked startup values. Target execution
//! reconstructs the original value without a configuration enum JSON decoder.
use conduit_core as c;
use std::fmt;

pub(super) fn emit(value: &c::ConfigurationValue) -> Result<String, fmt::Error> {
    use c::ConfigurationValue as V;
    Ok(match value {
        V::Bool(value) => format!("c::ConfigurationValue::Bool({value:?})"),
        V::U64(value) => format!("c::ConfigurationValue::U64({value}u64)"),
        V::I64(value) => format!("c::ConfigurationValue::I64({value}i64)"),
        V::Text(value) => format!(
            "c::ConfigurationValue::Text(c::ConfigurationText::from_static({:?}))",
            value.as_str()
        ),
        V::Structured(value) => format!(
            "c::ConfigurationValue::Structured(c::StructuredConfigurationValue::from_static({:?}.into(), &{:?}).expect(\"Make-checked structured configuration\"))",
            value.profile().as_str(), value.canonical_value()
        ),
        V::Quantity(value) => format!(
            "c::ConfigurationValue::Quantity(c::QuantityConfigurationValue::new(c::Quantity::decode(&{:?}).expect(\"Make-checked quantity encoding\"), {:?}.into()).expect(\"Make-checked quantity evidence\"))",
            value.canonical_value(), value.source()
        ),
        V::Unit(value) => format!(
            "c::ConfigurationValue::Unit(c::UnitConfigurationValue::new(c::Unit::decode(&{:?}).expect(\"Make-checked unit encoding\"), {:?}.into()).expect(\"Make-checked unit evidence\"))",
            value.canonical_value(), value.source()
        ),
        V::TemperatureDifference(value) => format!(
            "c::ConfigurationValue::TemperatureDifference(c::ExactTemperatureDifferenceConfigurationValue::new(c::ExactTemperatureDifference::from_quantity(c::Quantity::decode(&{:?}).expect(\"Make-checked difference encoding\")).expect(\"Make-checked difference dimension\"), {:?}.into()).expect(\"Make-checked difference evidence\"))",
            value.canonical_value(), value.source()
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_checked_value_compiles_and_preserves_exact_canonical_bytes() {
        let leaf = c::StructuredInfoType::leaf(c::kind_id(c::COUNT_INFO_ID)).unwrap();
        let structured =
            c::StructuredInfoValue::leaf(leaf.clone(), c::encode_count(7).to_vec()).unwrap();
        let values = vec![
            c::ConfigurationValue::Bool(true),
            c::ConfigurationValue::U64(u64::MAX),
            c::ConfigurationValue::I64(i64::MIN),
            c::ConfigurationValue::Text("quoted \"value\"\nUnicode °F\\end".into()),
            c::ConfigurationValue::Structured(
                c::StructuredConfigurationValue::new(
                    leaf.profile().unwrap().value_kind().clone(),
                    structured.canonical_bytes().unwrap(),
                )
                .unwrap(),
            ),
            c::ConfigurationValue::Quantity(c::QuantityConfigurationValue::parse("1kHz").unwrap()),
            c::ConfigurationValue::Unit(c::UnitConfigurationValue::parse("um").unwrap()),
            c::ConfigurationValue::TemperatureDifference(
                c::ExactTemperatureDifferenceConfigurationValue::parse("TemperatureDelta(21, °C)")
                    .unwrap(),
            ),
        ];
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let root = manifest.join("../../..").canonicalize().unwrap();
        let directory = manifest.join("target/linked-value-conformance");
        std::fs::create_dir_all(directory.join("src")).unwrap();
        std::fs::write(directory.join("Cargo.toml"), format!(
            "[package]\nname=\"linked-value-conformance\"\nversion=\"0.0.0\"\nedition=\"2021\"\n[workspace]\n[dependencies]\nconduit-core={{path={:?}}}\nserde_json=\"1\"\n", root.join("architecture/core")
        )).unwrap();
        let expressions = values
            .iter()
            .map(|value| emit(value).unwrap())
            .collect::<Vec<_>>()
            .join(",\n");
        let expected = serde_json::to_vec(&values).unwrap();
        std::fs::write(directory.join("src/main.rs"), format!(
            "extern crate alloc; use conduit_core as c; fn main() {{ let actual: alloc::vec::Vec<c::ConfigurationValue> = alloc::vec![{expressions}]; for value in &actual {{ if let c::ConfigurationValue::Structured(value) = value {{ assert_eq!(value.canonical_value().as_ptr(), value.clone().canonical_value().as_ptr()); }} }} assert_eq!(serde_json::to_vec(&actual).unwrap(), {:?}); }}", expected
        )).unwrap();
        let output = std::process::Command::new("cargo")
            .args(["+stable", "run", "--offline", "--quiet", "--manifest-path"])
            .arg(directory.join("Cargo.toml"))
            .env(
                "CARGO_TARGET_DIR",
                std::env::var_os("CARGO_TARGET_DIR")
                    .map(|path| std::path::PathBuf::from(path).join("linked-value-conformance"))
                    .unwrap_or_else(|| root.join("target/linked-value-conformance")),
            )
            .env("CARGO_INCREMENTAL", "0")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "generated value compile/parity: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
