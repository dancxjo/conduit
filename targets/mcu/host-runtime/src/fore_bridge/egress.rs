//! Check the complete cold output ABI once; retain only its compact hot map.
use super::*;

/// Exact checked native Back output. The cold kind/structured contract is
/// borrowed only during `checked`; native output bytes need no second parse.
#[derive(Clone, Copy)]
pub struct EgressSpec<'p> {
    pin: &'p c::AssignedLineForeRequirement,
    byte_capacity: u32,
}
impl<'p> EgressSpec<'p> {
    pub fn checked(
        pin: &'p c::AssignedLineForeRequirement,
        abi: c::AssignedForeAbi<'_>,
    ) -> Result<Self, Refusal> {
        if pin.fore().direction() != c::AssignedForeDirection::Output
            || abi.track != c::ConnectionTrack::Payload
            || abi.abnormal_kind.is_some()
            || abi.item_capacity != 1
            || abi.byte_capacity == 0
        {
            return Err(Refusal::UnsupportedAbi);
        }
        let byte_capacity = abi.byte_capacity;
        if pin.fore().payload()[21..] != c::assigned_fore_abi_digest(abi) {
            return Err(Refusal::UnsupportedAbi);
        }
        Ok(Self { pin, byte_capacity })
    }
    pub const fn pin(&self) -> &'p c::AssignedLineForeRequirement {
        self.pin
    }
    pub const fn byte_capacity(&self) -> u32 {
        self.byte_capacity
    }
}
