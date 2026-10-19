//! Bounded coordinate enclosure with the physical frame retained independently.
use crate::{
    BorrowedMathematicalEnclosure, ExactNumericRefusal, ExactNumericWork, ExactRational,
    MathematicalEnclosureRefusal, MathematicalEnclosureRequirements, MathematicalQuantityRefusal,
    MathematicalQuantityView, PreparedMathematicalEnclosureEvaluator, RationalQuantity,
    MATHEMATICAL_VALUE_MAX_ENCODED_BYTES,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathematicalQuantityEnclosureRefusal {
    Quantity(MathematicalQuantityRefusal),
    Enclosure(MathematicalEnclosureRefusal),
    Numeric(ExactNumericRefusal),
    RetainedStorageLimit,
}
use MathematicalQuantityEnclosureRefusal as R;
pub struct PreparedMathematicalQuantityEnclosure {
    evaluator: PreparedMathematicalEnclosureEvaluator,
    maximum_work: u64,
    retained_payload_bytes: usize,
}
pub struct BorrowedMathematicalQuantityEnclosure<'a> {
    source: MathematicalQuantityView<'a>,
    enclosure: BorrowedMathematicalEnclosure<'a>,
}
impl PreparedMathematicalQuantityEnclosure {
    pub fn new(
        requirements: MathematicalEnclosureRequirements,
        maximum_work: u64,
    ) -> Result<Self, R> {
        let minimum_payload = crate::MATHEMATICAL_VALUE_MAX_NODES
            .checked_mul(core::mem::size_of::<crate::MathematicalInterval>())
            .and_then(|n| n.checked_add(core::mem::size_of::<Self>()))
            .ok_or(R::RetainedStorageLimit)?;
        if minimum_payload > requirements.maximum_retained_payload_bytes as usize {
            return Err(R::RetainedStorageLimit);
        }
        let evaluator = PreparedMathematicalEnclosureEvaluator::new(
            requirements,
            maximum_work,
            MATHEMATICAL_VALUE_MAX_ENCODED_BYTES,
        )
        .map_err(R::Enclosure)?;
        let mut state = Self {
            evaluator,
            maximum_work,
            retained_payload_bytes: 0,
        };
        state.retained_payload_bytes = core::mem::size_of::<Self>()
            .checked_add(state.evaluator.scratch_payload_bytes())
            .ok_or(R::RetainedStorageLimit)?;
        if state.retained_payload_bytes() > requirements.maximum_retained_payload_bytes as usize {
            return Err(R::RetainedStorageLimit);
        }
        Ok(state)
    }
    /// Container plus prepared interval vector; excludes stack, input bytes and
    /// returned endpoints. No Unit or numerical representation is changed here.
    pub fn retained_payload_bytes(&self) -> usize {
        self.retained_payload_bytes
    }
    pub fn execute<'a>(
        &mut self,
        encoded: &'a [u8],
        work: &mut ExactNumericWork,
    ) -> Result<BorrowedMathematicalQuantityEnclosure<'a>, R> {
        let mut local = ExactNumericWork::new(work.remaining().min(self.maximum_work));
        let result = (|| {
            let source =
                MathematicalQuantityView::decode(encoded, &mut local).map_err(R::Quantity)?;
            let enclosure = self
                .evaluator
                .execute(source.coordinate().encoded(), &mut local)
                .map_err(R::Enclosure)?;
            Ok(BorrowedMathematicalQuantityEnclosure { source, enclosure })
        })();
        work.charge(local.spent()).map_err(R::Numeric)?;
        result
    }
}
impl<'a> BorrowedMathematicalQuantityEnclosure<'a> {
    pub const fn source(&self) -> &MathematicalQuantityView<'a> {
        &self.source
    }
    pub fn lower(&self) -> RationalQuantity {
        RationalQuantity::new(
            self.enclosure.target().lower(),
            self.source.unit(),
            self.source.role(),
        )
        .expect("validated physical role")
    }
    pub fn upper(&self) -> RationalQuantity {
        RationalQuantity::new(
            self.enclosure.target().upper(),
            self.source.unit(),
            self.source.role(),
        )
        .expect("validated physical role")
    }
    /// Numerical width coefficient in the retained Unit frame; this is not a
    /// physical point and must not acquire an affine point offset.
    pub const fn width_coordinate(&self) -> ExactRational {
        self.enclosure.width()
    }
    pub const fn numerical(&self) -> &BorrowedMathematicalEnclosure<'a> {
        &self.enclosure
    }
}
