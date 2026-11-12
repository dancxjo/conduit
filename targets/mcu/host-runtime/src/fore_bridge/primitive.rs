//! Check the complete borrowed ingress ABI once; retain a compact hot validator.
use super::*;

/// Exact checked primitive input. Cold contracts never survive preparation.
#[derive(Clone, Copy)]
pub struct PrimitiveIngressSpec<'p> {
    pin: &'p c::AssignedLineForeRequirement,
    value_kind: &'static str,
    byte_capacity: u32,
    count_bounds: Option<(u64, u64)>,
}
impl<'p> PrimitiveIngressSpec<'p> {
    pub fn checked(
        pin: &'p c::AssignedLineForeRequirement,
        abi: c::AssignedForeAbi<'_>,
    ) -> Result<Self, Refusal> {
        let value_kind = [
            "value/empty",
            "control/cancellation-request",
            "value/bool",
            "value/count",
            "value/scalar",
            "value/text",
            "value/bytes",
            "value/u8",
            "value/u16",
            "value/u32",
            "value/u64",
            "value/u128",
            "value/i8",
            "value/i16",
            "value/i32",
            "value/i64",
            "value/i128",
            "value/ieee754-binary32",
            "value/ieee754-binary64",
        ]
        .into_iter()
        .find(|kind| *kind == abi.value_kind)
        .ok_or(Refusal::UnsupportedAbi)?;
        if pin.fore().direction() != c::AssignedForeDirection::Input
            || abi.track != c::ConnectionTrack::Payload
            || abi.abnormal_kind.is_some()
            || abi.item_capacity != 1
            || abi.byte_capacity == 0
            || !matches!(abi.temporal, "value" | "flow-open" | "flow-closing")
        {
            return Err(Refusal::UnsupportedAbi);
        }
        let count_bounds = if let Some(contract) = abi.value_contract {
            if value_kind != c::COUNT_INFO_ID
                || contract.value_kind.as_str() != value_kind
                || contract.maximum_bytes != 8
                || abi.byte_capacity != 8
                || contract.validate_definition().is_err()
            {
                return Err(Refusal::UnsupportedAbi);
            }
            use c::{IntervalEndpoint::Inclusive, ValueConstraint::UnsignedRange};
            Some(match contract.constraints.as_slice() {
                [] => (0, u64::MAX),
                [UnsignedRange {
                    minimum,
                    maximum,
                    minimum_endpoint: Inclusive,
                    maximum_endpoint: Inclusive,
                }] => (minimum.unwrap_or(0), maximum.unwrap_or(u64::MAX)),
                _ => return Err(Refusal::UnsupportedAbi),
            })
        } else {
            None
        };
        let byte_capacity = abi.byte_capacity;
        if pin.fore().payload()[21..] != c::assigned_fore_abi_digest(abi) {
            return Err(Refusal::UnsupportedAbi);
        }
        Ok(Self {
            pin,
            value_kind,
            byte_capacity,
            count_bounds,
        })
    }
    pub const fn pin(&self) -> &'p c::AssignedLineForeRequirement {
        self.pin
    }
    pub const fn byte_capacity(&self) -> u32 {
        self.byte_capacity
    }
    pub(super) fn validate(&self, bytes: &[u8]) -> Result<(), Refusal> {
        if bytes.len() > self.byte_capacity as usize {
            return Err(Refusal::Bounds);
        }
        c::validate_primitive_info(self.value_kind, bytes).map_err(|_| Refusal::InvalidValue)?;
        if let Some((minimum, maximum)) = self.count_bounds {
            let value = c::decode_count(bytes).map_err(|_| Refusal::InvalidValue)?;
            if value < minimum || value > maximum {
                return Err(Refusal::InvalidValue);
            }
        }
        Ok(())
    }
}
#[cfg(test)]
#[path = "primitive_tests.rs"]
mod tests;
