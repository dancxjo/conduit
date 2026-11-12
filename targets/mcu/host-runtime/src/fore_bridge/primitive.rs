//! First ingress profile: unconstrained simple primitive payloads only.
use super::*;

/// Trusted Make-derived input ABI description. Preparation recomputes the SAME
/// ABI including an empty value contract; constrained/structured pins refuse.
#[derive(Clone, Copy)]
pub struct PrimitiveIngressSpec<'p> {
    pub pin: &'p c::AssignedLineForeRequirement,
    pub value_kind: &'p str,
    pub temporal: &'p str,
    pub quantity_profile: c::QuantityValueProfile,
    pub item_capacity: u16,
    pub byte_capacity: u32,
}
impl PrimitiveIngressSpec<'_> {
    pub(super) fn check(&self) -> Result<(), Refusal> {
        use c::PrimitiveInfoKind as P;
        if self.pin.fore().direction() != c::AssignedForeDirection::Input
            || self.item_capacity != 1
            || self.byte_capacity == 0
            || !matches!(self.temporal, "value" | "flow-open" | "flow-closing")
            || !matches!(
                c::primitive_info_kind(self.value_kind),
                Some(
                    P::Empty
                        | P::CancellationRequest
                        | P::Bool
                        | P::Count
                        | P::Scalar
                        | P::Text
                        | P::Bytes
                        | P::U8
                        | P::U16
                        | P::U32
                        | P::U64
                        | P::U128
                        | P::I8
                        | P::I16
                        | P::I32
                        | P::I64
                        | P::I128
                        | P::F32
                        | P::F64
                )
            )
        {
            return Err(Refusal::UnsupportedAbi);
        }
        let digest = c::assigned_fore_abi_digest(c::AssignedForeAbi {
            value_kind: self.value_kind,
            value_contract: None,
            abnormal_kind: None,
            temporal: self.temporal,
            track: c::ConnectionTrack::Payload,
            quantity_profile: self.quantity_profile,
            item_capacity: self.item_capacity,
            byte_capacity: self.byte_capacity,
        });
        if self.pin.fore().payload()[21..] != digest {
            return Err(Refusal::UnsupportedAbi);
        }
        Ok(())
    }
    pub(super) fn validate(&self, bytes: &[u8]) -> Result<(), Refusal> {
        if bytes.len() > self.byte_capacity as usize {
            return Err(Refusal::Bounds);
        }
        c::validate_primitive_info(self.value_kind, bytes).map_err(|_| Refusal::InvalidValue)
    }
}
