use conduit_host_make::*;
const WHITE: &str = include_str!("../../profiles/pico-w-bme280-tm1637.host.conduit");
fn checked(source: &str) -> Result<CheckedHostConfiguration, String> {
    let packages = MakePackageSet::compose(&[&conduit_host_rp2040::Rp2040MakePackage]).unwrap();
    let catalog = MakeCatalog::canonical().with_packages(&packages);
    let config = parse_host_configuration_conduit(source).map_err(|e| format!("{e:?}"))?;
    check_host_configuration(config, &catalog, &packages).map_err(|e| format!("{e:?}"))
}
fn lower(source: &str) -> Result<String, String> {
    conduit_host_rp2040::environment_usb_rust(&checked(source)?)
}
#[test]
fn white_mixed_profile_retains_exact_sensor_and_display_ownership() {
    let a = checked(WHITE).unwrap();
    let rust = conduit_host_rp2040::environment_usb_rust(&a).unwrap();
    for expected in [
        "ENVIRONMENT_PROFILE:&str=\"bosch-tm1637\"",
        "FREQUENCY:u32=100000",
        "SENSOR_ADDRESS:u8=118",
        "SensorBus=embassy_rp::peripherals::I2C1",
        "SensorScl=embassy_rp::peripherals::PIN_7",
        "SensorSda=embassy_rp::peripherals::PIN_6",
        "DisplayClk=embassy_rp::peripherals::PIN_3",
        "DisplayDio=embassy_rp::peripherals::PIN_4",
        "DISPLAY_HALF_PERIOD_US:u32=50",
        "DISPLAY_WEAK_PULLUPS:bool=true",
        "I2C1_IRQ => embassy_rp::i2c::InterruptHandler<SensorBus>",
        "$p.I2C1,$p.PIN_7,$p.PIN_6,$p.PIN_3,$p.PIN_4",
        "HEAP_BYTES:usize=49152",
        "WIFI_ALLOWED:bool=true",
    ] {
        assert!(rust.contains(expected), "missing {expected}");
    }
    assert!(rust.contains(a.configuration_id()));
    assert!(!rust.contains("PIO1") && !rust.contains("DMA_CH2"));
    let canonical = canonical_host_configuration_conduit(a.configuration()).unwrap();
    assert_eq!(
        a.configuration_id(),
        checked(&canonical).unwrap().configuration_id()
    );
    let alternate = WHITE
        .replace(
            "controller: 1, sda: 6, scl: 7",
            "controller: 0, sda: 0, scl: 1",
        )
        .replace("address: 118", "address: 119");
    let b = checked(&alternate).unwrap();
    assert_ne!(a.configuration_id(), b.configuration_id());
    assert!(lower(&alternate)
        .unwrap()
        .contains("$p.I2C0,$p.PIN_1,$p.PIN_0,$p.PIN_3,$p.PIN_4"));
}
#[test]
fn mixed_lowering_refuses_mismatched_sensor_and_unowned_display_pins() {
    for bad in [
        WHITE.replace("controller: 1", "controller: 0"),
        WHITE.replace("sda: 6, scl: 7", "sda: 7, scl: 6"),
        WHITE.replace("sda: 6, scl: 7", "sda: 22, scl: 23"),
        WHITE.replace("frequency_hz: 100000", "frequency_hz: 500000"),
        WHITE.replace("address: 118", "address: 117"),
        WHITE.replace("bosch-environment@1", "dht11@1"),
        WHITE.replace("tm1637@1", "ht16k33@1"),
        WHITE.replace("clk: 3, dio: 4", "clk: 6, dio: 4"),
        WHITE.replace("clk: 3, dio: 4", "clk: 3, dio: 7"),
        WHITE.replace("clk: 3, dio: 4", "clk: 3, dio: 3"),
        WHITE.replace("clk: 3, dio: 4", "clk: 23, dio: 4"),
        WHITE.replace("clk: 3, dio: 4", "clock: 3, dio: 4"),
        WHITE.replace("brightness: 3", "brightness: 8"),
        WHITE.replace("layout: \"seven4\"", "layout: \"alpha4\""),
        WHITE.replace("half_period_us: 50", "half_period_us: 101"),
        WHITE.replace("weak_pullups: true", "weak_pullups: 1"),
        WHITE.replace("brightness: 3", "brightness: 3, mystery: true"),
        WHITE.replace(
            "implementation: \"bosch-environment@1\"",
            "implementation: \"bosch-environment@1\", configuration: {mystery: true}",
        ),
        WHITE.replace("heap_arena_bytes: 49152", "heap_arena_bytes: 16384"),
    ] {
        assert!(
            lower(&bad).is_err(),
            "accepted invalid mixed profile: {bad}"
        );
    }
}
#[test]
fn existing_green_and_dht_lowerings_remain_distinct_and_unchanged() {
    let green = lower(include_str!(
        "../../profiles/pico-w-environment-usb.host.conduit"
    ))
    .unwrap();
    let dht = lower(include_str!(
        "../../profiles/pico-w-dht11-tm1637.host.conduit"
    ))
    .unwrap();
    assert!(green.contains("ENVIRONMENT_PROFILE:&str=\"bosch-ht16k33\""));
    assert!(green.contains("$p.I2C1,$p.PIN_7,$p.PIN_6"));
    assert!(!green.contains("DisplayClk"));
    assert!(dht.contains("ENVIRONMENT_PROFILE:&str=\"dht11-tm1637\""));
    assert!(dht.contains("$p.PIO1,$p.DMA_CH2,$p.PIN_3,$p.PIN_7,$p.PIN_6"));
    assert!(!dht.contains("SensorBus"));
}
