//! Explicit construction intent for borrowing the original Root serial wrapper.
//! No IO, native custody, runtime authority or available Line is created here.
pub use conduit_host_make::fore_carrier::{initialization_contract, BASE_KIND};
use conduit_host_make::{CheckedHostConfiguration, ConfigurationValue};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckedForeCarrierConstruction {
    pub maximum_lines: u8,
    pub maximum_frame_bytes: u32,
    pub maximum_payload_bytes: u32,
    pub maximum_buffered_bytes: usize,
    pub maximum_in_flight_items: u16,
    pub maximum_frames: u32,
}

impl CheckedForeCarrierConstruction {
    pub fn from_configuration(
        checked: &CheckedHostConfiguration,
        implementation: &str,
    ) -> Result<Option<Self>, String> {
        let config = checked.configuration();
        let mut bases = config.bases.iter().filter(|b| b.kind == BASE_KIND);
        let Some(base) = bases.next() else {
            return Ok(None);
        };
        let mut resolved = checked
            .resolved_bases()
            .iter()
            .filter(|(kind, _)| kind == BASE_KIND);
        if bases.next().is_some()
            || resolved.next().is_none_or(|(_, imp)| imp != implementation)
            || resolved.next().is_some()
        {
            return Err("requires exactly one selected Fore carrier implementation".into());
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
    /// Text for the generated config, not a native installation or ticket.
    pub fn rust_bounds(self) -> String {
        format!("conduit_mcu_host_runtime::root::ForeCarrierBounds{{maximum_lines:{},maximum_frame_bytes:{},maximum_payload_bytes:{},maximum_buffered_bytes:{},maximum_in_flight_items:{},maximum_frames:{}}}", self.maximum_lines,self.maximum_frame_bytes,self.maximum_payload_bytes,self.maximum_buffered_bytes,self.maximum_in_flight_items,self.maximum_frames)
    }
}
#[cfg(test)]
mod tests;
