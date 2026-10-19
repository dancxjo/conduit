//! Consume bounded exact data with preallocated interval scratch and shared arithmetic.
use super::*;
use crate::{
    MathematicalValueRefusal, MathematicalValueView, MATHEMATICAL_VALUE_MAX_ENCODED_BYTES,
    MATHEMATICAL_VALUE_MAX_NODES,
};

#[derive(Debug, Clone, Copy)]
pub struct BorrowedMathematicalEnclosure<'source> {
    source: MathematicalValueView<'source>,
    target: MathematicalInterval,
    width: ExactRational,
    requirements: MathematicalEnclosureRequirements,
    admitted_limb_work: u64,
    limb_work: u64,
    maximum_used_constant_terms: u16,
}
impl<'source> BorrowedMathematicalEnclosure<'source> {
    pub const fn source(&self) -> MathematicalValueView<'source> {
        self.source
    }
    pub const fn target(&self) -> MathematicalInterval {
        self.target
    }
    pub const fn width(&self) -> ExactRational {
        self.width
    }
    pub(super) const fn width_ref(&self) -> &ExactRational {
        &self.width
    }
    pub const fn requirements(&self) -> MathematicalEnclosureRequirements {
        self.requirements
    }
    pub const fn admitted_limb_work(&self) -> u64 {
        self.admitted_limb_work
    }
    pub const fn limb_work(&self) -> u64 {
        self.limb_work
    }
    pub const fn maximum_used_constant_terms(&self) -> u16 {
        self.maximum_used_constant_terms
    }
}

