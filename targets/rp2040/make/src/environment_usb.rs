//! Consume checked Host construction into exact RP2040 peripheral selection.
use conduit_host_make::{
    CheckedHostConfiguration, ConfigurationBus, ConfigurationDevice, ConfigurationScalar,
};
pub fn environment_usb_rust(checked: &CheckedHostConfiguration) -> Result<String, String> {
    let c = checked.configuration();
    if c.target.architecture != "thumbv6m"
        || !matches!(c.target.machine.as_str(), "pico" | "pico-w")
    {
        return Err("environment USB requires RP2040 Pico target".into());
    }
    let [ConfigurationBus::I2c {
        id,
        controller,
        sda,
        scl,
        frequency_hz,
    }] = c.buses.as_slice()
    else {
        return Err("environment USB requires exactly one I2C bus".into());
    };
    let exposed = |p: u8| p <= 22 || (26..=28).contains(&p);
    if *controller > 1
        || !exposed(*sda)
        || !exposed(*scl)
        || *scl != sda.saturating_add(1)
        || sda % 4 != controller * 2
        || !(10_000..=400_000).contains(frequency_hz)
    {
        return Err("invalid RP2040 controller/SDA/SCL/frequency binding".into());
    }
    if c.devices.len() != 2 {
        return Err("environment USB requires sensor and display".into());
    }
    let mut sensor = None;
    let mut display = None;
    for d in &c.devices {
        if d.bus() != id {
            return Err("device bus differs".into());
        }
        let ConfigurationDevice::I2c {
            address,
            implementation,
            configuration,
            ..
        } = d;
        match implementation.as_str() {
            "bosch-environment@1"
                if sensor.is_none()
                    && matches!(address, 0x76 | 0x77)
                    && configuration.is_empty() =>
            {
                sensor = Some(*address);
            }
            "ht16k33@1" if display.is_none() && (0x70..=0x77).contains(address) => {
                if configuration.len() != 2 {
                    return Err("HT16K33 requires exactly brightness and layout".into());
                }
                let Some(ConfigurationScalar::Unsigned(brightness @ 0..=15)) =
                    configuration.get("brightness")
                else {
                    return Err("HT16K33 brightness must be unsigned0..15".into());
                };
                let Some(ConfigurationScalar::Text(layout)) = configuration.get("layout") else {
                    return Err("HT16K33 layout must be text".into());
                };
                if layout != "alpha4" {
                    return Err("unsupported HT16K33 layout".into());
                }
                display = Some((*address, *brightness as u8));
            }
            _ => {
                return Err("unsupported/duplicate implementation, configuration or address".into())
            }
        }
    }

    let sensor = sensor.ok_or("missing Bosch device")?;
    let (display, brightness) = display.ok_or("missing HT16K33 device")?;
    if sensor == display {
        return Err("I2C device address collision".into());
    }
    let wifi = c.target.machine == "pico-w";
    let role = if wifi {
        "conduit.environment-wifi-carrier@1"
    } else {
        "conduit.environment-usb@1"
    };
    let heap = c.limits.heap_arena_bytes;
    if !(32768..=98304).contains(&heap) {
        return Err("USB diagnostic heap outside supported32..96KiB".into());
    }
    Ok(format!(
        r#"pub const CONFIGURATION_ID:&str={:?};
pub const HEAP_BYTES:usize={heap};
pub const WIFI_ALLOWED:bool={wifi};
pub const ROLE:&str={role:?};
pub const FREQUENCY:u32={frequency_hz};
pub const SENSOR_ADDRESS:u8={sensor};
pub const DISPLAY_ADDRESS:u8={display};
pub const BRIGHTNESS:u8={brightness};
pub fn display_profile()->conduit_ht16k33::Profile{{conduit_ht16k33::Profile::adafruit_alpha4()}}
pub type Bus=embassy_rp::peripherals::I2C{controller};
pub type Scl=embassy_rp::peripherals::PIN_{scl};
pub type Sda=embassy_rp::peripherals::PIN_{sda};
embassy_rp::bind_interrupts!(pub struct SensorIrqs {{ I2C{controller}_IRQ => embassy_rp::i2c::InterruptHandler<Bus>; }});
macro_rules! start_environment {{($spawner:expr,$p:ident)=>{{$spawner.spawn(crate::environment::run($p.I2C{controller},$p.PIN_{scl},$p.PIN_{sda}).unwrap());}}}}
pub(crate) use start_environment;
"#,
        checked.configuration_id()
    ))
}
