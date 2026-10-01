//! Exact portable IEEE-754 values.
//!
//! Identity and equality are the encoded bits. This preserves NaN payloads,
//! signed zero, infinities, and subnormals instead of inheriting a target
//! language's floating-point equality law.

use core::cmp::Ordering;
use core::hash::{Hash, Hasher};
use serde::{Deserialize, Serialize};

macro_rules! ieee_float {
    ($name:ident, $float:ty, $bits:ty, $bytes:expr, $sign:expr) => {
        #[derive(Debug, Clone, Copy, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name($bits);

        impl $name {
            pub const ENCODED_LEN: usize = $bytes;

            pub const fn from_bits(bits: $bits) -> Self {
                Self(bits)
            }

            pub const fn bits(self) -> $bits {
                self.0
            }

            pub fn from_value(value: $float) -> Self {
                Self(value.to_bits())
            }

            pub fn value(self) -> $float {
                <$float>::from_bits(self.0)
            }

            pub fn is_finite(self) -> bool {
                self.value().is_finite()
            }

            pub const fn encode(self) -> [u8; $bytes] {
                self.0.to_le_bytes()
            }

            pub fn decode(encoded: &[u8]) -> Option<Self> {
                Some(Self(<$bits>::from_le_bytes(encoded.try_into().ok()?)))
            }

            fn total_order_key(self) -> $bits {
                let bits = self.0;
                if bits & $sign != 0 {
                    !bits
                } else {
                    bits | $sign
                }
            }
        }

        impl From<$float> for $name {
            fn from(value: $float) -> Self {
                Self::from_value(value)
            }
        }

        impl From<$name> for $float {
            fn from(value: $name) -> Self {
                value.value()
            }
        }

        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }

        impl Eq for $name {}

        impl PartialOrd for $name {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Ord for $name {
            fn cmp(&self, other: &Self) -> Ordering {
                self.total_order_key().cmp(&other.total_order_key())
            }
        }

        impl Hash for $name {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.0.hash(state);
            }
        }
    };
}

ieee_float!(IeeeF32, f32, u32, 4, 0x8000_0000);
ieee_float!(IeeeF64, f64, u64, 8, 0x8000_0000_0000_0000);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CheckedValueContract, IntervalEndpoint, ValueConstraint, ValueConstraintRefusal};
    use alloc::vec;

    #[test]
    fn bit_identity_preserves_every_edge_class() {
        let f32_bits = [
            0x7fc0_0001,
            0x7f80_0001,
            f32::INFINITY.to_bits(),
            f32::NEG_INFINITY.to_bits(),
            0.0f32.to_bits(),
            (-0.0f32).to_bits(),
            1,
            0x007f_ffff,
            f32::MIN.to_bits(),
            f32::MAX.to_bits(),
        ];
        for bits in f32_bits {
            let value = IeeeF32::from_bits(bits);
            assert_eq!(IeeeF32::decode(&value.encode()), Some(value));
        }
        assert_ne!(IeeeF32::from(0.0), IeeeF32::from(-0.0));
        assert_ne!(
            IeeeF32::from_bits(0x7fc0_0001),
            IeeeF32::from_bits(0x7fc0_0002)
        );
    }

    #[test]
    fn finite_and_ordered_contracts_refuse_nonfinite_and_out_of_range_values() {
        let finite = CheckedValueContract::new(
            crate::kind_id(crate::F32_INFO_ID),
            4,
            vec![ValueConstraint::FloatFinite],
        )
        .unwrap();
        for bits in [
            0.0f32.to_bits(),
            (-0.0f32).to_bits(),
            1,
            0x007f_ffff,
            f32::MIN.to_bits(),
            f32::MAX.to_bits(),
        ] {
            assert_eq!(finite.validate(&IeeeF32::from_bits(bits).encode()), Ok(()));
        }
        for bits in [
            0x7fc0_0001,
            0x7f80_0001,
            f32::INFINITY.to_bits(),
            f32::NEG_INFINITY.to_bits(),
        ] {
            assert_eq!(
                finite.validate(&IeeeF32::from_bits(bits).encode()),
                Err(ValueConstraintRefusal::FloatFinite)
            );
        }

        let probability = CheckedValueContract::new(
            crate::kind_id(crate::F32_INFO_ID),
            4,
            vec![
                ValueConstraint::FloatFinite,
                ValueConstraint::FloatRange {
                    minimum: Some(IeeeF32::from(0.0).encode().to_vec()),
                    maximum: Some(IeeeF32::from(1.0).encode().to_vec()),
                    minimum_endpoint: IntervalEndpoint::Inclusive,
                    maximum_endpoint: IntervalEndpoint::Inclusive,
                },
            ],
        )
        .unwrap();
        assert_eq!(
            probability.validate(&IeeeF32::from(-0.0).encode()),
            Err(ValueConstraintRefusal::FloatRange)
        );
        assert_eq!(
            probability.validate(&IeeeF32::from(1.000_000_1).encode()),
            Err(ValueConstraintRefusal::FloatRange)
        );
    }

    #[test]
    fn binary64_preserves_nan_payload_zero_and_subnormal_bits() {
        for bits in [
            0x7ff8_0000_0000_0001,
            0x7ff0_0000_0000_0001,
            f64::INFINITY.to_bits(),
            f64::NEG_INFINITY.to_bits(),
            0.0f64.to_bits(),
            (-0.0f64).to_bits(),
            1,
            0x000f_ffff_ffff_ffff,
            f64::MIN.to_bits(),
            f64::MAX.to_bits(),
        ] {
            let value = IeeeF64::from_bits(bits);
            assert_eq!(IeeeF64::decode(&value.encode()), Some(value));
        }
    }
}
