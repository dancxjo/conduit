//! Independent bounded verification of an interval record, without a value tree.
use super::*;
use crate::{
    validate_canonical_structured_value, MathematicalValueView, StructuredInfoType,
    StructuredInfoTypeShape, ValidatedCanonicalStructuredValue as Value, MATHEMATICAL_VALUE_ID,
    MATHEMATICAL_VALUE_MAX_ENCODED_BYTES, MATHEMATICAL_VALUE_VALIDATION_WORK,
};
pub const MATHEMATICAL_ENCLOSURE_EVIDENCE_MAX_BYTES: usize = 65536;
// Five initial field validations and at most fifteen field-traversal validations
// fit this conservative existing-validator allowance, reserved before parsing.
pub const MATHEMATICAL_ENCLOSURE_EVIDENCE_VALIDATION_WORK: u64 =
    32 * MATHEMATICAL_VALUE_VALIDATION_WORK;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathematicalEnclosureEvidenceRefusal {
    Schema,
    Malformed,
    Extent,
    StorageLimit,
    NegativeTolerance,
    PolicyMismatch,
    InsufficientTolerance,
    Numeric(ExactNumericRefusal),
    Enclosure(MathematicalEnclosureRefusal),
}
use MathematicalEnclosureEvidenceRefusal as R;
/// Schema and producer requirements are prepared independently of incoming data.
/// This checks numerical claims; it does not attest producer execution, unrelated
/// nominal Type laws, or physical/source authority. Returned fields are recomputed.
pub struct PreparedMathematicalEnclosureVerifier {
    schema: Vec<u8>,
    evaluator: PreparedMathematicalEnclosureEvaluator,
    maximum_limb_work: u64,
    retained_payload_bytes: usize,
}
impl PreparedMathematicalEnclosureVerifier {
    pub fn new(
        schema: &StructuredInfoType,
        requirements: MathematicalEnclosureRequirements,
        maximum_limb_work: u64,
        maximum_retained_payload_bytes: u32,
    ) -> Result<Self, R> {
        let StructuredInfoTypeShape::Record { fields, .. } = schema.shape() else {
            return Err(R::Schema);
        };
        let names = ["lower", "maximum-width", "source", "upper", "width"];
        if fields.len() != names.len()
            || fields.iter().zip(names).any(|(field, name)| {
                field.name() != name
                    || !matches!(field.value_type().shape(), StructuredInfoTypeShape::Leaf(kind)
                        if kind.as_str() == MATHEMATICAL_VALUE_ID)
            })
        {
            return Err(R::Schema);
        }
        let numerical_work = maximum_limb_work
            .checked_sub(MATHEMATICAL_ENCLOSURE_EVIDENCE_VALIDATION_WORK)
            .filter(|n| *n > 0)
            .ok_or(R::Numeric(ExactNumericRefusal::WorkLimit))?;
        let schema = schema.canonical_bytes().map_err(|_| R::Schema)?;
        if schema.len() > 4096 {
            return Err(R::Schema);
        }
        let evaluator = PreparedMathematicalEnclosureEvaluator::new(
            requirements,
            numerical_work,
            MATHEMATICAL_VALUE_MAX_ENCODED_BYTES,
        )
        .map_err(R::Enclosure)?;
        let retained_payload_bytes = size_of::<Self>()
            .checked_add(schema.capacity())
            .and_then(|n| n.checked_add(evaluator.scratch_payload_bytes()))
            .ok_or(R::StorageLimit)?;
        if retained_payload_bytes > maximum_retained_payload_bytes as usize {
            return Err(R::StorageLimit);
        }
        Ok(Self {
            schema,
            evaluator,
            maximum_limb_work,
            retained_payload_bytes,
        })
    }
    /// Container, schema and evaluator vector payload; excludes allocator overhead,
    /// stack, caller-owned envelope bytes and returned result storage.
    pub const fn retained_payload_bytes(&self) -> usize {
        self.retained_payload_bytes
    }
    /// Reserve a conservative validation allowance before reading the envelope.
    /// Arithmetic and failed recomputation are charged too. Caller budgets cannot
    /// increase the prepared ceiling. `expected_source` pins byte custody only.
    pub fn verify<'a>(
        &mut self,
        encoded: &'a [u8],
        consumer_maximum_width: ExactRational,
        expected_source: Option<&[u8]>,
        work: &mut ExactNumericWork,
    ) -> Result<BorrowedMathematicalEnclosure<'a>, R> {
        if encoded.len() > MATHEMATICAL_ENCLOSURE_EVIDENCE_MAX_BYTES {
            return Err(R::Extent);
        }
        if consumer_maximum_width.is_negative() {
            return Err(R::NegativeTolerance);
        }
        let mut local = ExactNumericWork::new(work.remaining().min(self.maximum_limb_work));
        let result =
            self.verify_value(encoded, consumer_maximum_width, expected_source, &mut local);
        work.charge(local.spent()).map_err(R::Numeric)?;
        result
    }
    fn verify_value<'a>(
        &mut self,
        encoded: &'a [u8],
        consumer_maximum_width: ExactRational,
        expected_source: Option<&[u8]>,
        work: &mut ExactNumericWork,
    ) -> Result<BorrowedMathematicalEnclosure<'a>, R> {
        work.charge(MATHEMATICAL_ENCLOSURE_EVIDENCE_VALIDATION_WORK)
            .map_err(R::Numeric)?;
        // Canonical values begin with their complete Type encoding. Pin it
        // before generic validation, so a foreign many-field schema cannot
        // spend a five-field profile's reserved numerical-validation budget.
        if !encoded.starts_with(&self.schema) {
            return Err(R::Schema);
        }
        let value = validate_canonical_structured_value(encoded).map_err(|_| R::Malformed)?;
        if value.type_bytes() != self.schema {
            return Err(R::Schema);
        }
        let maximum_width = number(field(value, "maximum-width")?, work)?;
        if maximum_width != self.evaluator.requirements().maximum_width {
            return Err(R::PolicyMismatch);
        }
        let source = leaf(field(value, "source")?)?;
        let lower = number(field(value, "lower")?, work)?;
        let upper = number(field(value, "upper")?, work)?;
        let width = number(field(value, "width")?, work)?;
        let result = self
            .evaluator
            .verify(source, expected_source, lower, upper, width, work)
            .map_err(R::Enclosure)?;
        if result
            .width()
            .checked_compare(consumer_maximum_width, work)
            .map_err(R::Numeric)?
            == Ordering::Greater
        {
            return Err(R::InsufficientTolerance);
        }
        Ok(result)
    }
}
fn field<'a>(value: Value<'a>, name: &str) -> Result<Value<'a>, R> {
    value
        .record_field(name)
        .map_err(|_| R::Malformed)?
        .ok_or(R::Schema)
}
fn leaf(value: Value<'_>) -> Result<&[u8], R> {
    let node = value.value_node();
    if node.first() != Some(&0) || node.len() < 5 {
        return Err(R::Schema);
    }
    let n = u32::from_le_bytes(node[1..5].try_into().unwrap()) as usize;
    if n != node.len() - 5 {
        return Err(R::Malformed);
    }
    Ok(&node[5..])
}
fn number(value: Value<'_>, work: &mut ExactNumericWork) -> Result<ExactRational, R> {
    MathematicalValueView::decode(leaf(value)?, work)
        .map_err(number_refusal)?
        .as_rational(work)
        .map_err(number_refusal)?
        .ok_or(R::Malformed)
}
fn number_refusal(why: crate::MathematicalValueRefusal) -> R {
    match why {
        crate::MathematicalValueRefusal::Numeric(why) => R::Numeric(why),
        _ => R::Malformed,
    }
}
