//! Shared checked construction for a finite local Face renderer. These ceilings
//! describe initialized support; they neither reserve memory nor authorize Play.
use conduit_host_make::{
    BaseInitializationContract, CheckedHostConfiguration, ConfigurationValue,
    KindConfigurationField, KindConfigurationRule,
};
pub const BASE_KIND: &str = "face/renderer";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemperatureUnit {
    Fahrenheit,
    Celsius,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckedRendererConstruction {
    pub face_bytes: u32,
    pub show_bytes: u32,
    pub resource_bytes: u64,
    pub maximum_frames: u32,
    pub page_period_ms: u64,
    pub show_lifetime_ms: u64,
    pub temperature_unit: TemperatureUnit,
}
pub fn initialization_contract() -> BaseInitializationContract {
    let count = |key: &str, minimum, maximum| KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::U64(minimum),
        rule: KindConfigurationRule::U64Range { minimum, maximum },
    };
    BaseInitializationContract {
        fields: vec![
            count("face_bytes", 1, 8192),
            count("show_bytes", 1, 8192),
            count("resource_bytes", 1, 128 * 1024),
            count("maximum_frames", 1, u64::from(u32::MAX)),
            count("page_period_ms", 1, 60_000),
            count("show_lifetime_ms", 1, 3_600_000),
            KindConfigurationField {
                key: "temperature_unit".into(),
                default_value: ConfigurationValue::Text("fahrenheit".into()),
                rule: KindConfigurationRule::TextOneOf {
                    values: vec!["fahrenheit".into(), "celsius".into()],
                },
            },
        ],
    }
}
impl CheckedRendererConstruction {
    pub fn from_configuration(
        checked: &CheckedHostConfiguration,
        implementation: &str,
    ) -> Result<Self, String> {
        let mut bases = checked
            .configuration()
            .bases
            .iter()
            .filter(|base| base.kind == BASE_KIND);
        let base = bases.next().ok_or("missing selected local Face renderer")?;
        if bases.next().is_some()
            || !checked
                .resolved_bases()
                .iter()
                .any(|(kind, selected)| kind == BASE_KIND && selected == implementation)
        {
            return Err("requires exactly one selected local Face renderer implementation".into());
        }
        let values = base
            .initialization
            .as_ref()
            .ok_or("missing renderer initialization")?
            .checked_values(&initialization_contract())?;
        let value = |key: &str| -> Result<u64, String> {
            match values.get(key) {
                Some(ConfigurationValue::U64(n)) => Ok(*n),
                _ => Err(format!("missing renderer bound {key}")),
            }
        };
        let construction = Self {
            face_bytes: value("face_bytes")? as u32,
            show_bytes: value("show_bytes")? as u32,
            resource_bytes: value("resource_bytes")?,
            maximum_frames: value("maximum_frames")? as u32,
            page_period_ms: value("page_period_ms")?,
            show_lifetime_ms: value("show_lifetime_ms")?,
            temperature_unit: match values.get("temperature_unit") {
                Some(ConfigurationValue::Text(unit)) if unit == "fahrenheit" => {
                    TemperatureUnit::Fahrenheit
                }
                Some(ConfigurationValue::Text(unit)) if unit == "celsius" => {
                    TemperatureUnit::Celsius
                }
                _ => return Err("missing or unsupported renderer temperature unit".into()),
            },
        };
        if construction.show_lifetime_ms < construction.page_period_ms {
            return Err("renderer Show lifetime cannot contain one page period".into());
        }
        if construction.resource_bytes
            < u64::from(construction.face_bytes) + u64::from(construction.show_bytes)
        {
            return Err("renderer storage ceiling cannot contain its selected wire buffers".into());
        }
        Ok(construction)
    }
}
