//! Shared finite initialization field contract for an explicitly selected Fore
//! carrier Base. Describing/checking these fields creates no carrier custody,
//! native initialization, Line availability, resource grant or runtime authority.
use crate::{
    BaseInitializationContract, ConfigurationValue, KindConfigurationField, KindConfigurationRule,
};
pub const BASE_KIND: &str = "line/fore-carrier";
pub fn initialization_contract() -> BaseInitializationContract {
    let count = |key: &str, maximum| KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::U64(1),
        rule: KindConfigurationRule::U64Range {
            minimum: 1,
            maximum,
        },
    };
    BaseInitializationContract {
        fields: vec![
            KindConfigurationField {
                key: "authorizer".into(),
                default_value: ConfigurationValue::Text("seed-owner".into()),
                rule: KindConfigurationRule::TextOneOf {
                    values: vec!["seed-owner".into()],
                },
            },
            count("maximum_lines", 16),
            count("maximum_frame_bytes", 4096),
            count("maximum_payload_bytes", 4096),
            count("maximum_buffered_bytes", 65536),
            count("maximum_in_flight_items", 1),
            count("maximum_frames", 1_000_000),
        ],
    }
}

/// Parsed finite intent, not a checked target, resolved implementation, initialized
/// Base, carrier possession, availability sign or resource grant. Callers must
/// independently check those facts and bind this declaration to installed code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeclaredForeCarrierBounds {
    pub maximum_lines: u8,
    pub maximum_frame_bytes: u32,
    pub maximum_payload_bytes: u32,
    pub maximum_buffered_bytes: usize,
    pub maximum_in_flight_items: u16,
    pub maximum_frames: u32,
}
impl DeclaredForeCarrierBounds {
    pub fn from_configuration(config: &crate::HostConfiguration) -> Result<Option<Self>, String> {
        let mut bases = config.bases.iter().filter(|b| b.kind == BASE_KIND);
        let Some(base) = bases.next() else {
            return Ok(None);
        };
        if bases.next().is_some() {
            return Err("requires exactly one declared Fore carrier Base".into());
        }
        if config
            .preparation
            .is_none_or(|p| p.authorizer != conduit_body::HostPreparationAuthorizer::SeedOwner)
            || config
                .execution
                .is_none_or(|p| p.authorizer != conduit_body::HostExecutionAuthorizer::SeedOwner)
        {
            return Err(
                "Fore carrier requires explicit SeedOwner preparation and execution policy".into(),
            );
        }
        let values = base
            .initialization
            .as_ref()
            .ok_or("Fore carrier initialization required")?
            .checked_values(&initialization_contract())?;
        let value = |key: &str| match values.get(key) {
            Some(ConfigurationValue::U64(n)) => Ok(*n),
            _ => Err(format!("missing Fore carrier bound {key}")),
        };
        let construction = Self {
            maximum_lines: value("maximum_lines")? as u8,
            maximum_frame_bytes: value("maximum_frame_bytes")? as u32,
            maximum_payload_bytes: value("maximum_payload_bytes")? as u32,
            maximum_buffered_bytes: usize::try_from(value("maximum_buffered_bytes")?)
                .map_err(|_| "Fore backing width")?,
            maximum_in_flight_items: value("maximum_in_flight_items")? as u16,
            maximum_frames: value("maximum_frames")? as u32,
        };
        if construction
            .maximum_payload_bytes
            .checked_add(52)
            .is_none_or(|n| n > construction.maximum_frame_bytes)
            || construction.maximum_frame_bytes as usize > construction.maximum_buffered_bytes
            || u64::from(construction.maximum_lines) > u64::from(config.limits.line_sessions)
            || construction.maximum_buffered_bytes as u64 > config.limits.buffered_bytes
            || construction.maximum_buffered_bytes as u64 > config.limits.heap_arena_bytes
        {
            return Err(
                "Fore carrier bounds exceed framing, backing or original Host limits".into(),
            );
        }
        Ok(Some(construction))
    }
}

#[cfg(test)]
mod tests;
