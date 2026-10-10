//! Execute generated consumers of closed value-parameterized Types.
use conduit_plot::rust_binding::{generate_rust_bindings, RustBindingOptions};
use std::{ffi::OsStr, fs, path::Path, process::Command};

#[test]
fn generated_value_parameters_enforce_construction_decode_and_nested_laws() {
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(include_str!(
            "fixtures/value_parameter_bindings.conduit"
        )),
        &conduit_plot::StartupCatalog::new(),
    )
    .unwrap();
    let generated =
        generate_rust_bindings(&checked.native_types, &RustBindingOptions::default()).unwrap();
    let dependencies = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let library = fs::read_dir(&dependencies)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension() == Some(OsStr::new("rlib"))
                && path
                    .file_name()
                    .and_then(OsStr::to_str)
                    .is_some_and(|name| name.starts_with("libconduit_plot-"))
        })
        .max_by_key(|path| {
            path.metadata()
                .and_then(|metadata| metadata.modified())
                .ok()
        })
        .expect("compiled Plot library");
    let directory =
        std::env::temp_dir().join(format!("conduit-value-binding-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let source = directory.join("bindings.rs");
    fs::write(&source, format!("{}{}", generated.source, EXERCISE)).unwrap();
    let executable = directory.join("bindings-test");
    let mut compiler = rustc(&dependencies, &library);
    let output = compiler
        .args(["--test"])
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "generated consumer compile:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(&executable).output().unwrap();
    assert!(
        output.status.success(),
        "generated consumer execution:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let result = String::from_utf8_lossy(&output.stdout);
    assert!(result.contains("3 passed; 0 failed"), "{result}");
    println!("{result}");
    let no_std_source = directory.join("bindings_no_std.rs");
    fs::write(&no_std_source, format!("#![no_std]\n{}", generated.source)).unwrap();
    let output = rustc(&dependencies, &library)
        .arg("--crate-type=lib")
        .arg(&no_std_source)
        .arg("-o")
        .arg(directory.join("libbindings_no_std.rlib"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "generated no_std consumer compile:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(directory).unwrap();
}

fn rustc(dependencies: &Path, library: &Path) -> Command {
    let mut compiler = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
    compiler
        .arg("--edition=2021")
        .arg("-L")
        .arg(format!("dependency={}", dependencies.display()))
        .arg("--extern")
        .arg(format!("conduit_plot={}", library.display()));
    compiler
}

const EXERCISE: &str = r#"
#[cfg(test)]
mod acceptance {
    use super::*;
    use conduit_core::StructuredInfoTypeShape;

    fn raw_matrix(columns: u16, rows: u16) -> StructuredInfoValue {
        let semantic = Matrix192x128::semantic_type().unwrap();
        let StructuredInfoTypeShape::Record { fields, .. } = semantic.shape() else { panic!() };
        let values = fields.iter().map(|field| {
            let integer = match field.name() { "columns" => columns, "rows" => rows, _ => panic!() };
            StructuredFieldValue::new(field.name(),
                StructuredInfoValue::leaf(field.value_type().clone(), integer.to_le_bytes().to_vec()).unwrap()
            ).unwrap()
        }).collect();
        StructuredInfoValue::record(semantic, values).unwrap()
    }

    #[test]
    fn matrix_dimensions_validate_on_construction_and_decode() {
        let matrix = Matrix192x128::new(192, 128).unwrap();
        let bytes = matrix.clone().encode().unwrap();
        assert_eq!(Matrix192x128::decode(&bytes).unwrap(), matrix);
        for (columns, rows) in [(193, 128), (192, 127)] {
            assert!(matches!(Matrix192x128::new(columns, rows), Err(NativeBindingRefusal::ViolatedInvariant { .. })));
            let encoded = raw_matrix(columns, rows).canonical_bytes().unwrap();
            assert!(matches!(Matrix192x128::decode(&encoded), Err(NativeBindingRefusal::ViolatedInvariant { .. })));
        }
        let foreign = Matrix193x128::new(193, 128).unwrap().encode().unwrap();
        assert!(Matrix192x128::decode(&foreign).is_err());
        let same_layout = ForeignMatrix::new(192, 128).unwrap().encode().unwrap();
        assert_eq!(core::mem::size_of::<ForeignMatrix>(), core::mem::size_of::<Matrix192x128>());
        assert!(Matrix192x128::decode(&same_layout).is_err());
        assert!(Matrix192x128::decode(&bytes[..bytes.len() - 1]).is_err());
    }

    #[test]
    fn nested_record_decode_cannot_bypass_dimension_laws() {
        let valid = Envelope::new(Matrix192x128::new(192, 128).unwrap()).unwrap();
        assert_eq!(Envelope::decode(&valid.clone().encode().unwrap()).unwrap(), valid);
        let semantic = Envelope::semantic_type().unwrap();
        let payload = StructuredInfoValue::record(semantic, vec![
            StructuredFieldValue::new("matrix", raw_matrix(193, 128)).unwrap()
        ]).unwrap().canonical_bytes().unwrap();
        assert!(matches!(Envelope::decode(&payload), Err(NativeBindingRefusal::ViolatedInvariant { .. })));
    }

    fn zero(semantic: StructuredInfoType) -> StructuredInfoValue {
        match semantic.shape() {
            StructuredInfoTypeShape::Nominal { representation, .. } => {
                StructuredInfoValue::nominal(semantic.clone(), zero(representation.clone())).unwrap()
            }
            StructuredInfoTypeShape::Collection { element, length } => {
                let values = (0..length).map(|_| zero(element.clone())).collect();
                StructuredInfoValue::collection(semantic, values).unwrap()
            }
            StructuredInfoTypeShape::Record { fields, .. } => {
                let values = fields.iter().map(|field| {
                    StructuredFieldValue::new(field.name(), zero(field.value_type().clone())).unwrap()
                }).collect();
                StructuredInfoValue::record(semantic, values).unwrap()
            }
            StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == "value/u8" => {
                StructuredInfoValue::leaf(semantic, vec![0]).unwrap()
            }
            _ => panic!("unexpected Window representation"),
        }
    }

    fn extent(semantic: &StructuredInfoType) -> u16 {
        match semantic.shape() {
            StructuredInfoTypeShape::Nominal { representation, .. } => extent(representation),
            StructuredInfoTypeShape::Collection { length, .. } => length,
            _ => panic!("expected exact collection"),
        }
    }

    #[test]
    fn computed_window_and_history_remain_exact_in_generated_bindings() {
        let semantic = Window2x3::semantic_type().unwrap();
        let StructuredInfoTypeShape::Record { fields, .. } = semantic.shape() else { panic!() };
        assert_eq!(extent(fields.iter().find(|field| field.name() == "window").unwrap().value_type()), 9);
        assert_eq!(extent(fields.iter().find(|field| field.name() == "history").unwrap().value_type()), 2);
        let raw = zero(semantic);
        let decoded = Window2x3::decode(&raw.canonical_bytes().unwrap()).unwrap();
        let constructed = Window2x3::new(decoded.history().clone(), decoded.window().clone()).unwrap();
        assert_eq!(constructed, decoded);
        let foreign = zero(Window3x2::semantic_type().unwrap()).canonical_bytes().unwrap();
        assert!(Window2x3::decode(&foreign).is_err());
    }
}
"#;
