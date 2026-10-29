//! Checked mixed-bus attachment: Bosch I2C sensor and GPIO TM1637 display.
use super::*;

pub(super) fn rust(checked: &CheckedHostConfiguration) -> Result<String, String> {
    let c = checked.configuration();
    let [ConfigurationBus::I2c {
        id,
        controller,
        sda,
        scl,
        frequency_hz,
    }] = c.buses.as_slice()
    else {
        return Err("mixed environment requires exactly one sensor I2C bus".into());
    };
    let exposed = |pin: u8| pin <= 22 || (26..=28).contains(&pin);
    if *controller > 1
        || !exposed(*sda)
        || !exposed(*scl)
        || *scl != sda.saturating_add(1)
        || sda % 4 != controller * 2
        || !(10_000..=400_000).contains(frequency_hz)
    {
        return Err("invalid RP2040 sensor controller/SDA/SCL/frequency binding".into());
    }
    if c.devices.len() != 2 {
        return Err("mixed environment requires exactly sensor and display".into());
    }
    let mut sensor = None;
    let mut display = None;
    for device in &c.devices {
        match device {
            ConfigurationDevice::I2c {
                bus,
                address,
                implementation,
                configuration,
                ..
            } if implementation == "bosch-environment@1"
                && sensor.is_none()
                && bus == id
                && matches!(address, 0x76 | 0x77)
                && configuration.is_empty() =>
            {
                sensor = Some(*address);
            }
            ConfigurationDevice::Gpio {
                implementation,
                pins,
                configuration,
                ..
            } if implementation == "tm1637@1" && display.is_none() && pins.len() == 2 => {
                if pins.values().any(|pin| !exposed(*pin)) {
                    return Err("TM1637 uses reserved or unexposed Pico pin".into());
                }
                if configuration.keys().any(|key| {
                    !matches!(
                        key.as_str(),
                        "brightness" | "layout" | "half_period_us" | "weak_pullups"
                    )
                }) {
                    return Err("unsupported TM1637 parameter".into());
                }
                let clk = *pins.get("clk").ok_or("TM1637 requires clk role")?;
                let dio = *pins.get("dio").ok_or("TM1637 requires dio role")?;
                let Some(ConfigurationScalar::Unsigned(brightness @ 0..=7)) =
                    configuration.get("brightness")
                else {
                    return Err("TM1637 brightness must be unsigned0..7".into());
                };
                if !matches!(configuration.get("layout"),Some(ConfigurationScalar::Text(layout)) if layout == "seven4")
                {
                    return Err("TM1637 requires seven4 layout".into());
                }
                let half_period_us = match configuration.get("half_period_us") {
                    None => 5,
                    Some(ConfigurationScalar::Unsigned(value @ 5..=100)) => *value as u32,
                    _ => return Err("TM1637 half_period_us must be unsigned5..100".into()),
                };
                let weak_pullups = match configuration.get("weak_pullups") {
                    None => false,
                    Some(ConfigurationScalar::Boolean(value)) => *value,
                    _ => return Err("TM1637 weak_pullups must be boolean".into()),
                };
                if clk == dio || [*sda, *scl].contains(&clk) || [*sda, *scl].contains(&dio) {
                    return Err("mixed environment GPIO/I2C pin conflict".into());
                }
                display = Some((clk, dio, *brightness as u8, half_period_us, weak_pullups));
            }
            _ => return Err("unsupported mixed device, bus, address, roles or parameters".into()),
        }
    }
    let sensor = sensor.ok_or("missing Bosch sensor")?;
    let (clk, dio, brightness, half_period_us, weak_pullups) =
        display.ok_or("missing TM1637 display")?;
    let heap = c.limits.heap_arena_bytes;
    if !(32768..=98304).contains(&heap) {
        return Err("USB diagnostic heap outside supported32..96KiB".into());
    }
    let wifi = c.target.machine == "pico-w";
    let role = if wifi {
        "conduit.environment-wifi-carrier@1"
    } else {
        "conduit.environment-usb@1"
    };
    Ok(format!(
        r#"pub const CONFIGURATION_ID:&str={:?};
pub const ENVIRONMENT_PROFILE:&str="bosch-tm1637";
pub const HEAP_BYTES:usize={heap};
pub const WIFI_ALLOWED:bool={wifi};
pub const ROLE:&str={role:?};
pub const FREQUENCY:u32={frequency_hz};
pub const SENSOR_ADDRESS:u8={sensor};
pub const BRIGHTNESS:u8={brightness};
pub const DISPLAY_CLK_GPIO:u8={clk};
pub const DISPLAY_DIO_GPIO:u8={dio};
pub const DISPLAY_HALF_PERIOD_US:u32={half_period_us};
pub const DISPLAY_WEAK_PULLUPS:bool={weak_pullups};
pub type SensorBus=embassy_rp::peripherals::I2C{controller};
pub type SensorScl=embassy_rp::peripherals::PIN_{scl};
pub type SensorSda=embassy_rp::peripherals::PIN_{sda};
pub type Bus=SensorBus;
pub type Scl=SensorScl;
pub type Sda=SensorSda;
pub type DisplayClk=embassy_rp::peripherals::PIN_{clk};
pub type DisplayDio=embassy_rp::peripherals::PIN_{dio};
embassy_rp::bind_interrupts!(pub struct SensorIrqs {{ I2C{controller}_IRQ => embassy_rp::i2c::InterruptHandler<SensorBus>; }});
macro_rules! start_environment {{($spawner:expr,$p:ident)=>{{$spawner.spawn(crate::environment::run($p.I2C{controller},$p.PIN_{scl},$p.PIN_{sda},$p.PIN_{clk},$p.PIN_{dio}).unwrap());}}}}
pub(crate) use start_environment;
"#,
        checked.configuration_id()
    ))
}
