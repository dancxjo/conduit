//! Typed physical construction requests. Descriptions grant no bus possession.
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum ConfigurationBus {
    #[serde(rename = "i2c")]
    I2c {
        id: String,
        controller: u8,
        sda: u8,
        scl: u8,
        frequency_hz: u32,
    },
}
/// Finite construction parameters; no executable driver or runtime authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConfigurationScalar {
    Boolean(bool),
    Unsigned(u64),
    Text(String),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum ConfigurationDevice {
    #[serde(rename = "i2c")]
    I2c {
        id: String,
        bus: String,
        address: u8,
        implementation: String,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        configuration: BTreeMap<String, ConfigurationScalar>,
    },
}
impl ConfigurationBus {
    pub fn id(&self) -> &str {
        match self {
            Self::I2c { id, .. } => id,
        }
    }
}
impl ConfigurationDevice {
    pub fn id(&self) -> &str {
        match self {
            Self::I2c { id, .. } => id,
        }
    }
    pub fn bus(&self) -> &str {
        match self {
            Self::I2c { bus, .. } => bus,
        }
    }
    pub fn address(&self) -> u8 {
        match self {
            Self::I2c { address, .. } => *address,
        }
    }
}
pub(super) fn validate(
    buses: &[ConfigurationBus],
    devices: &[ConfigurationDevice],
) -> Result<(), String> {
    use crate::{MAX_PROFILE_ID_BYTES, MAX_PROFILE_ITEMS};
    if buses.len() > MAX_PROFILE_ITEMS || devices.len() > MAX_PROFILE_ITEMS {
        return Err("attachment count exceeds finite profile".into());
    }
    let name = |s: &str| {
        !s.is_empty() && s.len() <= MAX_PROFILE_ID_BYTES && s.bytes().all(|b| b.is_ascii_graphic())
    };
    let mut bus_ids = BTreeSet::new();
    let mut pins = BTreeSet::new();
    let mut controllers = BTreeSet::new();
    for bus in buses {
        match bus {
            ConfigurationBus::I2c {
                id,
                controller,
                sda,
                scl,
                frequency_hz,
            } => {
                if !name(id) || !bus_ids.insert(id.as_str()) {
                    return Err("invalid or duplicate bus id".into());
                }
                if sda == scl
                    || !pins.insert(*sda)
                    || !pins.insert(*scl)
                    || !controllers.insert(*controller)
                {
                    return Err("conflicting bus controller/pins".into());
                }
                if *frequency_hz == 0 || *frequency_hz > 1_000_000 {
                    return Err("I2C frequency outside finite bound".into());
                }
            }
        }
    }
    let mut ids = BTreeSet::new();
    let mut addresses = BTreeMap::new();
    for device in devices {
        if !name(device.id()) || !ids.insert(device.id()) {
            return Err("invalid or duplicate device id".into());
        }
        if !bus_ids.contains(device.bus()) {
            return Err("unknown device bus".into());
        }
        if !(0x08..=0x77).contains(&device.address())
            || addresses
                .insert((device.bus(), device.address()), device.id())
                .is_some()
        {
            return Err("invalid/conflicting I2C address".into());
        }
        let ConfigurationDevice::I2c {
            implementation,
            configuration,
            ..
        } = device;
        if !name(implementation) || configuration.len() > MAX_PROFILE_ITEMS {
            return Err("invalid implementation or excessive parameter count".into());
        }
        for (key, value) in configuration {
            let parameter_name = !key.is_empty()
                && key.len() <= MAX_PROFILE_ID_BYTES
                && key.bytes().enumerate().all(|(i, b)| {
                    b == b'_' || b.is_ascii_alphabetic() || (i != 0 && b.is_ascii_digit())
                });
            if !parameter_name
                || matches!(value, ConfigurationScalar::Text(text) if text.len() > MAX_PROFILE_ID_BYTES || text.chars().any(char::is_control))
            {
                return Err("invalid device parameter name/text".into());
            }
        }
    }
    Ok(())
}
pub(super) fn write(
    source: &mut String,
    buses: &[ConfigurationBus],
    devices: &[ConfigurationDevice],
) -> Result<(), serde_json::Error> {
    for ConfigurationBus::I2c {
        id,
        controller,
        sda,
        scl,
        frequency_hz,
    } in buses
    {
        writeln!(source,"  bus = {{kind: \"i2c\", id: {}, controller: {controller}, sda: {sda}, scl: {scl}, frequency_hz: {frequency_hz}}}",serde_json::to_string(id)?).unwrap();
    }
    for ConfigurationDevice::I2c {
        id,
        bus,
        address,
        implementation,
        configuration,
    } in devices
    {
        write!(
            source,
            "  device = {{kind: \"i2c\", id: {}, bus: {}, address: {address}, implementation: {}",
            serde_json::to_string(id)?,
            serde_json::to_string(bus)?,
            serde_json::to_string(implementation)?
        )
        .unwrap();
        if !configuration.is_empty() {
            source.push_str(", configuration: {");
            for (index, (key, value)) in configuration.iter().enumerate() {
                if index != 0 {
                    source.push_str(", ");
                }
                write!(source, "{}: {}", key, serde_json::to_string(value)?).unwrap();
            }
            source.push('}');
        }
        source.push_str("}\n");
    }
    Ok(())
}
