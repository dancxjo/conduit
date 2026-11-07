//! Physical GPIO construction only: no device initialization or execution grants.
use crate::hardware_initialization::{count, selected_values, value};
use crate::{
    bosch_initialization_contract, CheckedI2cConstruction, BOSCH_BASE_KIND, BOSCH_IMPLEMENTATION,
};
use conduit_host_make::{
    BaseInitializationContract, CheckedHostConfiguration, ConfigurationValue,
    KindConfigurationField, KindConfigurationRule,
};

pub const DHT11_BASE_KIND: &str = "sensor/environment";
pub const DHT11_IMPLEMENTATION: &str = "rp2040/dht11@1";
pub const TM1637_BASE_KIND: &str = "display/segmented";
pub const TM1637_IMPLEMENTATION: &str = "rp2040/tm1637@1";

/// Validate a supported physical composition before invoking its target builder.
/// This checks construction only; no equipment or execution authority is issued.
pub fn check_environmental_construction(checked: &CheckedHostConfiguration) -> Result<(), String> {
    let selected = |name| checked.resolved_bases().iter().any(|(_, id)| id == name);
    if selected(DHT11_IMPLEMENTATION) {
        CheckedDht11Tm1637Construction::from_configuration(checked).map(|_| ())
    } else if selected(TM1637_IMPLEMENTATION) {
        CheckedBoschTm1637Construction::from_configuration(checked).map(|_| ())
    } else {
        crate::CheckedBoschHt16k33Construction::from_configuration(checked).map(|_| ())
    }
}

