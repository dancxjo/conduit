//! Exact coordinate changes reuse the physical law and canonical math builder.
use crate::{
    ExactNumericRefusal, ExactNumericWork, ExactRational, MathematicalNode as Node,
    MathematicalQuantityRefusal, MathematicalQuantityView, MathematicalValueRefusal,
    MathematicalValueView, PreparedMathematicalValueBuilder, RationalQuantity,
    RationalQuantityRefusal, Unit, MATHEMATICAL_VALUE_MAX_ENCODED_BYTES,
};
use alloc::vec::Vec;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathematicalQuantityConversionRefusal {
    Quantity(MathematicalQuantityRefusal),
    Physical(RationalQuantityRefusal),
    Mathematical(MathematicalValueRefusal),
    Numeric(ExactNumericRefusal),
    RetainedStorageLimit,
}
use MathematicalQuantityConversionRefusal as R;
pub struct PreparedMathematicalQuantityConversion {
    builder: PreparedMathematicalValueBuilder,
    coordinate: Vec<u8>,
    maximum_work: u64,
    retained_payload_bytes: usize,
}
/// Computational evidence only; Source spelling and binding custody are separate.
pub struct MathematicalQuantityCoordinateConversion<'source> {
    source: MathematicalQuantityView<'source>,
    target_unit: Unit,
    scale: ExactRational,
    offset: ExactRational,
    encoded_bytes: usize,
    target_representation_digest: [u8; 32],
    spent_work: u64,
    admitted_work: u64,
}
impl MathematicalQuantityCoordinateConversion<'_> {
    pub const fn source(&self) -> &MathematicalQuantityView<'_> {
        &self.source
    }
    pub const fn target_unit(&self) -> Unit {
        self.target_unit
    }
    pub const fn scale(&self) -> ExactRational {
        self.scale
    }
    pub const fn offset(&self) -> ExactRational {
        self.offset
    }
    /// Identity of the exact emitted frame; not physical equality or authority.
    pub const fn target_representation_digest(&self) -> [u8; 32] {
        self.target_representation_digest
    }
    pub const fn encoded_bytes(&self) -> usize {
        self.encoded_bytes
    }
    /// Actual charge to this execution's local ExactNumericWork meter.
    pub const fn spent_work(&self) -> u64 {
        self.spent_work
    }
    pub const fn admitted_work(&self) -> u64 {
        self.admitted_work
    }
}
impl PreparedMathematicalQuantityConversion {
    pub fn new(maximum_retained_payload_bytes: u32, maximum_work: u64) -> Result<Self, R> {
        if maximum_work == 0 {
            return Err(R::Numeric(ExactNumericRefusal::WorkLimit));
        }
        let extra = core::mem::size_of::<Self>()
            .checked_sub(core::mem::size_of::<PreparedMathematicalValueBuilder>())
            .and_then(|n| n.checked_add(MATHEMATICAL_VALUE_MAX_ENCODED_BYTES))
            .ok_or(R::RetainedStorageLimit)?;
        let budget = (maximum_retained_payload_bytes as usize)
            .checked_sub(extra)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or(R::RetainedStorageLimit)?;
        let builder =
            PreparedMathematicalValueBuilder::new(budget, MATHEMATICAL_VALUE_MAX_ENCODED_BYTES)
                .map_err(R::Mathematical)?;
        let coordinate = alloc::vec![0;MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
        let retained_payload_bytes = extra
            .checked_sub(MATHEMATICAL_VALUE_MAX_ENCODED_BYTES)
            .and_then(|n| n.checked_add(coordinate.capacity()))
            .and_then(|n| n.checked_add(builder.retained_payload_bytes()))
            .ok_or(R::RetainedStorageLimit)?;
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
    /// Container and prepared buffers; excludes allocator bookkeeping, stack,
    /// caller-owned source/output, and the returned receipt.
    pub const fn retained_payload_bytes(&self) -> usize {
        self.retained_payload_bytes
    }
    pub fn execute<'source>(
        &mut self,
        encoded: &'source [u8],
        target: Unit,
        output: &mut [u8],
        work: &mut ExactNumericWork,
    ) -> Result<MathematicalQuantityCoordinateConversion<'source>, R> {
        let mut local = ExactNumericWork::new(work.remaining().min(self.maximum_work));
        let result = self.convert(encoded, target, output, &mut local);
        work.charge(local.spent()).map_err(R::Numeric)?;
        result
    }
    pub(super) fn execute_radians<'a>(
        &mut self,
        input: &'a [u8],
        output: &mut [u8],
        work: &mut ExactNumericWork,
    ) -> Result<super::AngleRadianQuantityConversion<'a>, super::AngleRadianQuantityRefusal> {
        super::PreparedAngleRadianQuantityConversion::convert_prepared(
            &mut self.builder,
            &mut self.coordinate,
            input,
            output,
            work,
        )
    }
    fn convert<'source>(
        &mut self,
        encoded: &'source [u8],
        target: Unit,
        output: &mut [u8],
        work: &mut ExactNumericWork,
    ) -> Result<MathematicalQuantityCoordinateConversion<'source>, R> {
        let before = work.spent();
        let admitted_work = work.remaining();
        let source = MathematicalQuantityView::decode(encoded, work).map_err(R::Quantity)?;
        // Derive affine coefficients through the existing role-aware physical
        // conversion. This checks family/origin compatibility and applies an
        // offset only to Points. No independent Unit scale algebra is invented.
        let zero = RationalQuantity::new(ExactRational::ZERO, source.unit(), source.role())
            .and_then(|q| q.convert_to_unit(target, work))
            .map_err(R::Physical)?;
        let one = RationalQuantity::new(ExactRational::ONE, source.unit(), source.role())
            .and_then(|q| q.convert_to_unit(target, work))
            .map_err(R::Physical)?;
        let offset = zero.coordinate();
        let scale = one
            .coordinate()
            .checked_subtract(offset, work)
            .map_err(R::Numeric)?;
        self.builder.reset();
        let mut root = self
            .builder
            .append_value(source.coordinate(), work)
            .map_err(R::Mathematical)?;
        if scale != ExactRational::ONE {
            let factor = self
                .builder
                .insert(Node::Rational(scale), work)
                .map_err(R::Mathematical)?;
            root = self
                .builder
                .insert(Node::Multiply(root, factor), work)
                .map_err(R::Mathematical)?;
        }
        if offset != ExactRational::ZERO {
            let shift = self
                .builder
                .insert(Node::Rational(offset), work)
                .map_err(R::Mathematical)?;
            root = self
                .builder
                .insert(Node::Add(root, shift), work)
                .map_err(R::Mathematical)?;
        }
        let n = self
            .builder
            .encode(root, &mut self.coordinate, work)
            .map_err(R::Mathematical)?;
        let value =
            MathematicalValueView::decode(&self.coordinate[..n], work).map_err(R::Mathematical)?;
        // Encoder preflights output/work before mutation. Earlier refusal only
        // changes private prepared scratch, reset on the next transaction.
        // Reserve bounded digest work before the atomic output commit.
        work.charge((2 + crate::UNIT_ENCODED_LEN + n) as u64)
            .map_err(R::Numeric)?;
        let encoded_bytes =
            super::encode_mathematical_quantity_value(&value, target, source.role(), output, work)
                .map_err(R::Quantity)?;
        let target_representation_digest =
            crate::semantic_digest(crate::QUANTITY_INFO_ID, &output[..encoded_bytes]);
        Ok(MathematicalQuantityCoordinateConversion {
            source,
            target_representation_digest,
            target_unit: target,
            scale,
            offset,
            encoded_bytes,
            spent_work: work.spent() - before,
            admitted_work,
        })
    }
}
