//! Finite initialization intent, bound to the checked Source rather than copied
//! into IMAGE. These values do not observe a device or grant display custody.
use conduit_host_make::{
    BaseInitializationContract, CheckedHostConfiguration, ConfigurationValue,
    KindConfigurationField, KindConfigurationRule,
};

pub(crate) fn contract() -> BaseInitializationContract {
    BaseInitializationContract {
        fields: vec![
            KindConfigurationField {
                key: "backend".into(),
                default_value: ConfigurationValue::Text("KMSDRM".into()),
                rule: KindConfigurationRule::TextOneOf {
                    values: vec!["KMSDRM".into()],
                },
            },
            dimension("width"),
            dimension("height"),
        ],
    }
}
fn dimension(key: &str) -> KindConfigurationField {
    KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::U64(1),
        rule: KindConfigurationRule::U64Range {
            minimum: 1,
            maximum: 4096,
        },
    }
}

/// A package-checked direct Linux display request. Construction is private so
/// a runtime can compare its Source identity with the selected IMAGE before
/// consuming an actual initialized display claim.
#[derive(Clone)]
pub struct CheckedScanoutInitialization {
    configuration_id: String,
    width: u32,
    height: u32,
    surface_bytes: u64,
}
impl CheckedScanoutInitialization {
    pub fn from_configuration(checked: &CheckedHostConfiguration) -> Result<Self, String> {
        if !checked.profile().target.key().starts_with("std/") {
            return Err("KMSDRM initialization requires a Linux hosted target".into());
        }
        let mut selected = checked
            .configuration()
            .bases
            .iter()
            .filter(|base| base.kind == "display/scanout");
        let base = selected.next().ok_or("no selected scanout Base")?;
        if selected.next().is_some()
            || !checked.resolved_bases().iter().any(|(kind, driver)| {
                kind == "display/scanout" && driver == "display/linear-framebuffer@1"
            })
        {
            return Err("scanout initialization requires one exact linear-framebuffer Base".into());
        }
        let initialization = base
            .initialization
            .as_ref()
            .ok_or("scanout requires explicit backend and geometry")?;
        let values = initialization.checked_values(&contract())?;
        let value = |key: &str| match values.get(key) {
            Some(ConfigurationValue::U64(value)) => {
                u32::try_from(*value).map_err(|_| "scanout dimension overflow".to_string())
            }
            _ => Err("scanout dimension has wrong type".into()),
        };
        let width = value("width")?;
        let height = value("height")?;
        let surface_bytes = u64::from(width) * u64::from(height) * 4;
        // The existing portable CPU raster has this exact preparation ceiling.
        if surface_bytes > 8 * 1024 * 1024 {
            return Err("scanout geometry exceeds finite raster capacity".into());
        }
        Ok(Self {
            configuration_id: checked.configuration_id().into(),
            width,
            height,
            surface_bytes,
        })
    }
    pub fn configuration_id(&self) -> &str {
        &self.configuration_id
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn surface_bytes(&self) -> u64 {
        self.surface_bytes
    }
}
