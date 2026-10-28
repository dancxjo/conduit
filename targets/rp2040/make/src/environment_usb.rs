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
    if c.devices
        .iter()
        .any(|d| matches!(d, ConfigurationDevice::Gpio { .. }))
    {
        return gpio_environment(checked);
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
        if d.bus() != Some(id.as_str()) {
            return Err("device bus differs".into());
        }
        let ConfigurationDevice::I2c {
            address,
            implementation,
            configuration,
            ..
        } = d
        else {
            return Err("I2C profile contains non-I2C device".into());
        };
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
pub const ENVIRONMENT_PROFILE:&str="bosch-ht16k33";
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

fn gpio_environment(checked: &CheckedHostConfiguration) -> Result<String, String> {
    let c = checked.configuration();
    if !c.buses.is_empty() || c.devices.len() != 2 {
        return Err("GPIO environment requires exactly sensor/display and no buses".into());
    }
    let mut sensor = None;
    let mut display = None;
    let exposed = |p: u8| p <= 22 || (26..=28).contains(&p);
    for device in &c.devices {
        let ConfigurationDevice::Gpio {
            implementation,
            pins,
            configuration,
            ..
        } = device
        else {
            return Err("mixed GPIO/I2C environment unsupported".into());
        };
        if pins.values().any(|p| !exposed(*p)) {
            return Err("GPIO device uses reserved or unexposed Pico pin".into());
        }
        match implementation.as_str() {
            "dht11@1" if sensor.is_none() && pins.len() == 1 && configuration.is_empty() => {
                sensor = Some(*pins.get("data").ok_or("DHT11 requires data pin role")?);
            }
            "tm1637@1" if display.is_none() && pins.len() == 2 => {
                if configuration.keys().any(|key| {
                    !matches!(
                        key.as_str(),
                        "brightness" | "layout" | "half_period_us" | "weak_pullups"
                    )
                }) {
                    return Err("unsupported TM1637 parameter".into());
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
                let clk = *pins.get("clk").ok_or("TM1637 requires clk role")?;
                let dio = *pins.get("dio").ok_or("TM1637 requires dio role")?;
                let Some(ConfigurationScalar::Unsigned(brightness @ 0..=7)) =
                    configuration.get("brightness")
                else {
                    return Err("TM1637 brightness must be unsigned0..7".into());
                };
                if !matches!(configuration.get("layout"),Some(ConfigurationScalar::Text(layout)) if layout=="seven4")
                {
                    return Err("TM1637 requires seven4 layout".into());
                }
                display = Some((clk, dio, *brightness as u8, half_period_us, weak_pullups));
            }
            _ => return Err("unsupported/duplicate GPIO driver, roles or parameters".into()),
        }
    }
    let sensor = sensor.ok_or("missing DHT11")?;
    let (clk, dio, brightness, half_period_us, weak_pullups) = display.ok_or("missing TM1637")?;
    if sensor == clk || sensor == dio || clk == dio {
        return Err("GPIO device pin conflict".into());
    }
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
pub const ENVIRONMENT_PROFILE:&str="dht11-tm1637";
pub const HEAP_BYTES:usize={heap};
pub const WIFI_ALLOWED:bool={wifi};
pub const ROLE:&str={role:?};
pub const DHT_GPIO:u8={sensor};
pub const DISPLAY_CLK_GPIO:u8={clk};
pub const DISPLAY_DIO_GPIO:u8={dio};
pub const DISPLAY_HALF_PERIOD_US:u32={half_period_us};
pub const DISPLAY_WEAK_PULLUPS:bool={weak_pullups};
pub const BRIGHTNESS:u8={brightness};
pub type DhtPin=embassy_rp::peripherals::PIN_{sensor};
pub type DisplayClk=embassy_rp::peripherals::PIN_{clk};
pub type DisplayDio=embassy_rp::peripherals::PIN_{dio};
pub type CapturePio=embassy_rp::peripherals::PIO1;
pub type CaptureDma=embassy_rp::peripherals::DMA_CH2;
macro_rules! start_environment {{($spawner:expr,$p:ident)=>{{$spawner.spawn(crate::environment::run($p.PIO1,$p.DMA_CH2,$p.PIN_{sensor},$p.PIN_{clk},$p.PIN_{dio}).unwrap());}}}}
pub(crate) use start_environment;
"#,
        checked.configuration_id()
    ))
}