pub fn dht11_initialization_contract() -> BaseInitializationContract {
    BaseInitializationContract {
        fields: vec![
            count("data", 0, 28),
            count("start_low_us", 18_000, 30_000),
            count("maximum_transaction_us", 30_000, 100_000),
        ],
    }
}
pub fn tm1637_initialization_contract() -> BaseInitializationContract {
    BaseInitializationContract {
        fields: vec![
            count("clk", 0, 28),
            count("dio", 0, 28),
            count("brightness", 0, 7),
            KindConfigurationField {
                key: "layout".into(),
                default_value: ConfigurationValue::Text(String::new()),
                rule: KindConfigurationRule::TextOneOf {
                    values: vec!["seven4".into()],
                },
            },
            count("half_period_us", 1, 1_000),
            count("settled_us", 0, 100_000),
            count("maximum_clock_rise_us", 1, 10_000),
            count("maximum_transaction_us", 1, 1_000_000),
            KindConfigurationField {
                key: "pull".into(),
                default_value: ConfigurationValue::Text("none".into()),
                rule: KindConfigurationRule::TextOneOf {
                    values: vec!["none".into(), "up".into()],
                },
            },
        ],
    }
}
fn exposed(pin: u8) -> bool {
    pin <= 22 || (26..=28).contains(&pin)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDht11Construction {
    pub configuration_id: String,
    pub data_gpio: u8,
    pub start_low_us: u32,
    pub maximum_transaction_us: u32,
}
impl CheckedDht11Construction {
    pub fn from_configuration(checked: &CheckedHostConfiguration) -> Result<Self, String> {
        let v = selected_values(
            checked,
            DHT11_BASE_KIND,
            DHT11_IMPLEMENTATION,
            &dht11_initialization_contract(),
        )?;
        let data_gpio = value(&v, "data")? as u8;
        if !exposed(data_gpio) {
            return Err("DHT11 GPIO is reserved or unavailable".into());
        }
        let start_low_us = value(&v, "start_low_us")? as u32;
        let maximum_transaction_us = value(&v, "maximum_transaction_us")? as u32;
        if maximum_transaction_us <= start_low_us {
            return Err("DHT11 transaction must leave time for capture after start pulse".into());
        }
        Ok(Self {
            configuration_id: checked.configuration_id().into(),
            data_gpio,
            start_low_us,
            maximum_transaction_us,
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedTm1637Construction {
    pub configuration_id: String,
    pub clock_gpio: u8,
    pub data_gpio: u8,
    pub brightness: u8,
    pub half_period_us: u32,
    pub settled_us: u32,
    pub maximum_clock_rise_us: u32,
    pub maximum_transaction_us: u32,
    pub weak_pullups: bool,
}
impl CheckedTm1637Construction {
    pub fn from_configuration(checked: &CheckedHostConfiguration) -> Result<Self, String> {
        let v = selected_values(
            checked,
            TM1637_BASE_KIND,
            TM1637_IMPLEMENTATION,
            &tm1637_initialization_contract(),
        )?;
        let clock_gpio = value(&v, "clk")? as u8;
        let data_gpio = value(&v, "dio")? as u8;
        if !exposed(clock_gpio) || !exposed(data_gpio) || clock_gpio == data_gpio {
            return Err("TM1637 GPIOs must be distinct exposed pins".into());
        }
        let weak_pullups = match v.get("pull") {
            Some(ConfigurationValue::Text(pull)) if pull == "none" => false,
            Some(ConfigurationValue::Text(pull)) if pull == "up" => true,
            _ => return Err("pull must be none or up".into()),
        };
        let half_period_us = value(&v, "half_period_us")? as u32;
        let settled_us = value(&v, "settled_us")? as u32;
        let maximum_clock_rise_us = value(&v, "maximum_clock_rise_us")? as u32;
        let maximum_transaction_us = value(&v, "maximum_transaction_us")? as u32;
        if maximum_transaction_us <= settled_us + half_period_us + maximum_clock_rise_us {
            return Err(
                "TM1637 transaction budget cannot contain even one settled clock edge".into(),
            );
        }
        Ok(Self {
            configuration_id: checked.configuration_id().into(),
            clock_gpio,
            data_gpio,
            brightness: value(&v, "brightness")? as u8,
            half_period_us,
            settled_us,
            maximum_clock_rise_us,
            maximum_transaction_us,
            weak_pullups,
        })
    }
    fn excludes(&self, pins: &[u8]) -> Result<(), String> {
        if pins.contains(&self.clock_gpio) || pins.contains(&self.data_gpio) {
            return Err("sensor/display physical GPIO ownership collides".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDht11Tm1637Construction {
    pub sensor: CheckedDht11Construction,
    pub display: CheckedTm1637Construction,
}
impl CheckedDht11Tm1637Construction {
    pub fn from_configuration(checked: &CheckedHostConfiguration) -> Result<Self, String> {
        let sensor = CheckedDht11Construction::from_configuration(checked)?;
        let display = CheckedTm1637Construction::from_configuration(checked)?;
        display.excludes(&[sensor.data_gpio])?;
        if checked
            .configuration()
            .bases
            .iter()
            .any(|b| b.kind == crate::I2C_BASE_KIND)
        {
            let bus = CheckedI2cConstruction::from_configuration(checked)?;
            display.excludes(&[bus.sda_gpio, bus.scl_gpio])?;
            if [bus.sda_gpio, bus.scl_gpio].contains(&sensor.data_gpio) {
                return Err("DHT11/I2C physical GPIO ownership collides".into());
            }
        }
        Ok(Self { sensor, display })
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedBoschTm1637Construction {
    pub bus: CheckedI2cConstruction,
    pub sensor_address: u8,
    pub display: CheckedTm1637Construction,
}
impl CheckedBoschTm1637Construction {
    pub fn from_configuration(checked: &CheckedHostConfiguration) -> Result<Self, String> {
        let bus = CheckedI2cConstruction::from_configuration(checked)?;
        let sensor = selected_values(
            checked,
            BOSCH_BASE_KIND,
            BOSCH_IMPLEMENTATION,
            &bosch_initialization_contract(),
        )?;
        if value(&sensor, "controller")? != u64::from(bus.controller) {
            return Err("Bosch sensor/I2C controller mismatch".into());
        }
        let display = CheckedTm1637Construction::from_configuration(checked)?;
        display.excludes(&[bus.sda_gpio, bus.scl_gpio])?;
        Ok(Self {
            bus,
            sensor_address: value(&sensor, "address")? as u8,
            display,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_host_make::{
        check_host_configuration, parse_host_configuration_conduit, MakeCatalog, MakePackageSet,
    };
    const GREEN: &str = include_str!("../../profiles/pico-w-house-green.host.conduit");
    const DISPLAY: &str = "  base = {kind: \"display/segmented\", implementation: \"rp2040/tm1637@1\", initialization: {clk: 7, dio: 6, brightness: 3, layout: \"seven4\", half_period_us: 50, settled_us: 1000, maximum_clock_rise_us: 1000, maximum_transaction_us: 100000, pull: \"up\"}}";
    const SENSOR: &str = "  base = {kind: \"sensor/environment\", implementation: \"rp2040/dht11@1\", initialization: {data: 3, start_low_us: 20000, maximum_transaction_us: 50000}}";
    fn source(yellow: bool) -> String {
        GREEN
            .lines()
            .filter_map(|l| {
                if l.contains("rp2040/ht16k33@1") {
                    Some(if yellow {
                        DISPLAY.into()
                    } else {
                        DISPLAY.replace("clk: 7, dio: 6", "clk: 3, dio: 4")
                    })
                } else if yellow && l.contains("rp2040/bosch-environment@1") {
                    Some(SENSOR.into())
                } else if yellow && l.contains("rp2040/i2c@1") {
                    None
                } else {
                    Some(l.into())
                }
            })
            .collect::<Vec<String>>()
            .join("\n")
    }
    fn checked(source: &str) -> Result<CheckedHostConfiguration, String> {
        let packages =
            MakePackageSet::compose(&[&crate::Rp2040MakePackage]).map_err(|e| format!("{e:?}"))?;
        let catalog = MakeCatalog::canonical().with_packages(&packages);
        let configuration =
            parse_host_configuration_conduit(source).map_err(|e| format!("{e:?}"))?;
        check_host_configuration(configuration, &catalog, &packages).map_err(|e| format!("{e:?}"))
    }
    fn yellow(source: &str) -> Result<CheckedDht11Tm1637Construction, String> {
        CheckedDht11Tm1637Construction::from_configuration(&checked(source)?)
    }
    fn white(source: &str) -> Result<CheckedBoschTm1637Construction, String> {
        CheckedBoschTm1637Construction::from_configuration(&checked(source)?)
    }
    #[test]
    fn exact_attended_yellow_and_white_pin_bindings() {
        for source in [
            GREEN,
            include_str!("../../profiles/pico-w-house-yellow.host.conduit"),
            include_str!("../../profiles/pico-w-house-white.host.conduit"),
        ] {
            check_environmental_construction(&checked(source).unwrap()).unwrap();
        }
        let installed_yellow = yellow(include_str!(
            "../../profiles/pico-w-house-yellow.host.conduit"
        ))
        .unwrap();
        let installed_white = white(include_str!(
            "../../profiles/pico-w-house-white.host.conduit"
        ))
        .unwrap();
        assert_eq!(installed_yellow.sensor.data_gpio, 3);
        assert_eq!(installed_yellow.display.clock_gpio, 7);
        assert_eq!(installed_yellow.display.data_gpio, 6);
        assert_eq!(installed_white.bus.sda_gpio, 6);
        assert_eq!(installed_white.bus.scl_gpio, 7);
        assert_eq!(installed_white.display.clock_gpio, 3);
        assert_eq!(installed_white.display.data_gpio, 4);
        let y = yellow(&source(true)).unwrap();
        assert_eq!(
            (
                y.sensor.data_gpio,
                y.display.clock_gpio,
                y.display.data_gpio
            ),
            (3, 7, 6)
        );
        assert_eq!(y.display.half_period_us, 50);
        assert!(y.display.weak_pullups);
        let w = white(&source(false)).unwrap();
        assert_eq!(
            (
                w.bus.controller,
                w.bus.sda_gpio,
                w.bus.scl_gpio,
                w.sensor_address
            ),
            (1, 6, 7, 118)
        );
        assert_eq!((w.display.clock_gpio, w.display.data_gpio), (3, 4));
    }
    #[test]
    fn ownership_conflicts_reserved_pins_and_wrong_bus_refuse() {
        let y = source(true);
        for (from, to) in [
            ("data: 3", "data: 7"),
            ("data: 3", "data: 25"),
            ("dio: 6", "dio: 7"),
            ("clk: 7", "clk: 23"),
        ] {
            assert!(yellow(&y.replace(from, to)).is_err());
        }
        let w = source(false);
        assert!(white(&w.replace("clk: 3", "clk: 6")).is_err());
        assert!(
            white(&w.replace("controller: 1, address: 118", "controller: 0, address: 118"))
                .is_err()
        );
        let bus_line = GREEN.lines().find(|l| l.contains("rp2040/i2c@1")).unwrap();
        assert!(yellow(&y.replace(SENSOR, &format!("{SENSOR}\n{bus_line}"))).is_err());
    }
    #[test]
    fn explicit_finite_timing_brightness_and_pull_are_checked() {
        let y = source(true);
        for (from, to) in [
            (
                "maximum_transaction_us: 50000",
                "maximum_transaction_us: 20000",
            ),
            (
                "maximum_transaction_us: 100000",
                "maximum_transaction_us: 1000",
            ),
            ("half_period_us: 50", "half_period_us: 0"),
            ("brightness: 3", "brightness: 8"),
            ("pull: \"up\"", "pull: 1"),
            ("pull: \"up\"", "pull: \"sideways\""),
            ("layout: \"seven4\"", "layout: \"unknown\""),
        ] {
            assert!(y.contains(from), "fixture lacks mutation target: {from}");
            assert!(yellow(&y.replace(from, to)).is_err(), "{from} => {to}");
        }
        let old = yellow(&y).unwrap();
        let new = yellow(&y.replace("half_period_us: 50", "half_period_us: 60")).unwrap();
        assert_ne!(old.display.configuration_id, new.display.configuration_id);
    }
}
