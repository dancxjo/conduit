use super::*;
use conduit_host_make::*;
fn checked(source: &str) -> Result<CheckedHostConfiguration, String> {
    let packages = MakePackageSet::compose(&[&Esp32MakePackage]).map_err(|e| format!("{e:?}"))?;
    let catalog = MakeCatalog::canonical().with_packages(&packages);
    let config = parse_host_configuration_conduit(source).map_err(|e| format!("{e:?}"))?;
    check_host_configuration(config, &catalog, &packages).map_err(|e| format!("{e:?}"))
}
#[test]
fn blue_source_selects_exact_physical_routes_and_finite_timings() {
    let source = include_str!("../../profiles/blue-wroom32.host.conduit");
    let config = checked(source).unwrap();
    let pins = physical::CheckedPhysicalConstruction::from_configuration(&config).unwrap();
    assert_eq!(
        (pins.sensor_data, pins.display_clock, pins.display_data),
        (27, 18, 19)
    );
    assert_eq!(
        (pins.start_low_us, pins.maximum_capture_transaction_us),
        (20000, 50000)
    );
    assert!(pins.weak_pullups);
    assert_eq!(pins.maximum_interrupt_exclusion_us, 20000);
    assert_eq!((pins.maximum_wait_ms, pins.wait_slots), (60000, 4));
    for (from, to) in [
        ("maximum_duration_ms: 60000", "maximum_duration_ms: 60001"),
        ("slots: 4}", "slots: 5}"),
        ("timer_slots: 4", "timer_slots: 3"),
        ("maximum_duration_ms: 60000, ", ""),
        ("data: 27", "data: 18"),
        ("dio: 19", "dio: 18"),
        ("tx: 1", "tx: 27"),
        (
            "maximum_interrupt_exclusion_us: 20000",
            "maximum_interrupt_exclusion_us: 0",
        ),
        (
            "maximum_interrupt_exclusion_us: 20000",
            "maximum_interrupt_exclusion_us: 20001",
        ),
        (", maximum_interrupt_exclusion_us: 20000", ""),
        (
            "start_low_us: 20000, maximum_transaction_us: 50000",
            "start_low_us: 30000, maximum_transaction_us: 35000",
        ),
        (
            "maximum_transaction_us: 50000",
            "maximum_transaction_us: 18000",
        ),
    ] {
        let changed = source.replace(from, to);
        assert_ne!(changed, source);
        let result = checked(&changed)
            .and_then(|c| physical::CheckedPhysicalConstruction::from_configuration(&c));
        assert!(
            result.is_err(),
            "accepted invalid attachment {from} -> {to}"
        );
    }
}

#[test]
fn blue_interrupt_exclusion_budget_is_part_of_checked_configuration_identity() {
    let source = include_str!("../../profiles/blue-wroom32.host.conduit");
    let original = checked(source).unwrap();
    let changed = checked(&source.replace(
        "maximum_interrupt_exclusion_us: 20000",
        "maximum_interrupt_exclusion_us: 9000",
    ))
    .unwrap();
    assert_ne!(original.configuration_id(), changed.configuration_id());
    assert_eq!(
        physical::CheckedPhysicalConstruction::from_configuration(&changed)
            .unwrap()
            .maximum_interrupt_exclusion_us,
        9000
    );
}

#[test]
fn blue_descriptor_reserves_only_native_equipment_and_bounded_dram() {
    let descriptor = crate::blue_descriptor::blue_wroom32_native();
    crate::validate_esp32_descriptor(&descriptor).unwrap();
    assert_eq!(descriptor.memory_regions[0].usable_bytes, 128 * 1024);
    assert_eq!(descriptor.memory_regions.len(), 2);
    assert_eq!(descriptor.memory_regions[1].physical_bytes, 98768);
    assert_eq!(descriptor.memory_regions[1].usable_bytes, 96 * 1024);
    assert_eq!(
        descriptor
            .memory_regions
            .iter()
            .map(|r| r.usable_bytes)
            .sum::<u64>(),
        224 * 1024
    );
    assert_eq!(
        descriptor.pins.iter().map(|p| p.gpio).collect::<Vec<_>>(),
        [1, 3, 18, 19, 27]
    );
    assert!(descriptor.radios.is_empty());
    println!(
        "BLUE_NATIVE_BINDING={}",
        crate::esp32_descriptor_binding(&descriptor).unwrap()
    );
}

#[test]
fn blue_renderer_look_and_lifetime_are_checked_construction() {
    use conduit_mcu_source_build::renderer::{CheckedRendererConstruction, TemperatureUnit};
    let source = include_str!("../../profiles/blue-wroom32.host.conduit");
    let configuration = checked(source).unwrap();
    let renderer =
        CheckedRendererConstruction::from_configuration(&configuration, physical::RENDERER)
            .unwrap();
    assert_eq!(renderer.temperature_unit, TemperatureUnit::Fahrenheit);
    assert_eq!(
        (renderer.page_period_ms, renderer.show_lifetime_ms),
        (2000, 120000)
    );
    for (from, to) in [
        (
            "temperature_unit: \"fahrenheit\"",
            "temperature_unit: \"kelvin\"",
        ),
        ("page_period_ms: 2000", "page_period_ms: 0"),
        ("show_lifetime_ms: 120000", "show_lifetime_ms: 1000"),
        ("show_lifetime_ms: 120000, ", ""),
    ] {
        let changed = source.replace(from, to);
        assert_ne!(changed, source);
        let result = checked(&changed).and_then(|configuration| {
            CheckedRendererConstruction::from_configuration(&configuration, physical::RENDERER)
        });
        assert!(
            result.is_err(),
            "accepted invalid renderer initialization: {from}->{to}"
        );
    }
}
