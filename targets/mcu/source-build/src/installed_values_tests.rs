//! Original checked Source, Host material and ordinary software selection.
use super::*;
const TYPES: &str = "type Settings = {\n setting: U8 in 8..=119\n}\n";
fn checked(text: &str) -> p::CheckedSyntaxDocument {
    p::check_syntax_document(&p::parse_syntax_document(text), &p::StartupCatalog::new()).unwrap()
}
fn input(
    source: &p::CheckedSyntaxDocument,
    key: &str,
    setting: u8,
    maximum: u32,
) -> InstalledValueInput {
    let native = source
        .native_types
        .iter()
        .find(|ty| ty.name == "Settings")
        .unwrap();
    let c::StructuredInfoTypeShape::Record { fields, .. } = native.value_type.shape() else {
        panic!("record");
    };
    let field =
        c::StructuredInfoValue::leaf(fields[0].value_type().clone(), vec![setting]).unwrap();
    let value = c::StructuredInfoValue::record(
        native.value_type.clone(),
        vec![c::StructuredFieldValue::new("setting", field).unwrap()],
    )
    .unwrap();
    InstalledValueInput {
        key: key.into(),
        type_name: "Settings".into(),
        maximum_bytes: maximum,
        image: LinkedSourceImage::prepare(native, &value).unwrap(),
    }
}
fn source(key: &str, maximum: u32) -> String {
    let source = checked(TYPES);
    let schema = &source.native_types[0].value_type;
    let kind = conduit_semantic_catalog::installed_value_once_semantic_contract(
        &contract(schema, maximum).unwrap(),
        schema,
    )
    .unwrap();
    format!("with {} as SettingsSource\n{TYPES}\nplot sample (\n value: Settings...| <= {maximum}B >>\n) {{\n settings: SettingsSource(key = \"{key}\")\n settings.value >> value\n}}\n", kind.kind_id.as_str())
}
fn workset(key: &str, setting: u8) -> Result<crate::workset::Workset, String> {
    let text = source(key, 512);
    crate::workset::compile_specialized_with_installed(
        text.as_bytes(),
        &format!("{:x}", Sha256::digest(text.as_bytes())),
        &["sample".into()],
        None,
        &[],
        &[ValueReference {
            type_name: "Settings",
            maximum_bytes: 512,
        }],
        |source| {
            InstalledValueBindings::prepare(
                "fixture/configuration",
                source,
                vec![input(source, "sensor-settings", setting, 512)],
            )
        },
    )
}
#[test]
fn original_checked_workset_selects_and_emits_the_actual_installed_provider() {
    let workset = workset("sensor-settings", 118).unwrap();
    let entry = &workset.entries[0];
    let material = crate::software::compile_with_installed(
        &entry.plot.expanded,
        Some(&workset.checked_source),
        24 * 1024,
        &workset.installed_values,
    )
    .unwrap();
    assert_eq!(material.nodes.len(), 1);
    assert!(matches!(
        material.nodes[0].role,
        crate::software::SoftwareRole::Installed { binding: 0, .. }
    ));
    assert!(material.nodes[0].offer_image.is_some());
    assert!(entry.rust.contains("D::LinkedSourceStatic"));
    assert!(entry.rust.contains("super::installed_values::VALUE_0"));
    let shared = workset.installed_values.emit_shared();
    assert_eq!(shared.matches("pub static VALUE_0:").count(), 1);
    // Re-emission under an explicitly selected layout must retain this same
    // material, rather than fall back to a literal or an ambient Host lookup.
    let mut regenerated = crate::software::emit_rust_for_layout_with_installed(
        &entry.plot.expanded,
        Some(&workset.checked_source),
        &[0],
        24 * 1024,
        1024 * 1024,
        &workset.installed_values,
    )
    .unwrap();
    crate::workset::emit_mask_routing_images(&entry.plot.expanded, &mut regenerated, None).unwrap();
    assert_eq!(entry.rust, regenerated);
    let generated = format!("{shared}pub mod entry{{{regenerated}}}");
    crate::compiled_rust::verify_with_libraries(
        &generated,
        r#"
fn main() {
    use conduit_mcu_host_runtime::execution_factory::SoftwareDefinition as D;
    let Some(D::LinkedSourceStatic { schema, value, maximum_bytes }) = entry::definition_at(0, &[]).unwrap() else { panic!("installed owner"); };
    assert_eq!(schema.as_ptr(), installed_values::SCHEMA_0.as_ptr());
    assert_eq!(value.as_ptr(), installed_values::VALUE_0.as_ptr());
    assert_eq!(maximum_bytes, 512);
    assert!(entry::identity_at(0, &[]).unwrap().is_some());
    conduit_mcu_host_runtime::execution_software::SoftwareBack::linked_source_static(schema, value, maximum_bytes).unwrap();
}
"#,
        "installed-source-workset-provider",
        &[
            "conduit_core",
            "conduit_kernel",
            "conduit_plan_lowering",
            "conduit_environment_runtime",
            "conduit_mcu_host_runtime",
        ],
    );
    assert!(crate::software::compile_with_types(
        &entry.plot.expanded,
        Some(&workset.checked_source),
        24 * 1024
    )
    .is_err());
}
#[test]
fn unavailable_and_duplicate_keys_never_select_another_datum() {
    assert!(workset("missing-settings", 118)
        .err()
        .unwrap()
        .contains("key unavailable"));
    let source = checked(TYPES);
    assert!(InstalledValueBindings::prepare(
        "configuration",
        &source,
        vec![input(&source, "a", 118, 512), input(&source, "a", 119, 512)]
    )
    .err()
    .unwrap()
    .contains("duplicate"));
    assert!(InstalledValueBindings::prepare(
        "configuration",
        &source,
        vec![input(&source, "", 118, 512)]
    )
    .is_err());
}
#[test]
fn original_source_and_material_identity_remain_independent_pins() {
    let first = workset("sensor-settings", 118).unwrap();
    let second = workset("sensor-settings", 119).unwrap();
    assert_eq!(first.sha256, second.sha256);
    let first_material = crate::software::compile_with_installed(
        &first.entries[0].plot.expanded,
        Some(&first.checked_source),
        24 * 1024,
        &first.installed_values,
    )
    .unwrap();
    let second_material = crate::software::compile_with_installed(
        &second.entries[0].plot.expanded,
        Some(&second.checked_source),
        24 * 1024,
        &second.installed_values,
    )
    .unwrap();
    assert_ne!(
        first_material.nodes[0].implementation,
        second_material.nodes[0].implementation
    );
    assert_ne!(
        first_material.nodes[0].offer_image,
        second_material.nodes[0].offer_image
    );
    assert!(first
        .installed_values
        .validate_source(&checked(&format!(
            "{TYPES}\ntype Other = {{\n value: U64\n}}\n"
        )))
        .is_err());
    let source = checked(TYPES);
    let a = InstalledValueBindings::prepare(
        "configuration/a",
        &source,
        vec![input(&source, "key", 118, 512)],
    )
    .unwrap();
    let b = InstalledValueBindings::prepare(
        "configuration/b",
        &source,
        vec![input(&source, "key", 118, 512)],
    )
    .unwrap();
    assert_ne!(a.values[0].digest, b.values[0].digest);
}
#[test]
fn exact_schema_native_laws_and_selected_envelope_are_rechecked() {
    let source = checked(TYPES);
    let mut wrong = input(&source, "key", 118, 512);
    wrong.type_name = "Missing".into();
    assert!(InstalledValueBindings::prepare("configuration", &source, vec![wrong]).is_err());
    let image = input(&source, "key", 118, 512);
    let actual = image.image.value().len() as u32;
    let mut too_small = image;
    too_small.maximum_bytes = actual - 1;
    assert!(
        InstalledValueBindings::prepare("configuration", &source, vec![too_small])
            .err()
            .unwrap()
            .contains("envelope")
    );
    for maximum in [0, 4097] {
        assert!(InstalledValueBindings::prepare(
            "configuration",
            &source,
            vec![input(&source, "key", 118, maximum)]
        )
        .is_err());
    }
    // An image lawfully built against a wider Native cannot be substituted
    // for the original Source's stricter range, even with the same record ABI.
    let wider = checked("type Settings = {\n setting: U8 in 0..=255\n}\n");
    assert!(InstalledValueBindings::prepare(
        "configuration",
        &source,
        vec![input(&wider, "key", 120, 512)]
    )
    .is_err());
}

#[test]
fn recalibration_cannot_replace_the_original_installed_material() {
    let first = workset("sensor-settings", 118).unwrap();
    let mut changed = workset("sensor-settings", 119).unwrap();
    let before = changed.entries[0].rust.clone();
    let calibration = crate::template_calibration::Calibration {
        entries: vec![],
        installed_values_identity: first.installed_values.identity(),
    };
    assert!(calibration
        .apply_software_mapping(&mut changed, None, 24 * 1024, 1024 * 1024)
        .unwrap_err()
        .contains("different installed material"));
    assert_eq!(changed.entries[0].rust, before);
}
