//! Exact linked structured ingress through the same admitted carrier mailbox.
use super::*;
#[derive(Clone, Copy)]
pub struct StructuredIngressSpec<'p> {
    pin: &'p c::AssignedLineForeRequirement,
    validator: c::BorrowedStructuredValueValidator<'static>,
    byte_capacity: u32,
}
impl<'p> StructuredIngressSpec<'p> {
    /// The ABI must match BOTH the original Line pin and complete linked Type.
    /// Unsupported constraints stay unsupported; possession is still required
    /// by the original Root activation before a value can reach the kernel.
    pub fn checked(
        pin: &'p c::AssignedLineForeRequirement,
        abi: c::AssignedForeAbi<'_>,
        schema: &'static [u8],
    ) -> Result<Self, Refusal> {
        if pin.fore().direction() != c::AssignedForeDirection::Input
            || abi.track != c::ConnectionTrack::Payload
            || abi.abnormal_kind.is_some()
            || abi.item_capacity != 1
            || abi.byte_capacity == 0
            || abi.quantity_profile != c::QuantityValueProfile::Decimal
            || !matches!(abi.temporal, "value" | "flow-open" | "flow-closing")
        {
            return Err(Refusal::UnsupportedAbi);
        }
        if let Some(contract) = abi.value_contract {
            if contract.value_kind.as_str() != abi.value_kind
                || contract.maximum_bytes > abi.byte_capacity
                || !contract.constraints.is_empty()
                || contract.validate_definition().is_err()
            {
                return Err(Refusal::UnsupportedAbi);
            }
        }
        let maximum = abi
            .value_contract
            .map_or(abi.byte_capacity, |contract| contract.maximum_bytes);
        let validator = c::BorrowedStructuredValueValidator::new(schema, maximum as usize)
            .map_err(|_| Refusal::UnsupportedAbi)?;
        let byte_capacity = abi.byte_capacity;
        if !validator.matches_profile(abi.value_kind)
            || pin.fore().payload()[21..] != c::assigned_fore_abi_digest(abi)
        {
            return Err(Refusal::UnsupportedAbi);
        }
        Ok(Self {
            pin,
            validator,
            byte_capacity,
        })
    }
    pub const fn pin(&self) -> &'p c::AssignedLineForeRequirement {
        self.pin
    }
    pub fn byte_capacity(&self) -> u32 {
        self.byte_capacity
    }
    fn validate(&self, bytes: &[u8]) -> Result<(), Refusal> {
        if bytes.len() > self.validator.maximum_bytes() {
            return Err(Refusal::Bounds);
        }
        self.validator
            .validate(bytes)
            .map(|_| ())
            .map_err(|_| Refusal::InvalidValue)
    }
}
#[derive(Clone, Copy)]
pub enum IngressSpec<'p> {
    Primitive(PrimitiveIngressSpec<'p>),
    Structured(StructuredIngressSpec<'p>),
}
impl<'p> From<PrimitiveIngressSpec<'p>> for IngressSpec<'p> {
    fn from(value: PrimitiveIngressSpec<'p>) -> Self {
        Self::Primitive(value)
    }
}
impl<'p> From<StructuredIngressSpec<'p>> for IngressSpec<'p> {
    fn from(value: StructuredIngressSpec<'p>) -> Self {
        Self::Structured(value)
    }
}
impl<'p> IngressSpec<'p> {
    pub const fn pin(&self) -> &'p c::AssignedLineForeRequirement {
        match self {
            Self::Primitive(spec) => spec.pin(),
            Self::Structured(spec) => spec.pin(),
        }
    }
    pub fn byte_capacity(&self) -> u32 {
        match self {
            Self::Primitive(spec) => spec.byte_capacity(),
            Self::Structured(spec) => spec.byte_capacity(),
        }
    }
    pub(super) fn validate(&self, bytes: &[u8]) -> Result<(), Refusal> {
        match self {
            Self::Primitive(spec) => spec.validate(bytes),
            Self::Structured(spec) => spec.validate(bytes),
        }
    }
}
