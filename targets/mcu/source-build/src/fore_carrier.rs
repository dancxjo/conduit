//! Explicit checked construction for borrowing the original Root serial wrapper.
//! No IO, native custody, runtime authority or available Line is created here.
pub use conduit_host_make::fore_carrier::{initialization_contract, BASE_KIND};
use conduit_host_make::{fore_carrier::DeclaredForeCarrierBounds, CheckedHostConfiguration};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckedForeCarrierConstruction(DeclaredForeCarrierBounds);
impl core::ops::Deref for CheckedForeCarrierConstruction {
    type Target = DeclaredForeCarrierBounds;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl CheckedForeCarrierConstruction {
    pub fn from_configuration(
        checked: &CheckedHostConfiguration,
        implementation: &str,
    ) -> Result<Option<Self>, String> {
        let config = checked.configuration();
        if !config.bases.iter().any(|b| b.kind == BASE_KIND) {
            return Ok(None);
        }
        let mut resolved = checked
            .resolved_bases()
            .iter()
            .filter(|(kind, _)| kind == BASE_KIND);
        if resolved.next().is_none_or(|(_, imp)| imp != implementation) || resolved.next().is_some()
        {
            return Err("requires exactly one selected Fore carrier implementation".into());
        }
        Ok(DeclaredForeCarrierBounds::from_configuration(config)?.map(Self))
    }
    /// Text for the generated config, not a native installation or ticket.
    pub fn rust_bounds(self) -> String {
        format!("conduit_mcu_host_runtime::root::ForeCarrierBounds{{maximum_lines:{},maximum_frame_bytes:{},maximum_payload_bytes:{},maximum_buffered_bytes:{},maximum_in_flight_items:{},maximum_frames:{}}}", self.maximum_lines,self.maximum_frame_bytes,self.maximum_payload_bytes,self.maximum_buffered_bytes,self.maximum_in_flight_items,self.maximum_frames)
    }
}
#[cfg(test)]
mod tests;
