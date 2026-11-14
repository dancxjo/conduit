//! Checked physical construction requests. No initialization or runtime offers.
use conduit_host_make::{
    BaseInitializationContract, CheckedHostConfiguration, ConfigurationValue, ImplementationOffer,
    KindConfigurationField, KindConfigurationRule,
};

pub const FORE_CARRIER_IMPLEMENTATION: &str = "rp2040/root-usb-fore@1";
pub const I2C_BASE_KIND: &str = "bus/i2c";
pub const I2C_IMPLEMENTATION: &str = "rp2040/i2c@1";
pub const BOSCH_BASE_KIND: &str = "sensor/environment";
pub const BOSCH_IMPLEMENTATION: &str = "rp2040/bosch-environment@1";
pub const HT16K33_BASE_KIND: &str = "display/segmented";
pub const HT16K33_IMPLEMENTATION: &str = "rp2040/ht16k33@1";

pub const WIFI_BASE_KIND: &str = "network/wifi";
pub const WIFI_IMPLEMENTATION: &str = "rp2040/cyw43-retained@1";

pub fn retained_wifi_initialization_contract() -> BaseInitializationContract {
    BaseInitializationContract {
        fields: vec![
            KindConfigurationField {
                key: "credential_source".into(),
                default_value: ConfigurationValue::Text(String::new()),
                rule: KindConfigurationRule::TextOneOf {
                    values: vec!["private-flash-network-journal".into()],
                },
            },
            count("maximum_sockets", 4, 4),
        ],
    }
}

/// Absence selects no radio initialization or firmware assets. A present base
/// must still satisfy the exact retained-radio construction contract.
pub fn retained_wifi_selected(checked: &CheckedHostConfiguration) -> Result<bool, String> {
    if !checked
        .configuration()
        .bases
        .iter()
        .any(|base| base.kind == WIFI_BASE_KIND)
    {
        return Ok(false);
    }
    check_retained_wifi_construction(checked)?;
    Ok(true)
}

pub fn check_retained_wifi_construction(checked: &CheckedHostConfiguration) -> Result<(), String> {
    selected_values(
        checked,
        WIFI_BASE_KIND,
        WIFI_IMPLEMENTATION,
        &retained_wifi_initialization_contract(),
    )
    .map(|_| ())
}

pub(super) fn count(key: &str, minimum: u64, maximum: u64) -> KindConfigurationField {
    KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::U64(minimum),
        rule: KindConfigurationRule::U64Range { minimum, maximum },
    }
}

pub fn i2c_initialization_contract() -> BaseInitializationContract {
    BaseInitializationContract {
        fields: vec![
            count("controller", 0, 1),
            count("sda_gpio", 0, 28),
            count("scl_gpio", 0, 28),
            count("frequency_hz", 10_000, 400_000),
            count("maximum_transaction_polls", 1, u64::from(u32::MAX)),
            // Both zero leave native Body calls unselected. A declared range
            // constrains local attachment policy, never grants a Play access.
            count("minimum_call_address", 0, 119),
            count("maximum_call_address", 0, 119),
            // Zero leaves native issuance disabled. A positive ceiling bounds
            // each locally issued Play capability; it never proves readiness.
            count("maximum_call_operations", 0, u64::from(u32::MAX)),
        ],
    }
}

pub fn bosch_initialization_contract() -> BaseInitializationContract {
    BaseInitializationContract {
        fields: vec![count("controller", 0, 1), count("address", 0x76, 0x77)],
    }
}

pub fn ht16k33_initialization_contract() -> BaseInitializationContract {
    BaseInitializationContract {
        fields: vec![
            count("controller", 0, 1),
            count("address", 0x70, 0x77),
            count("brightness", 0, 15),
            KindConfigurationField {
                key: "layout".into(),
                default_value: ConfigurationValue::Text(String::new()),
                rule: KindConfigurationRule::TextOneOf {
                    values: vec!["adafruit-alpha4".into()],
                },
            },
        ],
    }
}

