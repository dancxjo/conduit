//! Checked finite monotonic provider construction; no timer is initialized here.
use conduit_host_make::{
    BaseInitializationContract, CheckedHostConfiguration, ConfigurationValue,
    KindConfigurationField, KindConfigurationRule,
};
pub const BASE_KIND: &str = "clock/monotonic";
#[derive(Clone, Copy, Debug)]
pub struct CheckedClockConstruction {
    pub slots: usize,
    pub maximum_duration_ms: u64,
    /// Explicit lifetime deadline-call ceiling. Zero selects no deadline calls;
    /// it does not disable the independently bounded periodic wait provider.
    pub maximum_call_operations: u32,
}
pub fn initialization_contract() -> BaseInitializationContract {
    BaseInitializationContract {
        fields: vec![
            KindConfigurationField {
                key: "slots".into(),
                default_value: ConfigurationValue::U64(1),
                rule: KindConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: 16,
                },
            },
            KindConfigurationField {
                key: "maximum_duration_ms".into(),
                default_value: ConfigurationValue::U64(1),
                rule: KindConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: 86_400_000,
                },
            },
            KindConfigurationField {
                key: "maximum_call_operations".into(),
                // Contract defaults describe the type; initialization still
                // requires an explicit Source value, including disabled zero.
                default_value: ConfigurationValue::U64(0),
                rule: KindConfigurationRule::U64Range {
                    minimum: 0,
                    maximum: u64::from(u32::MAX),
                },
            },
        ],
    }
}
impl CheckedClockConstruction {
    /// Return only an explicitly selected finite deadline-call ceiling. This
    /// grants no authority: Root's original Issuer must authorize the operation.
    pub fn deadline_call_operations(&self) -> Result<u32, String> {
        if self.maximum_call_operations == 0 {
            return Err("deadline clock calls are explicitly unselected".into());
        }
        Ok(self.maximum_call_operations)
    }

    pub fn from_configuration(
        checked: &CheckedHostConfiguration,
        implementation: &str,
    ) -> Result<Self, String> {
        let mut bases = checked
            .configuration()
            .bases
            .iter()
            .filter(|b| b.kind == BASE_KIND);
        let base = bases.next().ok_or("missing monotonic clock construction")?;
        if bases.next().is_some()
            || !checked
                .resolved_bases()
                .iter()
                .any(|(kind, imp)| kind == BASE_KIND && imp == implementation)
        {
            return Err("ambiguous monotonic clock construction".into());
        }
        let values = base
            .initialization
            .as_ref()
            .ok_or("clock initialization required")?
            .checked_values(&initialization_contract())?;
        let value = |key: &str| match values.get(key) {
            Some(ConfigurationValue::U64(n)) => Ok(*n),
            _ => Err(format!("missing clock bound {key}")),
        };
        let slots = value("slots")?;
        if slots > u64::from(checked.configuration().limits.timer_slots) {
            return Err("clock slots exceed original Host timer limit".into());
        }
        Ok(Self {
            slots: usize::try_from(slots).map_err(|_| "clock slot width")?,
            maximum_duration_ms: value("maximum_duration_ms")?,
            maximum_call_operations: u32::try_from(value("maximum_call_operations")?)
                .map_err(|_| "clock operation ceiling exceeds U32")?,
        })
    }
}