/// The 32-node scratch profile is prepared before execution. Inputs remain
/// borrowed and successful results own their numerical endpoints. Failure
/// yields no result; partially updated private scratch is cleared on the next
/// call. This does not attest producer execution or bind a Source/physical Type.
#[derive(Debug)]
pub struct PreparedMathematicalEnclosureEvaluator {
    requirements: MathematicalEnclosureRequirements,
    values: Vec<MathematicalInterval>,
    maximum_limb_work: u64,
    maximum_source_bytes: usize,
    scratch_payload_bytes: usize,
    retained_payload_bytes: usize,
}
impl PreparedMathematicalEnclosureEvaluator {
    pub fn new(
        requirements: MathematicalEnclosureRequirements,
        maximum_limb_work: u64,
        maximum_source_bytes: usize,
    ) -> Result<Self, MathematicalEnclosureRefusal> {
        use MathematicalEnclosureRefusal as R;
        if requirements.maximum_width.is_negative() || requirements.maximum_leaf_width.is_negative()
        {
            return Err(R::NegativeTolerance);
        }
        if !(4..=MATHEMATICAL_VALUE_MAX_ENCODED_BYTES).contains(&maximum_source_bytes) {
            return Err(R::Numeric(ExactNumericRefusal::ExtentExceeded));
        }
        if maximum_limb_work == 0 {
            return Err(R::Numeric(ExactNumericRefusal::WorkLimit));
        }
        let scratch = MATHEMATICAL_VALUE_MAX_NODES
            .checked_mul(size_of::<MathematicalInterval>())
            .ok_or(R::ScratchStorageLimit)?;
        let retained = scratch
            .checked_add(size_of::<Self>())
            .ok_or(R::RetainedStorageLimit)?;
        if scratch > requirements.maximum_scratch_payload_bytes as usize {
            return Err(R::ScratchStorageLimit);
        }
        if retained > requirements.maximum_retained_payload_bytes as usize {
            return Err(R::RetainedStorageLimit);
        }
        let values = Vec::with_capacity(MATHEMATICAL_VALUE_MAX_NODES);
        let scratch = values
            .capacity()
            .checked_mul(size_of::<MathematicalInterval>())
            .ok_or(R::ScratchStorageLimit)?;
        let retained = scratch
            .checked_add(size_of::<Self>())
            .ok_or(R::RetainedStorageLimit)?;
        if scratch > requirements.maximum_scratch_payload_bytes as usize {
            return Err(R::ScratchStorageLimit);
        }
        if retained > requirements.maximum_retained_payload_bytes as usize {
            return Err(R::RetainedStorageLimit);
        }
        Ok(Self {
            requirements,
            values,
            maximum_limb_work,
            maximum_source_bytes,
            scratch_payload_bytes: scratch,
            retained_payload_bytes: retained,
        })
    }
    /// Accounted vector/container payload, excluding allocator overhead, stack,
    /// caller input and the returned result's storage.
    pub const fn retained_payload_bytes(&self) -> usize {
        self.retained_payload_bytes
    }
    pub const fn scratch_payload_bytes(&self) -> usize {
        self.scratch_payload_bytes
    }
    pub const fn requirements(&self) -> MathematicalEnclosureRequirements {
        self.requirements
    }
    pub const fn maximum_limb_work(&self) -> u64 {
        self.maximum_limb_work
    }
    pub const fn maximum_source_bytes(&self) -> usize {
        self.maximum_source_bytes
    }
    /// Validate and enclose exact input data without allocation or growth. Work
    /// consumed on refusals is charged too; caller budgets cannot loosen this
    /// prepared evaluator's ceiling. A result borrows only the caller's source,
    /// so later execution cannot invalidate an earlier successful interval.
    pub fn execute<'source>(
        &mut self,
        encoded: &'source [u8],
        work: &mut ExactNumericWork,
    ) -> Result<BorrowedMathematicalEnclosure<'source>, MathematicalEnclosureRefusal> {
        if encoded.len() > self.maximum_source_bytes {
            return Err(MathematicalEnclosureRefusal::Numeric(
                ExactNumericRefusal::ExtentExceeded,
            ));
        }
        let mut local = ExactNumericWork::new(work.remaining().min(self.maximum_limb_work));
        let result = self.evaluate(encoded, &mut local);
        work.charge(local.spent())?;
        result
    }
    /// Independently recompute an incoming interval claim. An optional separately
    /// retained source pins representation custody; it is not a mathematical
    /// equality decision across distinct symbolic encodings. Only recomputed
    /// values and work metadata are returned, never untrusted claim fields.
    pub fn verify<'source>(
        &mut self,
        encoded: &'source [u8],
        expected_source: Option<&[u8]>,
        claimed_lower: ExactRational,
        claimed_upper: ExactRational,
        claimed_width: ExactRational,
        work: &mut ExactNumericWork,
    ) -> Result<BorrowedMathematicalEnclosure<'source>, MathematicalEnclosureRefusal> {
        if encoded.len() > self.maximum_source_bytes {
            return Err(MathematicalEnclosureRefusal::Numeric(
                ExactNumericRefusal::ExtentExceeded,
            ));
        }
        if expected_source.is_some_and(|expected| expected != encoded) {
            return Err(MathematicalEnclosureRefusal::SourceMismatch);
        }
        let result = self.execute(encoded, work)?;
        if result.target.lower() != claimed_lower
            || result.target.upper() != claimed_upper
            || result.width != claimed_width
        {
            return Err(MathematicalEnclosureRefusal::InvalidClaim);
        }
        Ok(result)
    }
    fn evaluate<'source>(
        &mut self,
        encoded: &'source [u8],
        work: &mut ExactNumericWork,
    ) -> Result<BorrowedMathematicalEnclosure<'source>, MathematicalEnclosureRefusal> {
        let before = work.spent();
        let admitted = work.remaining();
        let map = |why| match why {
            MathematicalValueRefusal::Numeric(n) => MathematicalEnclosureRefusal::Numeric(n),
            _ => MathematicalEnclosureRefusal::InvalidExpression,
        };
        let source = MathematicalValueView::decode(encoded, work).map_err(map)?;
        self.values.clear();
        let mut terms = 0;
        for index in 0..source.node_count() {
            let node = source.node(index, work).map_err(map)?;
            self.values.push(evaluate::node(
                node,
                &self.values,
                self.requirements,
                &mut terms,
                work,
            )?);
        }
        let target = *self
            .values
            .last()
            .ok_or(MathematicalEnclosureRefusal::InvalidExpression)?;
        let width = target.width(work)?;
        if width.checked_compare(self.requirements.maximum_width, work)? == Ordering::Greater {
            return Err(MathematicalEnclosureRefusal::ToleranceExceeded);
        }
        Ok(BorrowedMathematicalEnclosure {
            source,
            target,
            width,
            requirements: self.requirements,
            admitted_limb_work: admitted,
            limb_work: work.spent() - before,
            maximum_used_constant_terms: terms,
        })
    }
}
