use std::{fs, path::PathBuf};
#[path = "reference.rs"]
mod reference;
pub fn emit() {
    let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let types_path = root.join("../../../plots/device-protocols/i2c-types.conduit");
    let frame_path = root.join("../../../plots/device-protocols/ht16k33.conduit");
    let check = |path: &std::path::Path, catalog: &conduit_plot::StartupCatalog| {
        println!("cargo:rerun-if-changed={}", path.display());
        let syntax = conduit_plot::parse_syntax_document(&fs::read_to_string(path).unwrap());
        assert!(syntax.diagnostics.is_empty());
        conduit_plot::check_syntax_document(&syntax, catalog).unwrap()
    };
    let mut catalog = conduit_plot::StartupCatalog::new();
    let types = check(&types_path, &catalog);
    for (name, path) in [
        ("I2cTransaction", "machine/i2c/transact/request"),
        ("I2cResult", "machine/i2c/transact/result"),
    ] {
        let ty = types
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap();
        catalog
            .insert_checked_native_type_from_document(path, &types, &ty.identity)
            .unwrap();
    }
    let checked = check(&frame_path, &catalog);
    let schema = &checked
        .native_types
        .iter()
        .find(|ty| ty.name == "Ht16k33Frame")
        .unwrap()
        .value_type;
    let template = reference::encode(schema, 0x70, [0; 16]);
    let mut offsets = [0; 17];
    for index in 0..17 {
        let mut ram = [0; 16];
        let address = if index == 0 {
            0x71
        } else {
            ram[index - 1] = 1;
            0x70
        };
        let changed = reference::encode(schema, address, ram);
        assert_eq!(template.len(), changed.len());
        let differences: Vec<_> = template
            .iter()
            .zip(&changed)
            .enumerate()
            .filter_map(|(n, (a, b))| (a != b).then_some(n))
            .collect();
        let [offset] = differences.as_slice() else {
            panic!("one scalar byte per field")
        };
        assert!(!offsets[..index].contains(offset));
        offsets[index] = *offset;
    }
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    fs::write(
        out.join("ht16k33-frame.type"),
        schema.canonical_bytes().unwrap(),
    )
    .unwrap();
    fs::write(
        out.join("ht16k33-frame-material.rs"),
        format!(
            "pub const TEMPLATE:[u8;{}]={:?};\npub const OFFSETS:[usize;17]={:?};\n",
            template.len(),
            template,
            offsets
        ),
    )
    .unwrap();
    // Source supplies the exact nominal acknowledgement Type. A write uses
    // count one; initialization uses the same Type with count five. Neither
    // value is a transport delivery acknowledgement.
    let acknowledgement = &checked
        .native_types
        .iter()
        .find(|ty| ty.name == "Ht16k33Acknowledgement")
        .unwrap()
        .value_type;
    use conduit_core::{PreparedStructuredComposer, StructuredInfoType, StructuredInfoTypeShape};
    let StructuredInfoTypeShape::Record { fields, .. } = acknowledgement.shape() else {
        panic!("acknowledgement record")
    };
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name(), "transactions");
    assert_eq!(
        fields[0].value_type(),
        &StructuredInfoType::leaf("value/u8".into()).unwrap()
    );
    let mut count = PreparedStructuredComposer::new(fields[0].value_type(), 64).unwrap();
    count.leaf(&[1]).unwrap();
    let mut value = PreparedStructuredComposer::new(acknowledgement, 2048).unwrap();
    value
        .record(&[conduit_core::validate_canonical_structured_value(count.encoded()).unwrap()])
        .unwrap();
    conduit_core::validate_canonical_structured_value(value.encoded()).unwrap();
    fs::write(
        out.join("ht16k33-acknowledgement.type"),
        acknowledgement.canonical_bytes().unwrap(),
    )
    .unwrap();
    fs::write(
        out.join("ht16k33-write-acknowledgement.value"),
        value.encoded(),
    )
    .unwrap();
}
