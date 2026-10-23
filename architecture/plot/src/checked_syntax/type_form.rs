//! Checked representation metadata, separate from semantic Type identity.
use crate::prelude::*;

/// One checked compatibility Form, distinct from semantic Type identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedTypeForm {
    pub name: String,
    pub compatibility_id: String,
    pub value_type_name: String,
    pub value_type: conduit_core::KindId,
    pub storage: CheckedTypeFormStorage,
    /// Exact variant tags or Boolean-field bit masks, in selected Form order.
    pub mappings: Vec<CheckedTypeFormMapping>,
    pub invalid_refusal: CheckedTypeFormRefusal,
    pub exact_bytes: u16,
    pub maximum_bytes: u16,
    pub maximum_decode_steps: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedTypeFormStorage {
    U8,
    /// One octet, with exactly the selected Boolean field bits admitted.
    BooleanBits(u8),
    DecimalQuantity(alloc::boxed::Box<conduit_core::DecimalQuantityForm>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedTypeFormRefusal {
    InvalidTag,
    ReservedBits,
    InvalidQuantity,
}

impl CheckedTypeFormRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidTag => "invalid_tag",
            Self::ReservedBits => "reserved_bits",
            Self::InvalidQuantity => "invalid_quantity",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedTypeFormMapping {
    pub variant: String,
    pub discriminant: u8,
}
