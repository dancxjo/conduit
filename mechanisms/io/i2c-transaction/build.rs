//! Check the authored protocol schemas on the fabrication host, not on the MCU.
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    let source = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../../../plots/device-protocols/i2c-types.conduit");
    println!("cargo:rerun-if-changed={}", source.display());
    let syntax = parse_syntax_document(&fs::read_to_string(source).expect("I2C Type Source"));
    assert!(syntax.diagnostics.is_empty(), "invalid I2C Type Source");
    let checked =
        check_syntax_document(&syntax, &StartupCatalog::new()).expect("checked I2C Type Source");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    for (name, file) in [
        ("I2cTransaction", "request.type"),
        ("I2cResult", "result.type"),
    ] {
        let ty = checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .expect("named I2C Type");
        fs::write(
            output.join(file),
            ty.value_type.canonical_bytes().expect("canonical I2C Type"),
        )
        .expect("write checked I2C schema");
    }
    prepare_result_storage(
        &checked
            .native_types
            .iter()
            .find(|t| t.name == "I2cResult")
            .unwrap()
            .value_type,
        &output,
    );
}

/// Derive storage from the checked finite wire schema, not the transport ceiling.
fn prepare_result_storage(result: &conduit_core::StructuredInfoType, output: &std::path::Path) {
    use conduit_core::{
        StructuredFieldValue, StructuredInfoTypeShape as Shape, StructuredInfoValue as Value,
    };
    let Shape::Variant { cases, .. } = result.shape() else {
        panic!("I2C result variant")
    };
    let mut maximum = 0;
    for case in cases {
        let ty = case.payload_type();
        let payload = match ty.shape() {
            Shape::Record { fields, .. } => {
                let [field] = fields else {
                    panic!("I2C result payload")
                };
                assert_eq!(field.name(), "input");
                let Shape::Sequence {
                    element,
                    minimum_items,
                    maximum_items,
                } = field.value_type().shape()
                else {
                    panic!("I2C input sequence")
                };
                assert_eq!((minimum_items, maximum_items), (0, 32));
                let octets = Value::sequence(
                    field.value_type().clone(),
                    (0..maximum_items)
                        .map(|_| Value::leaf(element.clone(), vec![255]).unwrap())
                        .collect(),
                )
                .unwrap();
                let value = Value::record(
                    ty.clone(),
                    vec![StructuredFieldValue::new("input", octets).unwrap()],
                )
                .unwrap();
                fs::write(
                    output.join(format!("{}.type", case.tag())),
                    ty.canonical_bytes().unwrap(),
                )
                .unwrap();
                fs::write(
                    output.join(format!("{}.bound", case.tag())),
                    value.canonical_bytes().unwrap().len().to_string(),
                )
                .unwrap();
                value
            }
            Shape::Leaf(kind) if kind.as_str() == "value/empty" => {
                Value::leaf(ty.clone(), vec![]).unwrap()
            }
            _ => panic!("unsupported I2C result shape"),
        };
        maximum = maximum.max(
            Value::variant(result.clone(), case.tag(), payload)
                .unwrap()
                .canonical_bytes()
                .unwrap()
                .len(),
        );
    }
    fs::write(output.join("result.bound"), maximum.to_string()).unwrap();
}
