//! Checked recording selectors and reported context; no IO or authority issuance.
use conduit_core::BoundedResourceRef;
use conduit_host_make::CheckedHostConfiguration;
use conduit_std_offers::{
    ENVIRONMENT_RECORDING_HISTORY_BASE, ENVIRONMENT_RECORDING_HISTORY_IMPLEMENTATION,
};
use std::path::Path;

pub struct CheckedEnvironmentRecordingHistory<'a> {
    pub source_pool: &'a str,
    pub memory_pool: &'a str,
    /// None means independently retained selections must be supplied to the
    /// unsafe external-binding initializer; it grants no implicit file access.
    pub directory: Option<&'a str>,
    pub reference: Option<BoundedResourceRef>,
    pub producer: [&'a str; 4],
    /// Temperature, relative humidity; source identity/name, channel identity/name.
    pub channels: [[&'a str; 4]; 2],
}
impl<'a> CheckedEnvironmentRecordingHistory<'a> {
    pub fn from_configuration(checked: &'a CheckedHostConfiguration) -> Result<Self, String> {
        let mut bases = checked
            .configuration()
            .bases
            .iter()
            .filter(|base| base.kind == ENVIRONMENT_RECORDING_HISTORY_BASE);
        let base = bases
            .next()
            .ok_or("environment recording Source Base is missing")?;
        if bases.next().is_some()
            || base.implementation.as_deref() != Some(ENVIRONMENT_RECORDING_HISTORY_IMPLEMENTATION)
            || !checked
                .resolved_bases()
                .iter()
                .any(|(kind, implementation)| {
                    kind == ENVIRONMENT_RECORDING_HISTORY_BASE
                        && implementation == ENVIRONMENT_RECORDING_HISTORY_IMPLEMENTATION
                })
        {
            return Err("environment recording Source Base is ambiguous or unsupported".into());
        }
        let values = &base
            .initialization
            .as_ref()
            .ok_or("environment recording Source initialization is missing")?
            .values;
        const KEYS: [&str; 15] = [
            "source_pool",
            "memory_pool",
            "capture_access",
            "directory",
            "reference",
            "producer_host",
            "producer_boot",
            "producer_plan",
            "producer_play",
            "temperature_source",
            "temperature_channel",
            "temperature_name",
            "humidity_source",
            "humidity_channel",
            "humidity_name",
        ];
        if values.len() != KEYS.len() || values.keys().any(|key| !KEYS.contains(&key.as_str())) {
            return Err(
                "environment recording Source requires its sixteen exact initialization fields"
                    .into(),
            );
        }
        let text = |key: &str, maximum: usize| {
            values
                .get(key)
                .and_then(serde_json::Value::as_str)
                .filter(|value| value.len() <= maximum && !value.chars().any(char::is_control))
                .ok_or_else(|| format!("environment recording Source requires bounded text {key}"))
        };
        let source_pool = text("source_pool", 128)?;
        let memory_pool = text("memory_pool", 128)?;
        if !conduit_core::valid_data_label(source_pool)
            || !conduit_core::valid_data_label(memory_pool)
            || source_pool == memory_pool
        {
            return Err("environment recording Source requires distinct exact named pools".into());
        }
        if text("capture_access", 32)? != "operator-admitted" {
            return Err(
                "environment recording Source requires explicit operator read intent".into(),
            );
        }
        let nonempty = |key: &str| {
            text(key, 128).and_then(|value| {
                if value.is_empty() {
                    Err(format!(
                        "environment recording Source requires nonempty {key}"
                    ))
                } else {
                    Ok(value)
                }
            })
        };
        let producer = [
            nonempty("producer_host")?,
            nonempty("producer_boot")?,
            nonempty("producer_plan")?,
            nonempty("producer_play")?,
        ];
        let channels = [
            [
                nonempty("temperature_source")?,
                nonempty("temperature_name")?,
                nonempty("temperature_channel")?,
                nonempty("temperature_name")?,
            ],
            [
                nonempty("humidity_source")?,
                nonempty("humidity_name")?,
                nonempty("humidity_channel")?,
                nonempty("humidity_name")?,
            ],
        ];
        let identities = [
            channels[0][0],
            channels[0][2],
            channels[1][0],
            channels[1][2],
        ];
        for (index, identity) in identities.iter().enumerate() {
            if identities[..index].contains(identity) {
                return Err(
                    "environment recording Source/channel identities must be distinct".into(),
                );
            }
        }
        let directory = text("directory", 1024)?;
        let selector = text("reference", 1024)?;
        let (directory, reference) = if directory.is_empty() && selector.is_empty() {
            (None, None)
        } else {
            if !Path::new(directory).is_absolute() || selector.is_empty() {
                return Err("environment recording selectors require an absolute directory and canonical reference".into());
            }
            let bytes = conduit_data::bytes_literal::decode_hex(selector)
                .map_err(|why| format!("environment recording reference hex: {why:?}"))?;
            let reference = BoundedResourceRef::decode(&bytes)
                .map_err(|why| format!("environment recording reference: {why:?}"))?;
            if reference
                .encode()
                .map_err(|why| format!("environment recording canonical reference: {why:?}"))?
                != bytes
                || reference.content_profile.as_str() != "text/json-utf8@1"
                || reference.access_class.as_str() != "resource/snapshot@1"
                || !(1..=4096).contains(&reference.extent.bytes)
                || reference.extent.items != Some(1)
                || reference.lifetime.expires_at.is_some()
            {
                return Err(
                    "environment recording reference differs from the exact immutable JSON profile"
                        .into(),
                );
            }
            (Some(directory), Some(reference))
        };
        Ok(Self {
            source_pool,
            memory_pool,
            directory,
            reference,
            producer,
            channels,
        })
    }
}

#[cfg(test)]
#[path = "configuration_tests.rs"]
mod tests;