pub(super) fn offers() -> Vec<ImplementationOffer> {
    [
        (I2C_BASE_KIND, I2C_IMPLEMENTATION),
        (
            conduit_mcu_source_build::fore_carrier::BASE_KIND,
            FORE_CARRIER_IMPLEMENTATION,
        ),
        (BOSCH_BASE_KIND, BOSCH_IMPLEMENTATION),
        (HT16K33_BASE_KIND, HT16K33_IMPLEMENTATION),
        (WIFI_BASE_KIND, WIFI_IMPLEMENTATION),
        (crate::DHT11_BASE_KIND, crate::DHT11_IMPLEMENTATION),
        ("face/renderer", crate::RENDERER_IMPLEMENTATION),
        ("clock/monotonic", crate::CLOCK_IMPLEMENTATION),
        (
            conduit_mcu_source_build::recording::BASE_KIND,
            crate::RECORDING_IMPLEMENTATION,
        ),
        (
            conduit_mcu_source_build::recording::BASE_KIND,
            crate::RECORD_SERVICE_IMPLEMENTATION,
        ),
        (crate::TM1637_BASE_KIND, crate::TM1637_IMPLEMENTATION),
    ]
    .into_iter()
    .map(|(kind, implementation)| ImplementationOffer {
        base_kind: kind.into(),
        implementation_id: implementation.into(),
        implementation_revision: 1,
        target_patterns: vec![super::TARGET_ID.into()],
        prerequisites: Vec::new(),
        build_feature: None,
    })
    .collect()
}

pub(super) fn selected_values(
    checked: &CheckedHostConfiguration,
    kind: &str,
    implementation: &str,
    contract: &BaseInitializationContract,
) -> Result<std::collections::BTreeMap<String, ConfigurationValue>, String> {
    if checked.configuration().target.architecture != "thumbv6m"
        || checked.configuration().target.machine != "pico-w"
        || !checked
            .resolved_bases()
            .iter()
            .any(|(k, i)| k == kind && i == implementation)
    {
        return Err("physical construction requires exact selected RP2040 implementation".into());
    }
    let mut bases = checked
        .configuration()
        .bases
        .iter()
        .filter(|base| base.kind == kind);
    let base = bases.next().ok_or("physical Base is absent")?;
    if bases.next().is_some() {
        return Err("physical Base selection is ambiguous".into());
    }
    base.initialization
        .as_ref()
        .ok_or("explicit physical initialization is required")?
        .checked_values(contract)
}

pub(super) fn value(
    values: &std::collections::BTreeMap<String, ConfigurationValue>,
    key: &str,
) -> Result<u64, String> {
    match values.get(key) {
        Some(ConfigurationValue::U64(value)) => Ok(*value),
        _ => Err("physical field must be an unsigned integer".into()),
    }
}

/// Source-bound bus ownership, never proof that the bus was initialized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedI2cConstruction {
    pub configuration_id: String,
    pub controller: u8,
    pub sda_gpio: u8,
    pub scl_gpio: u8,
    pub frequency_hz: u32,
    pub maximum_transaction_polls: u32,
    pub call_address_range: Option<(u8, u8)>,
    pub maximum_call_operations: u32,
}
impl CheckedI2cConstruction {
    pub fn from_configuration(checked: &CheckedHostConfiguration) -> Result<Self, String> {
        let values = selected_values(
            checked,
            I2C_BASE_KIND,
            I2C_IMPLEMENTATION,
            &i2c_initialization_contract(),
        )?;
        let controller = value(&values, "controller")? as u8;
        let sda_gpio = value(&values, "sda_gpio")? as u8;
        let scl_gpio = value(&values, "scl_gpio")? as u8;
        let exposed = |pin| pin <= 22 || (26..=28).contains(&pin);
        if !exposed(sda_gpio)
            || !exposed(scl_gpio)
            || scl_gpio != sda_gpio + 1
            || sda_gpio % 4 != controller * 2
        {
            return Err("RP2040 I2C pin/controller assignment is invalid or reserved".into());
        }
        let minimum = value(&values, "minimum_call_address")? as u8;
        let maximum = value(&values, "maximum_call_address")? as u8;
        let call_address_range = match (minimum, maximum) {
            (0, 0) => None,
            (8..=119, 8..=119) if minimum <= maximum => Some((minimum, maximum)),
            _ => {
                return Err(
                    "native I2C call range must be absent or an ordered 7-bit interval".into(),
                )
            }
        };
        let maximum_call_operations = value(&values, "maximum_call_operations")? as u32;
        if maximum_call_operations != 0 && call_address_range.is_none() {
            return Err("native I2C issuance requires an explicit local address range".into());
        }
        Ok(Self {
            configuration_id: checked.configuration_id().into(),
            controller,
            sda_gpio,
            scl_gpio,
            frequency_hz: value(&values, "frequency_hz")? as u32,
            maximum_transaction_polls: value(&values, "maximum_transaction_polls")? as u32,
            call_address_range,
            maximum_call_operations,
        })
    }
}

