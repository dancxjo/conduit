//! Checked whole-protocol Source selection; no admission or hardware claim.
use conduit_mcu_source_build as source_build;
#[allow(dead_code)]
#[path = "../../../mcu/source-build/tests/fixtures/environmental_exchange_source.rs"]
mod environmental_exchange_source;
use conduit_core as c;
use conduit_host_make::*;
use conduit_host_rp2040::{CheckedProtocolConfigurationImages, Rp2040MakePackage};
use conduit_plot as p;
use sha2::{Digest, Sha256};
const GREEN: &str = include_str!("../../profiles/pico-w-house-green.host.conduit");
fn host(source: &str) -> CheckedHostConfiguration {
    let packages = MakePackageSet::compose(&[&Rp2040MakePackage]).unwrap();
    check_host_configuration(
        parse_host_configuration_conduit(source).unwrap(),
        &MakeCatalog::canonical().with_packages(&packages),
        &packages,
    )
    .unwrap()
}
fn types() -> p::CheckedSyntaxDocument {
    let units = [
        include_str!("../../../../plots/device-protocols/bme280-lifecycle.conduit"),
        include_str!("../../../../plots/device-protocols/ht16k33.conduit"),
    ];
    let imports = units
        .iter()
        .flat_map(|s| s.lines())
        .filter(|s| s.starts_with("with "))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join("\n");
    let body = units
        .iter()
        .flat_map(|s| s.lines())
        .filter(|s| !s.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    let mut startup = p::StartupCatalog::new();
    source_build::protocol_catalog::install(&mut startup, &mut p::ProfileCatalog::new()).unwrap();
    p::check_syntax_document(
        &p::parse_syntax_document(&format!("{imports}\n{body}")),
        &startup,
    )
    .unwrap()
}
fn selected_source(
    sensor_key: &str,
) -> (
    String,
    [source_build::composite::ValueReference<'static>; 2],
) {
    let checked = types();
    let definitions = ["BmeProtocolBegin", "Ht16k33Configuration"].map(|name| {
        let schema = &checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap()
            .value_type;
        let maximum_bytes = p::maximum_prepared_canonical_value_bytes(schema).unwrap();
        let contract = c::CheckedValueContract::new(
            schema.profile().unwrap().value_kind().clone(),
            maximum_bytes,
            vec![],
        )
        .unwrap();
        let kind =
            conduit_semantic_catalog::installed_value_once_semantic_contract(&contract, schema)
                .unwrap();
        (
            kind.kind_id,
            source_build::composite::ValueReference {
                type_name: name,
                maximum_bytes,
            },
        )
    });
    let source = format!(
        "with {} as SensorSettings\nwith {} as DisplaySettings\n{}\n",
        definitions[0].0.as_str(),
        definitions[1].0.as_str(),
        environmental_exchange_source::selected_source().source
    );
    let wrapper = format!(
        r#"
plot installed-environment (
 >> frame: Ht16k33Frame...|
 captured: BmeProtocolState...| >>
 observation: BmeObservationResult...| >>
 sensor_refused: BmeProtocolFailure...| >>
 clock_failed: ClockResult...| >>
 initialized: Ht16k33Acknowledgement...| >>
 initialization_refused: Ht16k33Failure...| >>
 written: Ht16k33Acknowledgement...| >>
 write_refused: Ht16k33Failure...| >>
 unrouted: I2cCorrelatedResult...| >>
) {{
 sensor: SensorSettings(key = "{sensor_key}")
 display: DisplaySettings(key = "display-settings")
 instrument: shared-environment()
 sensor.value >> instrument.begin
 display.value >> instrument.configuration
 frame >> instrument.frame
 instrument.captured >> captured
 instrument.observation >> observation
 instrument.sensor_refused >> sensor_refused
 instrument.clock_failed >> clock_failed
 instrument.initialized >> initialized
 instrument.initialization_refused >> initialization_refused
 instrument.written >> written
 instrument.write_refused >> write_refused
 instrument.unrouted >> unrouted
}}
"#
    );
    (
        source + &wrapper,
        definitions.map(|(_, selection)| selection),
    )
}
#[test]
fn actual_green_configuration_selects_internal_producers_in_the_whole_protocol_workset() {
    let checked_host = host(GREEN);
    let changed_host = host(&GREEN.replace("brightness: 3", "brightness: 9"));
    let (source, selections) = selected_source("sensor-settings");
    let mut retained = None;
    let workset = source_build::workset::compile_specialized_with_installed(
        source.as_bytes(),
        &format!("{:x}", Sha256::digest(source.as_bytes())),
        &["installed-environment".into()],
        None,
        &environmental_exchange_source::specializations(),
        &selections,
        |checked| {
            let images = || {
                CheckedProtocolConfigurationImages::prepare(&checked_host, checked, 0, true)
                    .unwrap()
            };
            assert!(images()
                .into_bindings(
                    &changed_host,
                    checked,
                    "sensor-settings",
                    "display-settings"
                )
                .is_err());
            let bindings = images().into_bindings(
                &checked_host,
                checked,
                "sensor-settings",
                "display-settings",
            )?;
            bindings.validate_configuration(&checked_host)?;
            assert!(bindings.validate_configuration(&changed_host).is_err());
            retained = Some((checked.clone(), bindings.clone()));
            Ok(bindings)
        },
    )
    .unwrap();
    let (checked, bindings) = retained.unwrap();
    let entry = &workset.entries[0];
    let gears = &entry.plot.expanded.gears;
    assert_eq!(
        gears
            .iter()
            .filter(|g| g
                .kind_id
                .as_str()
                .starts_with(conduit_semantic_catalog::INSTALLED_VALUE_ONCE_KIND))
            .count(),
        2
    );
    assert_eq!(
        gears
            .iter()
            .filter(|g| g.kind_id.as_str() == conduit_i2c_transaction::contract::I2C_KIND)
            .count(),
        1
    );
    assert_eq!(gears.iter().filter(|g| g.kind_id.as_str() == conduit_monotonic_wait::clock_at::contract::CLOCK_KIND).count(), 1);
    assert_eq!(entry.plot.front.inputs().len(), 1);
    assert_eq!(entry.plot.front.inputs()[0].port_id.as_str(), "frame");
    assert_eq!(entry.plot.front.outputs().len(), 9);
    let material = source_build::software::compile_with_installed(
        &entry.plot.expanded,
        Some(&checked),
        32 * 1024,
        &bindings,
    )
    .unwrap();
    let installed: Vec<_> = material
        .nodes
        .iter()
        .filter(|n| {
            matches!(
                n.role,
                source_build::software::SoftwareRole::Installed { .. }
            )
        })
        .collect();
    assert_eq!(installed.len(), 2);
    assert!(installed.iter().all(|n| n.offer_image.is_some()));
    assert_eq!(entry.rust.matches("D::LinkedSourceStatic").count(), 2);
}
#[test]
fn unavailable_configuration_key_refuses_the_original_complete_source() {
    let (source, selections) = selected_source("unavailable-sensor-settings");
    let checked_host = host(GREEN);
    let error = source_build::workset::compile_specialized_with_installed(
        source.as_bytes(),
        &format!("{:x}", Sha256::digest(source.as_bytes())),
        &["installed-environment".into()],
        None,
        &environmental_exchange_source::specializations(),
        &selections,
        |checked| {
            CheckedProtocolConfigurationImages::prepare(&checked_host, checked, 0, true)?
                .into_bindings(
                    &checked_host,
                    checked,
                    "sensor-settings",
                    "display-settings",
                )
        },
    )
    .err()
    .unwrap();
    assert!(error.contains("key unavailable"), "{error}");
}
