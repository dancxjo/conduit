//! Exact Angle coordinate conversion across all explicitly admitted wire forms.
use crate::*;
use alloc::vec::Vec;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AngleRadianQuantityRefusal {
    Input(QuantityValueContractRefusal),
    Physical(RationalQuantityRefusal),
    Mathematical(MathematicalValueRefusal),
    Numeric(ExactNumericRefusal),
    RetainedStorageLimit,
}
use AngleRadianQuantityRefusal as R;
pub struct PreparedAngleRadianQuantityConversion {
    builder: PreparedMathematicalValueBuilder,
    coordinate: Vec<u8>,
    maximum_work: u64,
    retained_payload_bytes: usize,
}
/// Computational receipt; spelling, binding and execution custody stay separate.
pub struct AngleRadianQuantityConversion<'a> {
    source: QuantityValueView<'a>,
    scale: ExactRational,
    uses_pi_law: bool,
    encoded_bytes: usize,
    target_digest: [u8; 32],
}
impl<'a> AngleRadianQuantityConversion<'a> {
    pub const fn source(&self) -> &QuantityValueView<'a> {
        &self.source
    }
    pub const fn scale(&self) -> ExactRational {
        self.scale
    }
    pub const fn reference_law(&self) -> Option<&'static str> {
        if self.uses_pi_law {
            Some(TURN_RADIAN_REFERENCE_LAW)
        } else {
            None
        }
    }
    pub const fn encoded_bytes(&self) -> usize {
        self.encoded_bytes
    }
    pub const fn target_representation_digest(&self) -> [u8; 32] {
        self.target_digest
    }
}
impl PreparedAngleRadianQuantityConversion {
    pub fn new(maximum_retained_payload_bytes: u32, maximum_work: u64) -> Result<Self, R> {
        if maximum_work == 0 {
            return Err(R::Numeric(ExactNumericRefusal::WorkLimit));
        }
        let extra = core::mem::size_of::<Self>()
            - core::mem::size_of::<PreparedMathematicalValueBuilder>()
            + MATHEMATICAL_VALUE_MAX_ENCODED_BYTES;
        let budget = (maximum_retained_payload_bytes as usize)
            .checked_sub(extra)
            .and_then(|v| u32::try_from(v).ok())
            .ok_or(R::RetainedStorageLimit)?;
        let builder =
            PreparedMathematicalValueBuilder::new(budget, MATHEMATICAL_VALUE_MAX_ENCODED_BYTES)
                .map_err(R::Mathematical)?;
        let coordinate = alloc::vec![0;MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
        let retained_payload_bytes = extra - MATHEMATICAL_VALUE_MAX_ENCODED_BYTES
            + coordinate.capacity()
            + builder.retained_payload_bytes();
        if retained_payload_bytes > maximum_retained_payload_bytes as usize {
            return Err(R::RetainedStorageLimit);
        }
        Ok(Self {
            builder,
            coordinate,
            maximum_work,
            retained_payload_bytes,
        })
    }
    /// Container and prepared buffers; excludes input/output, stack and receipt.
    pub const fn retained_payload_bytes(&self) -> usize {
        self.retained_payload_bytes
    }
    pub fn execute<'a>(
        &mut self,
        input: &'a [u8],
        output: &mut [u8],
        work: &mut ExactNumericWork,
    ) -> Result<AngleRadianQuantityConversion<'a>, R> {
        let mut local = ExactNumericWork::new(work.remaining().min(self.maximum_work));
        let result = self.convert(input, output, &mut local);
        work.charge(local.spent()).map_err(R::Numeric)?;
        result
    }
    fn convert<'a>(
        &mut self,
        input: &'a [u8],
        output: &mut [u8],
        work: &mut ExactNumericWork,
    ) -> Result<AngleRadianQuantityConversion<'a>, R> {
        Self::convert_prepared(&mut self.builder, &mut self.coordinate, input, output, work)
    }
    pub(super) fn convert_prepared<'a>(
        builder: &mut PreparedMathematicalValueBuilder,
        coordinate: &mut [u8],
        input: &'a [u8],
        output: &mut [u8],
        work: &mut ExactNumericWork,
    ) -> Result<AngleRadianQuantityConversion<'a>, R> {
        let source = QuantityValueProfile::Mathematical
            .decode(ANGLE_INFO_ID, input, work)
            .map_err(R::Input)?;
        if source.unit() == Unit::Radian && input.first() == Some(&3) {
            let target = output
                .get_mut(..input.len())
                .ok_or(R::Numeric(ExactNumericRefusal::EncodingBufferTooSmall))?;
            work.charge((2 * input.len()) as u64).map_err(R::Numeric)?;
            target.copy_from_slice(input);
            return Ok(AngleRadianQuantityConversion {
                source,
                scale: ExactRational::ONE,
                uses_pi_law: false,
                encoded_bytes: input.len(),
                target_digest: semantic_digest(QUANTITY_INFO_ID, target),
            });
        }
        let one = RationalQuantity::new(ExactRational::ONE, source.unit(), QuantityRole::Linear)
            .map_err(R::Physical)?;
        let (scale, uses_pi_law) = match one.convert_to_unit(Unit::Radian, work) {
            Ok(radians) => (radians.coordinate(), false),
            Err(RationalQuantityRefusal::Physical(QuantityConversionRefusal::Inexact)) => {
                let turns = one.convert_to_unit(Unit::Turn, work).map_err(R::Physical)?;
                (
                    turns
                        .coordinate()
                        .checked_multiply(ExactRational::from_i128(2), work)
                        .map_err(R::Numeric)?,
                    true,
                )
            }
            Err(why) => return Err(R::Physical(why)),
        };
        builder.reset();
        let mathematical = if input.first() == Some(&3) {
            Some(
                MathematicalQuantityView::decode(input, work).map_err(|why| {
                    R::Input(QuantityValueContractRefusal::Encoding(
                        QuantityValueRefusal::Mathematical(why),
                    ))
                })?,
            )
        } else {
            None
        };
        let rational = if let Some(value) = &mathematical {
            value
                .coordinate()
                .as_rational(work)
                .map_err(R::Mathematical)?
        } else {
            Some(
                source
                    .rational(work)
                    .map_err(|why| R::Input(QuantityValueContractRefusal::Encoding(why)))?
                    .coordinate(),
            )
        };
        let root = if let Some(value) = rational {
            let coefficient = value.checked_multiply(scale, work).map_err(R::Numeric)?;
            if !uses_pi_law || coefficient == ExactRational::ZERO {
                builder
                    .insert(MathematicalNode::Rational(coefficient), work)
                    .map_err(R::Mathematical)?
            } else {
                let pi = builder
                    .insert(MathematicalNode::Pi, work)
                    .map_err(R::Mathematical)?;
                if coefficient == ExactRational::ONE {
                    pi
                } else {
                    let factor = builder
                        .insert(MathematicalNode::Rational(coefficient), work)
                        .map_err(R::Mathematical)?;
                    builder
                        .insert(MathematicalNode::Multiply(factor, pi), work)
                        .map_err(R::Mathematical)?
                }
            }
        } else {
            let value = mathematical.ok_or(R::Input(QuantityValueContractRefusal::Encoding(
                QuantityValueRefusal::Truncated,
            )))?;
            let mut root = builder
                .append_value(value.coordinate(), work)
                .map_err(R::Mathematical)?;
            if scale != ExactRational::ONE {
                let factor = builder
                    .insert(MathematicalNode::Rational(scale), work)
                    .map_err(R::Mathematical)?;
                root = builder
                    .insert(MathematicalNode::Multiply(root, factor), work)
                    .map_err(R::Mathematical)?;
            }
            if uses_pi_law {
                let pi = builder
                    .insert(MathematicalNode::Pi, work)
                    .map_err(R::Mathematical)?;
                root = builder
                    .insert(MathematicalNode::Multiply(root, pi), work)
                    .map_err(R::Mathematical)?;
            }
            root
        };
        let n = builder
            .encode(root, coordinate, work)
            .map_err(R::Mathematical)?;
        let value =
            MathematicalValueView::decode(&coordinate[..n], work).map_err(R::Mathematical)?;
        work.charge((2 + UNIT_ENCODED_LEN + n) as u64)
            .map_err(R::Numeric)?;
        let encoded_bytes = encode_mathematical_quantity_value(
            &value,
            Unit::Radian,
            QuantityRole::Linear,
            output,
            work,
        )
        .map_err(|why| {
            R::Input(QuantityValueContractRefusal::Encoding(
                QuantityValueRefusal::Mathematical(why),
            ))
        })?;
        let target_digest = semantic_digest(QUANTITY_INFO_ID, &output[..encoded_bytes]);
        Ok(AngleRadianQuantityConversion {
            source,
            scale,
            uses_pi_law,
            encoded_bytes,
            target_digest,
        })
    }
}
