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
    #[serde(rename = "serial")]
    Serial {
        id: String,
        path: String,
        implementation: String,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        configuration: BTreeMap<String, ConfigurationScalar>,
    },
    #[serde(rename = "network")]
    Network {
        id: String,
        authority: String,
        endpoint: String,
        implementation: String,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        configuration: BTreeMap<String, ConfigurationScalar>,
    },
    #[serde(rename = "gpio")]
    Gpio {
        id: String,
        implementation: String,
        pins: BTreeMap<String, u8>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        configuration: BTreeMap<String, ConfigurationScalar>,
    },
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
            Self::I2c { id, .. }
            | Self::Gpio { id, .. }
            | Self::Serial { id, .. }
            | Self::Network { id, .. } => id,
        }
    }
    pub fn bus(&self) -> Option<&str> {
        match self {
            Self::I2c { bus, .. } => Some(bus),
            Self::Gpio { .. } | Self::Serial { .. } | Self::Network { .. } => None,
        }
    }
    pub fn address(&self) -> Option<u8> {
        match self {
            Self::I2c { address, .. } => Some(*address),
            Self::Gpio { .. } | Self::Serial { .. } | Self::Network { .. } => None,
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
    let mut serial_paths = BTreeSet::new();
    let mut network_endpoints = BTreeSet::new();
    for device in devices {
        if !name(device.id()) || !ids.insert(device.id()) {
            return Err("invalid or duplicate device id".into());
        }
        let (implementation, configuration) = match device {
            ConfigurationDevice::Serial {
                path,
                implementation,
                configuration,
                ..
            } => {
                if !name(path) || !serial_paths.insert(path.as_str()) {
                    return Err("invalid or duplicate serial path".into());
                }
                (implementation, configuration)
            }
            ConfigurationDevice::Network {
                authority,
                endpoint,
                implementation,
                configuration,
                ..
            } => {
                if !name(authority)
                    || !name(endpoint)
                    || !network_endpoints.insert(endpoint.as_str())
                {
                    return Err("invalid authority or invalid/duplicate network endpoint".into());
                }
                (implementation, configuration)
            }

            ConfigurationDevice::I2c {
                bus,
                address,
                implementation,
                configuration,
                ..
            } => {
                if !bus_ids.contains(bus.as_str()) {
                    return Err("unknown device bus".into());
                }
                if !(0x08..=0x77).contains(address)
                    || addresses
                        .insert((bus.as_str(), *address), device.id())
                        .is_some()
                {
                    return Err("invalid/conflicting I2C address".into());
                }
                (implementation, configuration)
            }
            ConfigurationDevice::Gpio {
                implementation,
                pins: roles,
                configuration,
                ..
            } => {
                if roles.is_empty() || roles.len() > MAX_PROFILE_ITEMS {
                    return Err("invalid GPIO role count".into());
                }
                for (role, pin) in roles {
                    if !parameter_name(role) || !pins.insert(*pin) {
                        return Err("invalid GPIO role or conflicting attachment pin".into());
                    }
                }
                (implementation, configuration)
            }
        };
        if !name(implementation) || configuration.len() > MAX_PROFILE_ITEMS {
            return Err("invalid implementation or excessive parameter count".into());
        }
        for (key, value) in configuration {
            if !parameter_name(key)
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
    for device in devices {
        match device {
            ConfigurationDevice::Serial {
                id,
                path,
                implementation,
                configuration,
            } => {
                write!(
                    source,
                    "  device = {{kind: \"serial\", id: {}, path: {}, implementation: {}",
                    serde_json::to_string(id)?,
                    serde_json::to_string(path)?,
                    serde_json::to_string(implementation)?
                )
                .unwrap();
                write_configuration(source, configuration)?;
                source.push_str("}\n");
                continue;
            }
            ConfigurationDevice::Network {
                id,
                authority,
                endpoint,
                implementation,
                configuration,
            } => {
                write!(source, "  device = {{kind: \"network\", id: {}, authority: {}, endpoint: {}, implementation: {}", serde_json::to_string(id)?, serde_json::to_string(authority)?, serde_json::to_string(endpoint)?, serde_json::to_string(implementation)?).unwrap();
                write_configuration(source, configuration)?;
                source.push_str("}\n");
                continue;
            }
            _ => {}
        }
        let ConfigurationDevice::I2c {
            id,
            bus,
            address,
            implementation,
            configuration,
        } = device
        else {
            let ConfigurationDevice::Gpio {
                id,
                implementation,
                pins,
                configuration,
            } = device
            else {
                unreachable!()
            };
            write!(
                source,
                "  device = {{kind: \"gpio\", id: {}, implementation: {}, pins: {{",
                serde_json::to_string(id)?,
                serde_json::to_string(implementation)?
            )
            .unwrap();
            for (index, (role, pin)) in pins.iter().enumerate() {
                if index != 0 {
                    source.push_str(", ");
                }
                write!(source, "{role}: {pin}").unwrap();
            }
            source.push('}');
            write_configuration(source, configuration)?;
            source.push_str("}\n");
            continue;
        };
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

fn parameter_name(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= crate::MAX_PROFILE_ID_BYTES
        && key
            .bytes()
            .enumerate()
            .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || (i != 0 && b.is_ascii_digit()))
}
fn write_configuration(
    source: &mut String,
    configuration: &BTreeMap<String, ConfigurationScalar>,
) -> Result<(), serde_json::Error> {
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
    Ok(())
}

#[cfg(test)]
mod gpio_tests {
    use super::*;
    fn device(id: &str, role: &str, pin: u8) -> ConfigurationDevice {
        ConfigurationDevice::Gpio {
            id: id.into(),
            implementation: "generic/driver@1".into(),
            pins: [(role.into(), pin)].into(),
            configuration: Default::default(),
        }
    }
    #[test]
    fn generic_gpio_roles_roundtrip_and_conflict_with_every_other_attachment() {
        let devices = [device("sensor", "data", 3), device("display", "clk", 7)];
        validate(&[], &devices).unwrap();
        let bytes = serde_json::to_vec(&devices).unwrap();
        assert_eq!(
            serde_json::from_slice::<Vec<ConfigurationDevice>>(&bytes).unwrap(),
            devices
        );
        let mut source = String::new();
        write(&mut source, &[], &devices).unwrap();
        assert!(source.contains("kind: \"gpio\""));
        assert!(source.contains("pins: {data: 3}"));
        assert!(validate(&[], &[device("one", "data", 3), device("two", "clk", 3)]).is_err());
        assert!(validate(
            &[ConfigurationBus::I2c {
                id: "bus".into(),
                controller: 0,
                sda: 2,
                scl: 3,
                frequency_hz: 100_000
            }],
            &[device("sensor", "data", 3)]
        )
        .is_err());
        assert!(validate(&[], &[device("one", "bad-role", 3)]).is_err());
        let mut duplicate_roles = device("one", "data", 3);
        let ConfigurationDevice::Gpio { pins, .. } = &mut duplicate_roles else {
            unreachable!()
        };
        pins.insert("other".into(), 3);
        assert!(validate(&[], &[duplicate_roles]).is_err());
    }
}

#[cfg(test)]
mod native_tests {
    use super::*;
    use crate::{
        canonical_host_configuration_conduit, check_host_configuration,
        parse_host_configuration_conduit, MakeCatalog,
    };
    const SOURCE: &str = r#"host native-fixture {
  schema = 1
  target = {architecture: "x86_64", machine: "computer", os: "linux"}
  base = {kind: "storage/protected-file", implementation: "hosted/protected-file@1"}
  device = {kind: "serial", id: "usb", path: "/dev/serial/by-id/example", implementation: "fixture/serial@1", configuration: {baud: 115200, enabled: true, label: "room sensor"}}
  device = {kind: "network", id: "remote", authority: "sensor.local:8767", endpoint: "192.0.2.1:8767", implementation: "fixture/network@1", configuration: {timeout_ms: 2000}}
  limits = {static_memory_bytes: 1048576, heap_arena_bytes: 1048576, queue_items: 64, buffered_bytes: 65536, active_instances: 8, operation_slots: 8, timer_slots: 8, line_sessions: 2, evidence_items: 64}
}
"#;
    #[test]
    fn native_construction_roundtrips_canonical_source_and_checked_identity() {
        let packages = crate::test_packages::test_package_set();
        let catalog = MakeCatalog::canonical().with_packages(&packages);
        let original = check_host_configuration(
            parse_host_configuration_conduit(SOURCE).unwrap(),
            &catalog,
            &packages,
        )
        .unwrap();
        let encoded = canonical_host_configuration_conduit(original.configuration()).unwrap();
        let recovered = check_host_configuration(
            parse_host_configuration_conduit(&encoded).unwrap(),
            &catalog,
            &packages,
        )
        .unwrap();
        let mut expected = original.configuration().clone();
        expected.devices.sort_by(|a, b| a.id().cmp(b.id()));
        assert_eq!(&expected, recovered.configuration());
        assert_eq!(original.configuration_id(), recovered.configuration_id());
        assert_eq!(
            encoded,
            canonical_host_configuration_conduit(recovered.configuration()).unwrap()
        );
        assert_eq!(original.configuration().devices.len(), 2);
        for device in &original.configuration().devices {
            assert_eq!(device.bus(), None);
            assert_eq!(device.address(), None);
        }
    }
    #[test]
    fn native_construction_rejects_duplicate_ids_paths_endpoints_and_unbounded_fields() {
        let devices = parse_host_configuration_conduit(SOURCE).unwrap().devices;
        validate(&[], &devices).unwrap();
        for index in 0..2 {
            let mut duplicate = devices[index].clone();
            match &mut duplicate {
                ConfigurationDevice::Serial { id, .. }
                | ConfigurationDevice::Network { id, .. } => *id = "distinct".into(),
                _ => unreachable!(),
            }
            assert!(validate(&[], &[devices[index].clone(), duplicate]).is_err());
        }
        let mut repeated_id = devices.clone();
        let ConfigurationDevice::Network { id, .. } = &mut repeated_id[1] else {
            unreachable!()
        };
        *id = "usb".into();
        assert!(validate(&[], &repeated_id).is_err());
        for field in [
            "path",
            "authority",
            "endpoint",
            "implementation",
            "parameter",
            "count",
        ] {
            let mut invalid = devices.clone();
            match field {
                "path" => {
                    let ConfigurationDevice::Serial { path, .. } = &mut invalid[0] else {
                        unreachable!()
                    };
                    *path = "x".repeat(crate::MAX_PROFILE_ID_BYTES + 1);
                }
                "authority" | "endpoint" => {
                    let ConfigurationDevice::Network {
                        authority,
                        endpoint,
                        ..
                    } = &mut invalid[1]
                    else {
                        unreachable!()
                    };
                    if field == "authority" {
                        *authority = "".into();
                    } else {
                        *endpoint = "bad\nendpoint".into();
                    }
                }
                "implementation" => {
                    let ConfigurationDevice::Serial { implementation, .. } = &mut invalid[0] else {
                        unreachable!()
                    };
                    *implementation = "".into();
                }
                _ => {
                    let ConfigurationDevice::Serial { configuration, .. } = &mut invalid[0] else {
                        unreachable!()
                    };
                    configuration.clear();
                    if field == "parameter" {
                        configuration.insert("bad-key".into(), ConfigurationScalar::Boolean(true));
                    } else {
                        for index in 0..=crate::MAX_PROFILE_ITEMS {
                            configuration
                                .insert(format!("key{index}"), ConfigurationScalar::Boolean(true));
                        }
                    }
                }
            }
            assert!(validate(&[], &invalid).is_err(), "{field}");
        }
        assert!(serde_json::from_str::<ConfigurationDevice>(r#"{"kind":"serial","id":"a","path":"/dev/a","implementation":"x","configuration":{"nested":{"x":1}}}"#).is_err());
    }
}
