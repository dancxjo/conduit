//! A quantity's meaning survives selection of another finite representation.
use conduit_core::{Quantity, QuantityFormRefusal as R, Unit};
use conduit_plot::{
    check_syntax_document, parse_syntax_document, CheckedTypeFormStorage, StartupCatalog,
};
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn form(source: &str) -> conduit_plot::CheckedTypeForm {
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    check_syntax_document(&syntax, &StartupCatalog::new())
        .unwrap()
        .type_forms
        .remove(0)
}

fn repr(source: &str) -> conduit_core::DecimalQuantityForm {
    let CheckedTypeFormStorage::DecimalQuantity(form) = form(source).storage else {
        panic!("quantity form")
    };
    *form
}

#[test]
fn another_representation_changes_form_identity_without_changing_quantity_type() {
    let a = form("form sensor/temperature = Temperature as i16le(°C, -1)\n");
    let b = form("form sensor/temperature = Temperature as i16le(°C, -2)\n");
    let c = form("form sensor/temperature = Temperature as i16le(°F, -1)\n");
    assert_eq!(a.value_type, b.value_type);
    assert_eq!(a.value_type, c.value_type);
    assert_ne!(a.compatibility_id, b.compatibility_id);
    assert_ne!(a.compatibility_id, c.compatibility_id);
    assert_eq!((a.exact_bytes, a.maximum_bytes), (2, 2));
    let CheckedTypeFormStorage::DecimalQuantity(codec) = a.storage else {
        unreachable!()
    };
    let q = codec.decode(&198_i16.to_le_bytes()).unwrap();
    assert_eq!(q, Quantity::parse_plot_literal("19.8°C").unwrap());
    assert_eq!(codec.encode(q).unwrap(), 198_i16.to_le_bytes());
    assert_eq!(q.role(), conduit_core::QuantityRole::Point);
}

#[test]
fn exact_conversion_negative_values_and_ratio_scale_use_existing_quantity_laws() {
    let temperature = repr("form sensor/temperature = Temperature as i16le(°C, -1)\n");
    let q = Quantity::parse_plot_literal("68°F").unwrap();
    assert_eq!(temperature.encode(q).unwrap(), 200_i16.to_le_bytes());
    assert_eq!(
        temperature.decode(&(-108_i16).to_le_bytes()).unwrap(),
        Quantity::parse_plot_literal("-10.8°C").unwrap()
    );
    let humidity = repr("form sensor/humidity = Ratio as u16le(%, -1)\n");
    let q = humidity.decode(&590_u16.to_le_bytes()).unwrap();
    assert_eq!(q, Quantity::parse_plot_literal("59%").unwrap());
    assert_eq!(
        q.convert_to_decimal(Unit::One).unwrap(),
        Quantity::parse_plot_literal("0.59one").unwrap()
    );
}

#[test]
fn forms_refuse_fraction_loss_overflow_wrong_family_role_and_wrong_length() {
    let temperature = repr("form sensor/temperature = Temperature as i16le(°C, -1)\n");
    assert_eq!(
        temperature.encode(Quantity::parse_plot_literal("19.85°C").unwrap()),
        Err(R::Inexact)
    );
    assert_eq!(
        temperature.encode(Quantity::parse_plot_literal("4000°C").unwrap()),
        Err(R::Overflow)
    );
    assert_eq!(
        temperature.encode(Quantity::parse_plot_literal("1m").unwrap()),
        Err(R::WrongFamily)
    );
    assert_eq!(
        temperature.encode(
            Quantity::from_decimal_role(20, 0, Unit::Celsius, conduit_core::QuantityRole::Delta)
                .unwrap()
        ),
        Err(R::WrongRole)
    );
    assert_eq!(temperature.decode(&[0]), Err(R::WrongLength));
    let humidity = repr("form sensor/humidity = Ratio as u16le(%, -1)\n");
    assert_eq!(
        humidity.encode(Quantity::parse_plot_literal("-1%").unwrap()),
        Err(R::Overflow)
    );
}

#[test]
fn incompatible_definitions_fail_during_checking() {
    for source in [
        "form bad = Temperature as i16le(m, -1)\n",
        "form bad = Ratio as u16le(°C, -1)\n",
        "form bad = Temperature as i16le(unknown, -1)\n",
        "form bad = Temperature as i16le(°C, 30000)\n",
        "form bad = U8 as i16le(°C, -1)\n",
    ] {
        let syntax = parse_syntax_document(source);
        assert!(
            check_syntax_document(&syntax, &StartupCatalog::new()).is_err(),
            "{source}"
        );
    }
}

#[test]
fn selected_quantity_forms_encode_and_decode_without_allocating() {
    let codec = repr("form sensor/temperature = Temperature as i16le(°C, -1)\n");
    let ((), observation) = allocation_probe::observe(|| {
        for coefficient in -200_i16..=600 {
            let bytes = coefficient.to_le_bytes();
            let value = codec.decode(&bytes).unwrap();
            assert_eq!(codec.encode(value).unwrap(), bytes);
        }
    });
    assert_eq!(observation.allocations, 0);
    assert_eq!(observation.reallocations, 0);
}

#[test]
fn generated_rust_quantity_forms_compile_and_preserve_native_meaning() {
    use std::{fs, process::Command};
    let source = "form sensor/temperature = Temperature as i16le(°C, -1)\nform sensor/humidity = Ratio as u16le(%, -1)\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let generated = conduit_plot::rust_binding::generate_rust_bindings_with_forms(
        &checked.native_types,
        &checked.type_forms,
        &Default::default(),
    )
    .unwrap();
    let directory =
        std::env::temp_dir().join(format!("conduit-quantity-forms-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let dependencies = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let library = fs::read_dir(&dependencies)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("libconduit_plot-")
                && e.path().extension().is_some_and(|x| x == "rlib")
        })
        .max_by_key(|e| e.metadata().unwrap().modified().unwrap())
        .unwrap()
        .path();
    let file = directory.join("forms.rs");
    let exercise = r#"
fn main() {
    let q = SensorTemperatureForm::decode(&198_i16.to_le_bytes()).unwrap();
    assert_eq!(q, conduit_core::Quantity::parse_plot_literal("19.8°C").unwrap());
    assert_eq!(SensorTemperatureForm::encode(q).unwrap(), 198_i16.to_le_bytes());
    let q = SensorHumidityForm::decode(&590_u16.to_le_bytes()).unwrap();
    assert_eq!(q, conduit_core::Quantity::parse_plot_literal("59%").unwrap());
}
"#;
    fs::write(&file, format!("{}{exercise}", generated.source)).unwrap();
    let executable = directory.join("forms");
    let compiled = Command::new("rustc")
        .args(["--edition=2021", "-L"])
        .arg(format!("dependency={}", dependencies.display()))
        .arg("--extern")
        .arg(format!("conduit_plot={}", library.display()))
        .arg(&file)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    assert!(Command::new(&executable).status().unwrap().success());
    fs::remove_dir_all(directory).unwrap();
}
