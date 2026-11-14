//! Wide native quantity codecs preserve Type meaning; these are not Source Gears.
use conduit_core::{Quantity, QuantityFormInteger, QuantityFormRefusal};
use conduit_plot::*;
use sha2::{Digest, Sha256};
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

const SOURCE: &str = "form native/wide-temp = Temperature as i128le(°C, -2)\nform native/wide-ratio = Ratio as i128le(%, -10)\n";

fn forms(source: &str) -> Vec<CheckedTypeForm> {
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    check_syntax_document(&syntax, &StartupCatalog::new())
        .unwrap()
        .type_forms
}

#[test]
fn wide_extent_and_identity_are_distinct_and_existing_identities_are_unchanged() {
    let source = "form native/value = Temperature as i16le(°C, -2)\n";
    let signed = forms(source).remove(0);
    let unsigned = forms(&source.replace("i16le", "u16le")).remove(0);
    let wide = forms(&source.replace("i16le", "i128le")).remove(0);
    for existing in [&signed, &unsigned] {
        let CheckedTypeFormStorage::DecimalQuantity(repr) = &existing.storage else {
            panic!("quantity Form")
        };
        // The original v1 identity used precisely this signed/unsigned byte.
        let mut original = Sha256::new();
        original.update(b"conduit.form.decimal-quantity/v1\0");
        for part in [
            existing.name.as_bytes(),
            existing.value_type.as_str().as_bytes(),
            &repr.unit.encode(),
            &repr.exponent.to_le_bytes(),
            &[u8::from(repr.integer == QuantityFormInteger::U16Le)],
        ] {
            original.update((part.len() as u64).to_le_bytes());
            original.update(part);
        }
        assert_eq!(
            existing.compatibility_id,
            format!("form/sha256:{:x}", original.finalize())
        );
        assert_eq!((existing.exact_bytes, existing.maximum_bytes), (2, 2));
    }
    assert_eq!((wide.exact_bytes, wide.maximum_bytes), (16, 16));
    assert_eq!(wide.value_type, signed.value_type);
    assert_ne!(wide.compatibility_id, signed.compatibility_id);
    assert_ne!(wide.compatibility_id, unsigned.compatibility_id);
    let CheckedTypeFormStorage::DecimalQuantity(repr) = wide.storage else {
        panic!("quantity Form")
    };
    assert_eq!(repr.integer, QuantityFormInteger::I128Le);
    assert_eq!(
        repr.decode(&(-1234_i128).to_le_bytes()),
        Ok(Quantity::parse_plot_literal("-12.34°C").unwrap())
    );
}

#[test]
fn wide_forms_are_allocation_free_and_refuse_invalid_storage_family_or_extent() {
    let CheckedTypeFormStorage::DecimalQuantity(codec) = forms(SOURCE).remove(0).storage else {
        panic!("quantity Form")
    };
    let (_, observation) = allocation_probe::observe(|| {
        for coefficient in [-4000_i128, 0, 2508, 8500] {
            let value = codec.decode(&coefficient.to_le_bytes()).unwrap();
            let mut encoded = [0; 16];
            assert_eq!(codec.encode_into(value, &mut encoded), Ok(16));
            assert_eq!(encoded, coefficient.to_le_bytes());
        }
        assert_eq!(
            codec.decode(&[0; 15]),
            Err(QuantityFormRefusal::WrongLength)
        );
        assert_eq!(
            codec.decode(&[0; 17]),
            Err(QuantityFormRefusal::WrongLength)
        );
    });
    assert_eq!(observation.allocations, 0);
    assert_eq!(observation.reallocations, 0);
    for bad in [
        "form bad = Temperature as i128le(%, -2)\n",
        "form bad = Temperature as i128le(°C, 129)\n",
        "form bad = Temperature as u128le(°C, -2)\n",
    ] {
        let syntax = parse_syntax_document(bad);
        assert!(
            !syntax.diagnostics.is_empty()
                || check_syntax_document(&syntax, &StartupCatalog::new()).is_err()
        );
    }
}

#[test]
fn generated_i128_forms_execute_and_compile_as_no_std() {
    use std::{fs, process::Command};
    let checked =
        check_syntax_document(&parse_syntax_document(SOURCE), &StartupCatalog::new()).unwrap();
    let generated = rust_binding::generate_rust_bindings_with_forms(
        &checked.native_types,
        &checked.type_forms,
        &Default::default(),
    )
    .unwrap();
    let directory = std::env::temp_dir().join(format!(
        "conduit-i128-quantity-forms-{}",
        std::process::id()
    ));
    fs::create_dir(&directory).unwrap();
    let dependencies = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let library = fs::read_dir(&dependencies)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("libconduit_plot-")
                && entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "rlib")
        })
        .max_by_key(|entry| entry.metadata().unwrap().modified().unwrap())
        .unwrap()
        .path();
    let exercise = r#"
fn main() {
    let wire = (-1234_i128).to_le_bytes();
    let q = NativeWideTempForm::decode(&wire).unwrap();
    assert_eq!(NativeWideTempForm::EXACT_BYTES, 16);
    let encoded: [u8; 16] = NativeWideTempForm::encode(q).unwrap();
    assert_eq!(encoded, wire);
    assert_eq!(q, conduit_core::Quantity::parse_plot_literal("-12.34°C").unwrap());
    let q = NativeWideRatioForm::decode(&(51200_i128 * 9765625).to_le_bytes()).unwrap();
    assert_eq!(q, conduit_core::Quantity::parse_plot_literal("50%").unwrap());
    assert!(NativeWideTempForm::decode(&[0; 2]).is_err());
}
"#;
    let file = directory.join("forms.rs");
    fs::write(&file, format!("{}{exercise}", generated.source)).unwrap();
    let binary = directory.join("forms");
    let compile = |input: &std::path::Path, output: &std::path::Path, library_kind: bool| {
        let mut command = Command::new("rustc");
        command
            .args(["--edition=2021", "-L"])
            .arg(format!("dependency={}", dependencies.display()))
            .arg("--extern")
            .arg(format!("conduit_plot={}", library.display()));
        if library_kind {
            command.args(["--crate-type=lib", "--crate-name=wide_quantity_forms"]);
        }
        let result = command.arg(input).arg("-o").arg(output).output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    };
    compile(&file, &binary, false);
    assert!(Command::new(&binary).status().unwrap().success());
    let no_std = directory.join("no_std.rs");
    fs::write(&no_std, format!("#![no_std]\n{}", generated.source)).unwrap();
    compile(&no_std, &directory.join("libforms.rlib"), true);
    fs::remove_dir_all(directory).unwrap();
}
