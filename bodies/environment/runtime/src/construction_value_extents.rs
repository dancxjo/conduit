//! Exact port Type ceilings, separate from queue counts and liveness admission.
use super::ApplianceSource;
use alloc::{format, string::String};
use conduit_core as c;
include!(concat!(env!("OUT_DIR"), "/sampling-value-extents.rs"));

impl ApplianceSource {
    /// Derive a conservative canonical ceiling for every normal payload port
    /// from independently checked linked Types. Bare Tick and Empty use their
    /// native codec lengths. Unknown or abnormal Types refuse explicitly.
    ///
    /// This does not prove a slot count, simultaneous occupancy, Host Call
    /// scratch, or a combined appliance allocation. It grants no authority.
    pub fn maximum_value_bytes(&self) -> Result<u32, String> {
        self.expanded
            .validate_expansion()
            .map_err(|reason| format!("value extent provenance: {reason:?}"))?;
        self.expanded
            .gears
            .iter()
            .flat_map(|gear| gear.inputs.iter().chain(&gear.outputs))
            .try_fold(1, |maximum, port| {
                if port.abnormal_kind.is_some() {
                    return Err(String::from("sampling abnormal Type needs an extent proof"));
                }
                let bytes = match port.value_kind.as_str() {
                    conduit_time::TICK_VALUE_KIND => conduit_time::TICK_ENCODED_LEN,
                    c::EMPTY_INFO_ID => 0,
                    kind => VALUE_EXTENTS
                        .iter()
                        .find(|(known, _)| *known == kind)
                        .map(|(_, bytes)| *bytes)
                        .ok_or_else(|| format!("sampling payload Type is not bounded: {kind}"))?,
                };
                Ok(maximum.max(bytes))
            })
    }
}
