//! Exact Unit changes across the existing native/rational/mathematical wires.
//! Native conversion is attempted before optional rational materialization.
use super::*;
use crate::{QuantityConversionRefusal, QuantityValueContractRefusal};
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantityCoordinateConversionRefusal {
    Input(QuantityValueContractRefusal),
    Encoding(QuantityValueRefusal),
    Physical(RationalQuantityRefusal),
    Mathematical(MathematicalQuantityConversionRefusal),
    Numeric(ExactNumericRefusal),
    RetainedStorageLimit,
    OutputTooSmall,
}
use QuantityCoordinateConversionRefusal as R;

pub struct PreparedQuantityCoordinateConversion {
    mathematical: PreparedMathematicalQuantityConversion,
    staged: Vec<u8>,
    maximum_work: u64,
    retained_payload_bytes: usize,
}
/// Private computational receipt; caller identities do not authenticate input.
pub struct QuantityCoordinateConversion<'source> {
    source: QuantityValueView<'source>,
    target_unit: Unit,
    target_representation: QuantityCoordinateRepresentation,
    scale: ExactRational,
    offset: ExactRational,
    reference_law: Option<&'static str>,
    encoded_bytes: usize,
    target_digest: [u8; 32],
    spent_work: u64,
    admitted_work: u64,
}
impl QuantityCoordinateConversion<'_> {
    pub const fn source(&self) -> &QuantityValueView<'_> {
        &self.source
    }
    pub const fn target_unit(&self) -> Unit {
        self.target_unit
    }
    pub const fn target_representation(&self) -> QuantityCoordinateRepresentation {
        self.target_representation
    }
    pub const fn scale(&self) -> ExactRational {
        self.scale
    }
    /// Additional exact reference law; scale is its rational coefficient.
    pub const fn reference_law(&self) -> Option<&'static str> {
        self.reference_law
    }
    pub const fn offset(&self) -> ExactRational {
        self.offset
    }
    pub const fn encoded_bytes(&self) -> usize {
        self.encoded_bytes
    }
    pub const fn target_representation_digest(&self) -> [u8; 32] {
        self.target_digest
    }
    pub const fn spent_work(&self) -> u64 {
        self.spent_work
    }
    pub const fn admitted_work(&self) -> u64 {
        self.admitted_work
    }
}
impl PreparedQuantityCoordinateConversion {
    pub fn new(maximum_retained_payload_bytes: u32, maximum_work: u64) -> Result<Self, R> {
        let extra = core::mem::size_of::<Self>()
            .checked_sub(core::mem::size_of::<PreparedMathematicalQuantityConversion>())
            .and_then(|n| n.checked_add(MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES))
            .ok_or(R::RetainedStorageLimit)?;
        let available = usize::try_from(maximum_retained_payload_bytes)
            .map_err(|_| R::RetainedStorageLimit)?
            .checked_sub(extra)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or(R::RetainedStorageLimit)?;
        let mathematical = PreparedMathematicalQuantityConversion::new(available, maximum_work)
            .map_err(R::Mathematical)?;
        let staged = alloc::vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
        let retained_payload_bytes = extra
            .checked_sub(MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES)
            .and_then(|n| n.checked_add(staged.capacity()))
            .and_then(|n| n.checked_add(mathematical.retained_payload_bytes()))
            .ok_or(R::RetainedStorageLimit)?;
        if retained_payload_bytes > maximum_retained_payload_bytes as usize {
            return Err(R::RetainedStorageLimit);
        }
        Ok(Self {
            mathematical,
            staged,
            maximum_work,
            retained_payload_bytes,
        })
    }
    /// Excludes stack, input/output and the returned borrowed receipt.
    pub const fn retained_payload_bytes(&self) -> usize {
        self.retained_payload_bytes
    }
    pub fn execute<'source>(
        &mut self,
        encoded: &'source [u8],
        target: Unit,
        output: &mut [u8],
        work: &mut ExactNumericWork,
    ) -> Result<QuantityCoordinateConversion<'source>, R> {
        let admitted = work.remaining().min(self.maximum_work);
        let mut local = ExactNumericWork::new(admitted);
        let result = self.convert(encoded, target, output, &mut local);
        work.charge(local.spent()).map_err(R::Numeric)?;
        result
    }
    fn convert<'source>(
        &mut self,
        encoded: &'source [u8],
        target: Unit,
        output: &mut [u8],
        work: &mut ExactNumericWork,
    ) -> Result<QuantityCoordinateConversion<'source>, R> {
        let admitted_work = work.remaining();
        let before = work.spent();
        let source = QuantityValueProfile::Mathematical
            .decode(crate::QUANTITY_INFO_ID, encoded, work)
            .map_err(R::Input)?;
        // Coefficients come from the role-aware physical law independently of
        // the source magnitude. Exact native conversion does not first
        // materialize an owning rational coordinate.
        let (scale, offset, n, reference_law) = if target == Unit::Radian
            && source.family() == Unit::Turn.family()
            && source.unit().reference_anchor() != Unit::Radian.reference_anchor()
        {
            let converted = self
                .mathematical
                .execute_radians(encoded, &mut self.staged, work)
                .map_err(|why| match why {
                    AngleRadianQuantityRefusal::Input(n) => R::Input(n),
                    AngleRadianQuantityRefusal::Physical(n) => R::Physical(n),
                    AngleRadianQuantityRefusal::Numeric(n) => R::Numeric(n),
                    AngleRadianQuantityRefusal::Mathematical(n) => {
                        R::Mathematical(MathematicalQuantityConversionRefusal::Mathematical(n))
                    }
                    AngleRadianQuantityRefusal::RetainedStorageLimit => R::RetainedStorageLimit,
                })?;
            (
                converted.scale(),
                ExactRational::ZERO,
                converted.encoded_bytes(),
                converted.reference_law(),
            )
        } else {
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
            let n = if source.representation() == QuantityCoordinateRepresentation::Mathematical {
                self.mathematical
                    .execute(encoded, target, &mut self.staged, work)
                    .map_err(R::Mathematical)?
                    .encoded_bytes()
            } else if let Some(native) = source.decimal() {
                work.charge(1).map_err(R::Numeric)?;
                match native.convert(target) {
                    Ok(value) => {
                        let bytes = value.encode();
                        self.staged[..bytes.len()].copy_from_slice(&bytes);
                        bytes.len()
                    }
                    Err(
                        QuantityConversionRefusal::Inexact | QuantityConversionRefusal::Overflow,
                    ) => self.rational(source, target, work)?,
                    Err(why) => return Err(R::Physical(RationalQuantityRefusal::Physical(why))),
                }
            } else {
                self.rational(source, target, work)?
            };
            (scale, offset, n, None)
        };
        let target_view = QuantityValueProfile::Mathematical
            .decode(crate::QUANTITY_INFO_ID, &self.staged[..n], work)
            .map_err(R::Input)?;
        let target_representation = target_view.representation();
        if output.len() < n {
            return Err(R::OutputTooSmall);
        }
        // Reserve the finite hash/copy passes before publishing caller output.
        work.charge(2 * n as u64).map_err(R::Numeric)?;
        let target_digest = crate::semantic_digest(crate::QUANTITY_INFO_ID, &self.staged[..n]);
        output[..n].copy_from_slice(&self.staged[..n]);
        Ok(QuantityCoordinateConversion {
            source,
            target_unit: target,
            target_representation,
            reference_law,
            scale,
            offset,
            encoded_bytes: n,
            target_digest,
            spent_work: work.spent() - before,
            admitted_work,
        })
    }
    fn rational(
        &mut self,
        source: QuantityValueView<'_>,
        target: Unit,
        work: &mut ExactNumericWork,
    ) -> Result<usize, R> {
        let value = source
            .rational(work)
            .map_err(R::Encoding)?
            .convert_to_unit(target, work)
            .map_err(R::Physical)?;
        work.charge(RATIONAL_QUANTITY_MAX_ENCODED_BYTES as u64)
            .map_err(R::Numeric)?;
        encode_rational_quantity_value(value, &mut self.staged).map_err(R::Encoding)
    }
}
