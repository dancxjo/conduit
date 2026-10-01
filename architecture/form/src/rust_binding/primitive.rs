use crate::prelude::*;
use crate::rust_binding::{BoundedBytes, NativeBindingRefusal};
use conduit_core::{
    validate_primitive_info, IeeeF32, IeeeF64, InfoBool, Quantity, Scalar, StructuredInfoType,
    StructuredInfoValue, StructuredInfoValueShape,
};

/// Rust representation mechanics for one primitive leaf.
///
/// The exact checked `StructuredInfoType` supplied alongside a value remains
/// authoritative; implementing this trait cannot create semantic identity.
pub trait NativePrimitive: Sized {
    fn encode_primitive(&self) -> Vec<u8>;
    fn decode_primitive(canonical: &[u8]) -> Option<Self>;
}

pub fn primitive_into_structured<T: NativePrimitive>(
    value_type: StructuredInfoType,
    value: &T,
) -> Result<StructuredInfoValue, NativeBindingRefusal> {
    StructuredInfoValue::leaf(value_type, value.encode_primitive())
        .map_err(NativeBindingRefusal::InvalidValue)
}

pub fn primitive_from_structured<T: NativePrimitive>(
    value: &StructuredInfoValue,
) -> Result<T, NativeBindingRefusal> {
    let StructuredInfoValueShape::Leaf(canonical) = value.shape() else {
        return Err(NativeBindingRefusal::InvalidValue(
            conduit_core::StructuredInfoRefusal::WrongType,
        ));
    };
    let kind = primitive_kind(value.value_type())?;
    validate_primitive_info(kind.as_str(), canonical)
        .map_err(NativeBindingRefusal::InvalidPrimitive)?;
    T::decode_primitive(canonical).ok_or(NativeBindingRefusal::InvalidValue(
        conduit_core::StructuredInfoRefusal::WrongType,
    ))
}

fn primitive_kind(
    value_type: &StructuredInfoType,
) -> Result<&conduit_core::KindId, NativeBindingRefusal> {
    match value_type.shape() {
        conduit_core::StructuredInfoTypeShape::Leaf(kind) => Ok(kind),
        conduit_core::StructuredInfoTypeShape::Nominal { representation, .. } => {
            primitive_kind(representation)
        }
        _ => Err(NativeBindingRefusal::InvalidValue(
            conduit_core::StructuredInfoRefusal::WrongType,
        )),
    }
}

impl NativePrimitive for () {
    fn encode_primitive(&self) -> Vec<u8> {
        Vec::new()
    }

    fn decode_primitive(canonical: &[u8]) -> Option<Self> {
        canonical.is_empty().then_some(())
    }
}

impl NativePrimitive for InfoBool {
    fn encode_primitive(&self) -> Vec<u8> {
        (*self).encode().to_vec()
    }

    fn decode_primitive(canonical: &[u8]) -> Option<Self> {
        Self::decode(canonical).ok()
    }
}

impl NativePrimitive for bool {
    fn encode_primitive(&self) -> Vec<u8> {
        InfoBool::new(*self).encode().to_vec()
    }

    fn decode_primitive(canonical: &[u8]) -> Option<Self> {
        InfoBool::decode(canonical).ok().map(InfoBool::get)
    }
}

impl NativePrimitive for Scalar {
    fn encode_primitive(&self) -> Vec<u8> {
        (*self).encode().to_vec()
    }

    fn decode_primitive(canonical: &[u8]) -> Option<Self> {
        Self::decode(canonical).ok()
    }
}

impl NativePrimitive for Quantity {
    fn encode_primitive(&self) -> Vec<u8> {
        (*self).encode().to_vec()
    }

    fn decode_primitive(canonical: &[u8]) -> Option<Self> {
        Self::decode(canonical).ok()
    }
}

impl NativePrimitive for String {
    fn encode_primitive(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }

    fn decode_primitive(canonical: &[u8]) -> Option<Self> {
        core::str::from_utf8(canonical).ok().map(str::to_string)
    }
}

impl<const MAXIMUM: usize> NativePrimitive for BoundedBytes<MAXIMUM> {
    fn encode_primitive(&self) -> Vec<u8> {
        self.as_slice().to_vec()
    }

    fn decode_primitive(canonical: &[u8]) -> Option<Self> {
        Self::new(canonical)
    }
}

macro_rules! fixed_integer {
    ($($value:ty),+ $(,)?) => {$ (
        impl NativePrimitive for $value {
            fn encode_primitive(&self) -> Vec<u8> {
                self.to_le_bytes().to_vec()
            }

            fn decode_primitive(canonical: &[u8]) -> Option<Self> {
                Some(<$value>::from_le_bytes(canonical.try_into().ok()?))
            }
        }
    )+ };
}

fixed_integer!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128);

macro_rules! ieee_float {
    ($($value:ty),+ $(,)?) => {$ (
        impl NativePrimitive for $value {
            fn encode_primitive(&self) -> Vec<u8> {
                self.encode().to_vec()
            }

            fn decode_primitive(canonical: &[u8]) -> Option<Self> {
                Self::decode(canonical)
            }
        }
    )+ };
}

ieee_float!(IeeeF32, IeeeF64);