/// Exact two-device bus construction used by the existing Bosch/HT drivers.
/// This neither advertises a computation Back nor calls the physical devices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedBoschHt16k33Construction {
    pub bus: CheckedI2cConstruction,
    pub sensor_address: u8,
    pub display_address: u8,
    pub brightness: u8,
}
impl CheckedBoschHt16k33Construction {
    pub fn from_configuration(checked: &CheckedHostConfiguration) -> Result<Self, String> {
        let bus = CheckedI2cConstruction::from_configuration(checked)?;
        let sensor = selected_values(
            checked,
            BOSCH_BASE_KIND,
            BOSCH_IMPLEMENTATION,
            &bosch_initialization_contract(),
        )?;
        let display = selected_values(
            checked,
            HT16K33_BASE_KIND,
            HT16K33_IMPLEMENTATION,
            &ht16k33_initialization_contract(),
        )?;
        let sensor_address = value(&sensor, "address")? as u8;
        let display_address = value(&display, "address")? as u8;
        if value(&sensor, "controller")? != u64::from(bus.controller)
            || value(&display, "controller")? != u64::from(bus.controller)
            || sensor_address == display_address
        {
            return Err("device bus mismatch or I2C address collision".into());
        }
        Ok(Self {
            bus,
            sensor_address,
            display_address,
            brightness: value(&display, "brightness")? as u8,
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

    fn prepare(source: &str) -> Result<CheckedBoschHt16k33Construction, String> {
        let packages =
            MakePackageSet::compose(&[&crate::Rp2040MakePackage]).map_err(|e| format!("{e:?}"))?;
        let catalog = MakeCatalog::canonical().with_packages(&packages);
        let source = parse_host_configuration_conduit(source).map_err(|e| format!("{e:?}"))?;
        let checked =
            check_host_configuration(source, &catalog, &packages).map_err(|e| format!("{e:?}"))?;
        CheckedBoschHt16k33Construction::from_configuration(&checked)
    }

    #[test]
    fn current_base_source_preserves_exact_green_hardware_selection() {
        let selected = prepare(GREEN).unwrap();
        assert_eq!(
            (
                selected.bus.controller,
                selected.bus.sda_gpio,
                selected.bus.scl_gpio
            ),
            (1, 6, 7)
        );
        assert_eq!(selected.bus.frequency_hz, 100_000);
        assert_eq!(selected.bus.maximum_transaction_polls, 100_000);
        assert_eq!(
            (
                selected.sensor_address,
                selected.display_address,
                selected.brightness
            ),
            (118, 112, 3)
        );
        assert!(!selected.bus.configuration_id.is_empty());
    }

    #[test]
    fn transaction_work_bound_is_explicit_nonzero_and_source_bound() {
        for invalid in ["0", "4294967296", "-1", "\"unbounded\""] {
            assert!(prepare(&GREEN.replace(
                "maximum_transaction_polls: 100000",
                &format!("maximum_transaction_polls: {invalid}")
            ))
            .is_err());
        }
        assert!(prepare(&GREEN.replace(", maximum_transaction_polls: 100000", "")).is_err());
        let original = prepare(GREEN).unwrap();
        let changed = prepare(&GREEN.replace(
            "maximum_transaction_polls: 100000",
            "maximum_transaction_polls: 200000",
        ))
        .unwrap();
        assert_eq!(changed.bus.maximum_transaction_polls, 200_000);
        assert_ne!(original.bus.configuration_id, changed.bus.configuration_id);
    }

    #[test]
    fn wrong_controller_reserved_gpio_and_device_collision_refuse() {
        assert!(prepare(&GREEN.replace("sda_gpio: 6", "sda_gpio: 5")).is_err());
        assert!(
            prepare(&GREEN.replace("sda_gpio: 6, scl_gpio: 7", "sda_gpio: 24, scl_gpio: 25"))
                .is_err()
        );
        assert!(prepare(
            &GREEN.replace("controller: 1, address: 118", "controller: 0, address: 118")
        )
        .is_err());
        assert!(prepare(&GREEN.replace("address: 112", "address: 118")).is_err());
        assert!(prepare(&GREEN.replace("brightness: 3", "brightness: 16")).is_err());
        assert!(prepare(&GREEN.replace("adafruit-alpha4", "unknown-layout")).is_err());
    }
}
