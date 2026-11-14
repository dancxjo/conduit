//! Checked finite NOR residence construction, not RAM or writable authority.
use conduit_host_make::{
    BaseInitializationContract, CheckedHostConfiguration, ConfigurationValue,
    KindConfigurationField, KindConfigurationRule,
};
pub const BASE_KIND: &str = "resource/nor-record-residence";
pub const PROGRAM_BYTES: u32 = 256;
pub const RECORD_SLOT_BYTES: u32 = 512;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckedRecordingConstruction {
    pub prefix_bytes: u32,
    pub slots: u32,
}
pub fn initialization_contract() -> BaseInitializationContract {
    BaseInitializationContract {
        fields: vec![
            KindConfigurationField {
                key: "prefix_bytes".into(),
                default_value: ConfigurationValue::U64(8192),
                rule: KindConfigurationRule::U64Range {
                    minimum: u64::from(PROGRAM_BYTES),
                    maximum: u64::from(u32::MAX),
                },
            },
            KindConfigurationField {
                key: "slots".into(),
                default_value: ConfigurationValue::U64(1),
                rule: KindConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: u64::from(u32::MAX / RECORD_SLOT_BYTES),
                },
            },
        ],
    }
}
impl CheckedRecordingConstruction {
    pub fn from_configuration(
        checked: &CheckedHostConfiguration,
        implementation: &str,
    ) -> Result<Self, String> {
        let values = checked_values(checked, implementation, &initialization_contract())?;
        Self::from_values(&values)
    }
    fn from_values(
        values: &std::collections::BTreeMap<String, ConfigurationValue>,
    ) -> Result<Self, String> {
        let value = |key: &str| match values.get(key) {
            Some(ConfigurationValue::U64(n)) => {
                u32::try_from(*n).map_err(|_| format!("recording bound width {key}"))
            }
            _ => Err(format!("missing recording bound {key}")),
        };
        let prefix_bytes = value("prefix_bytes")?;
        let slots = value("slots")?;
        if prefix_bytes == 0 || !prefix_bytes.is_multiple_of(PROGRAM_BYTES) || slots == 0 {
            return Err("unaligned or empty NOR recording construction".into());
        }
        let construction = Self {
            prefix_bytes,
            slots,
        };
        construction.end_bytes()?;
        Ok(construction)
    }
    /// Exclusive end relative to actual DURABLE; the board must additionally
    /// compare this against its checked physical partition before ownership.
    pub fn end_bytes(self) -> Result<u32, String> {
        self.slots
            .checked_mul(RECORD_SLOT_BYTES)
            .and_then(|bytes| self.prefix_bytes.checked_add(bytes))
            .ok_or_else(|| "NOR recording construction extent overflows".into())
    }
}

/// Supported preparation bounds for one service over the same finite residence.
/// This construction grants no read capability, resource capacity or execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckedRecordServiceConstruction {
    pub residence: CheckedRecordingConstruction,
    pub maximum_read_operations: u32,
}
/// The declared default is one; ordinary checked Host initialization still
/// requires an explicit read_operations field, as it does for residence fields.
pub fn service_initialization_contract() -> BaseInitializationContract {
    let mut contract = initialization_contract();
    contract.fields.push(KindConfigurationField {
        key: "read_operations".into(),
        default_value: ConfigurationValue::U64(1),
        rule: KindConfigurationRule::U64Range {
            minimum: 1,
            maximum: u64::from(u32::MAX),
        },
    });
    contract
}
impl CheckedRecordServiceConstruction {
    pub fn from_configuration(
        checked: &CheckedHostConfiguration,
        implementation: &str,
    ) -> Result<Self, String> {
        let values = checked_values(checked, implementation, &service_initialization_contract())?;
        let maximum_read_operations = match values.get("read_operations") {
            Some(ConfigurationValue::U64(value)) => {
                u32::try_from(*value).map_err(|_| "recording bound width read_operations")?
            }
            _ => return Err("missing recording bound read_operations".into()),
        };
        Ok(Self {
            residence: CheckedRecordingConstruction::from_values(&values)?,
            maximum_read_operations,
        })
    }
}
fn checked_values(
    checked: &CheckedHostConfiguration,
    implementation: &str,
    contract: &BaseInitializationContract,
) -> Result<std::collections::BTreeMap<String, ConfigurationValue>, String> {
    let mut bases = checked
        .configuration()
        .bases
        .iter()
        .filter(|base| base.kind == BASE_KIND);
    let base = bases.next().ok_or("missing NOR recording construction")?;
    if bases.next().is_some()
        || !checked
            .resolved_bases()
            .iter()
            .any(|(kind, imp)| kind == BASE_KIND && imp == implementation)
    {
        return Err("ambiguous or different NOR recording implementation".into());
    }
    base.initialization
        .as_ref()
        .ok_or("recording initialization required")?
        .checked_values(contract)
}
