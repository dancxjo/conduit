//! Borrow large leaf values; keep only arithmetic results inline during play.
use super::Refusal;
use crate::{BinaryOperator, UnaryOperator};
use conduit_core::{
    decode_count, encode_count, primitive_info_kind, FixedInteger, InfoBool, PrimitiveInfoKind,
    Quantity, Scalar, StructuredInfoTypeShape, BOOL_INFO_ID, COUNT_INFO_ID,
    MAXIMUM_STRUCTURED_LEAF_BYTES, SCALAR_INFO_ID,
};
use core::cmp::Ordering;

pub(super) struct PrimitiveValue<'a> {
    pub(super) kind: PrimitiveInfoKind,
    pub(super) length: usize,
    bytes: PrimitiveBytes<'a>,
}

enum PrimitiveBytes<'a> {
    Borrowed(&'a [u8]),
    // The widest computed primitive is a fixed 128-bit integer. Text and
    // projected leaves retain their already admitted storage by reference.
    Inline([u8; 16]),
}

impl<'a> PrimitiveValue<'a> {
    pub(super) fn borrowed(kind: PrimitiveInfoKind, encoded: &'a [u8]) -> Result<Self, Refusal> {
        if encoded.len() > MAXIMUM_STRUCTURED_LEAF_BYTES {
            return Err(Refusal::InvalidInput);
        }
        Ok(Self {
            kind,
            length: encoded.len(),
            bytes: PrimitiveBytes::Borrowed(encoded),
        })
    }

    pub(super) fn new(kind: PrimitiveInfoKind, encoded: &[u8]) -> Result<Self, Refusal> {
        let mut bytes = [0; 16];
        if encoded.len() > bytes.len() {
            return Err(Refusal::InvalidProgram);
        }
        bytes[..encoded.len()].copy_from_slice(encoded);
        Ok(Self {
            kind,
            length: encoded.len(),
            bytes: PrimitiveBytes::Inline(bytes),
        })
    }

    pub(super) fn as_slice(&self) -> &[u8] {
        match &self.bytes {
            PrimitiveBytes::Borrowed(bytes) => bytes,
            PrimitiveBytes::Inline(bytes) => &bytes[..self.length],
        }
    }
}

pub(super) fn evaluate_unary(
    operator: UnaryOperator,
    expected: PrimitiveInfoKind,
    operand: &PrimitiveValue<'_>,
) -> Result<PrimitiveValue<'static>, Refusal> {
    match operator {
        UnaryOperator::Not => {
            PrimitiveValue::new(expected, &InfoBool::new(!decode_bool(operand)?).encode())
        }
        UnaryOperator::Negate if operand.kind == PrimitiveInfoKind::Scalar => {
            let encoded = Scalar::decode(operand.as_slice())
                .map_err(|_| Refusal::InvalidProgram)?
                .checked_neg()
                .map_err(|_| Refusal::Arithmetic)?
                .encode();
            PrimitiveValue::new(expected, &encoded)
        }
        UnaryOperator::Negate => {
            let encoded = decode_integer(operand)?
                .checked_neg()
                .map_err(|_| Refusal::Arithmetic)?
                .encode();
            PrimitiveValue::new(expected, &encoded.0[..encoded.1])
        }
    }
}

pub(super) fn evaluate_binary(
    operator: BinaryOperator,
    proven: bool,
    expected: PrimitiveInfoKind,
    left: &PrimitiveValue<'_>,
    right: &PrimitiveValue<'_>,
) -> Result<PrimitiveValue<'static>, Refusal> {
    if left.kind != right.kind {
        return Err(Refusal::InvalidProgram);
    }
    if matches!(
        operator,
        BinaryOperator::BooleanAnd | BinaryOperator::BooleanOr
    ) {
        let value = match operator {
            BinaryOperator::BooleanAnd => decode_bool(left)? && decode_bool(right)?,
            BinaryOperator::BooleanOr => decode_bool(left)? || decode_bool(right)?,
            _ => unreachable!(),
        };
        return PrimitiveValue::new(expected, &InfoBool::new(value).encode());
    }
    if left.kind == PrimitiveInfoKind::F32
        && matches!(operator, BinaryOperator::Equal | BinaryOperator::NotEqual)
    {
        let equal = left.as_slice() == right.as_slice();
        return PrimitiveValue::new(
            expected,
            &InfoBool::new(if matches!(operator, BinaryOperator::Equal) {
                equal
            } else {
                !equal
            })
            .encode(),
        );
    }
    if matches!(
        operator,
        BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
            | BinaryOperator::Equal
            | BinaryOperator::NotEqual
    ) {
        let ordering = compare(left, right)?;
        let value = match operator {
            BinaryOperator::Less => ordering.is_lt(),
            BinaryOperator::LessOrEqual => !ordering.is_gt(),
            BinaryOperator::Greater => ordering.is_gt(),
            BinaryOperator::GreaterOrEqual => !ordering.is_lt(),
            BinaryOperator::Equal => ordering.is_eq(),
            BinaryOperator::NotEqual => !ordering.is_eq(),
            _ => unreachable!(),
        };
        return PrimitiveValue::new(expected, &InfoBool::new(value).encode());
    }
    if left.kind == PrimitiveInfoKind::F32 {
        let encoded =
            crate::expression_f32::arithmetic(operator, left.as_slice(), right.as_slice())
                .ok_or(Refusal::Arithmetic)?;
        return PrimitiveValue::new(expected, &encoded);
    }
    if left.kind == PrimitiveInfoKind::Count {
        let left = decode_count(left.as_slice()).map_err(|_| Refusal::InvalidProgram)?;
        let right = decode_count(right.as_slice()).map_err(|_| Refusal::InvalidProgram)?;
        let result = match operator {
            BinaryOperator::Multiply => left.checked_mul(right),
            BinaryOperator::Divide if right != 0 => left.checked_div(right),
            BinaryOperator::Remainder if right != 0 => left.checked_rem(right),
            BinaryOperator::Add => left.checked_add(right),
            BinaryOperator::Subtract => left.checked_sub(right),
            _ => None,
        }
        .ok_or(Refusal::Arithmetic)?;
        return PrimitiveValue::new(expected, &encode_count(result));
    }
    if left.kind == PrimitiveInfoKind::Scalar {
        let left = Scalar::decode(left.as_slice()).map_err(|_| Refusal::InvalidProgram)?;
        let right = Scalar::decode(right.as_slice()).map_err(|_| Refusal::InvalidProgram)?;
        let result = match operator {
            BinaryOperator::Multiply => left.checked_mul(right),
            BinaryOperator::Divide => left.checked_div(right),
            BinaryOperator::Remainder => left.checked_rem(right),
            BinaryOperator::Add => left.checked_add(right),
            BinaryOperator::Subtract => left.checked_sub(right),
            _ => return Err(Refusal::InvalidProgram),
        }
        .map_err(|_| Refusal::Arithmetic)?;
        return PrimitiveValue::new(expected, &result.encode());
    }
    let left = decode_integer(left)?;
    let right = decode_integer(right)?;
    let result = match (operator, proven) {
        (BinaryOperator::Multiply, true) => left.wrapping_mul(right),
        (BinaryOperator::Add, true) => left.wrapping_add(right),
        (BinaryOperator::Subtract, true) => left.wrapping_sub(right),
        (BinaryOperator::Multiply, false) => left.checked_mul(right),
        (BinaryOperator::Divide, _) => left.checked_div(right),
        (BinaryOperator::Remainder, _) => left.checked_rem(right),
        (BinaryOperator::Add, false) => left.checked_add(right),
        (BinaryOperator::Subtract, false) => left.checked_sub(right),
        (BinaryOperator::BitAnd, _) => left.bit_and(right),
        (BinaryOperator::BitXor, _) => left.bit_xor(right),
        (BinaryOperator::BitOr, _) => left.bit_or(right),
        (BinaryOperator::ShiftLeft, _) => left.checked_shift_left(shift_count(right)?),
        (BinaryOperator::ShiftRight, _) => left.checked_shift_right(shift_count(right)?),
        _ => return Err(Refusal::InvalidProgram),
    }
    .map_err(|_| Refusal::Arithmetic)?;
    let encoded = result.encode();
    PrimitiveValue::new(expected, &encoded.0[..encoded.1])
}

fn compare(left: &PrimitiveValue<'_>, right: &PrimitiveValue<'_>) -> Result<Ordering, Refusal> {
    match left.kind {
        PrimitiveInfoKind::F32 => {
            crate::expression_f32::ordering(left.as_slice(), right.as_slice())
                .ok_or(Refusal::Arithmetic)
        }
        PrimitiveInfoKind::Bool | PrimitiveInfoKind::Text => {
            Ok(left.as_slice().cmp(right.as_slice()))
        }
        PrimitiveInfoKind::Count => Ok(decode_count(left.as_slice())
            .map_err(|_| Refusal::InvalidProgram)?
            .cmp(&decode_count(right.as_slice()).map_err(|_| Refusal::InvalidProgram)?)),
        PrimitiveInfoKind::Scalar => Ok(Scalar::decode(left.as_slice())
            .map_err(|_| Refusal::InvalidProgram)?
            .cmp(&Scalar::decode(right.as_slice()).map_err(|_| Refusal::InvalidProgram)?)),
        kind if quantity_kind(kind) => Quantity::decode(left.as_slice())
            .map_err(|_| Refusal::InvalidProgram)?
            .compare(Quantity::decode(right.as_slice()).map_err(|_| Refusal::InvalidProgram)?)
            .map_err(|_| Refusal::Arithmetic),
        kind if fixed_integer(kind) => decode_integer(left)?
            .compare(decode_integer(right)?)
            .map_err(|_| Refusal::InvalidProgram),
        _ => Err(Refusal::UnsupportedType(kind_name(left.kind).into())),
    }
}

pub(super) fn decode_bool(value: &PrimitiveValue<'_>) -> Result<bool, Refusal> {
    if value.kind != PrimitiveInfoKind::Bool {
        return Err(Refusal::InvalidProgram);
    }
    InfoBool::decode(value.as_slice())
        .map(InfoBool::get)
        .map_err(|_| Refusal::InvalidProgram)
}

fn decode_integer(value: &PrimitiveValue<'_>) -> Result<FixedInteger, Refusal> {
    if !fixed_integer(value.kind) {
        return Err(Refusal::InvalidProgram);
    }
    FixedInteger::decode(value.kind, value.as_slice()).map_err(|_| Refusal::InvalidProgram)
}

fn shift_count(value: FixedInteger) -> Result<u32, Refusal> {
    let value = if signed_integer(value.kind()) {
        u128::try_from(value.signed().map_err(|_| Refusal::Arithmetic)?)
            .map_err(|_| Refusal::Arithmetic)?
    } else {
        value.unsigned().map_err(|_| Refusal::Arithmetic)?
    };
    u32::try_from(value).map_err(|_| Refusal::Arithmetic)
}

/// Checked strict widening preserves the entire source domain and uses inline
/// integer storage; it never allocates during evaluation or truncates bits.
pub(super) fn evaluate_widen(
    expected: PrimitiveInfoKind,
    operand: &PrimitiveValue<'_>,
) -> Result<PrimitiveValue<'static>, Refusal> {
    let source = decode_integer(operand)?;
    let widened = if signed_integer(source.kind()) {
        FixedInteger::from_signed(
            expected,
            source.signed().map_err(|_| Refusal::InvalidProgram)?,
        )
    } else if signed_integer(expected) {
        FixedInteger::from_signed(
            expected,
            i128::try_from(source.unsigned().map_err(|_| Refusal::InvalidProgram)?)
                .map_err(|_| Refusal::InvalidProgram)?,
        )
    } else {
        FixedInteger::from_unsigned(
            expected,
            source.unsigned().map_err(|_| Refusal::InvalidProgram)?,
        )
    }
    .map_err(|_| Refusal::InvalidProgram)?;
    let (bytes, length) = widened.encode();
    PrimitiveValue::new(expected, &bytes[..length])
}

pub(super) fn leaf_kind(
    value_type: &conduit_core::StructuredInfoType,
) -> Result<PrimitiveInfoKind, Refusal> {
    match value_type.shape() {
        StructuredInfoTypeShape::Leaf(kind) => primitive_info_kind(kind.as_str())
            .ok_or_else(|| Refusal::UnsupportedType(kind.as_str().into())),
        StructuredInfoTypeShape::Nominal { representation, .. } => leaf_kind(representation),
        _ => Err(Refusal::UnsupportedType(
            "structured expression runtime".into(),
        )),
    }
}

pub(super) const fn fixed_integer(kind: PrimitiveInfoKind) -> bool {
    matches!(
        kind,
        PrimitiveInfoKind::U8
            | PrimitiveInfoKind::U16
            | PrimitiveInfoKind::U32
            | PrimitiveInfoKind::U64
            | PrimitiveInfoKind::U128
            | PrimitiveInfoKind::I8
            | PrimitiveInfoKind::I16
            | PrimitiveInfoKind::I32
            | PrimitiveInfoKind::I64
            | PrimitiveInfoKind::I128
    )
}

pub(super) const fn signed_integer(kind: PrimitiveInfoKind) -> bool {
    matches!(
        kind,
        PrimitiveInfoKind::I8
            | PrimitiveInfoKind::I16
            | PrimitiveInfoKind::I32
            | PrimitiveInfoKind::I64
            | PrimitiveInfoKind::I128
    )
}

pub(super) const fn quantity_kind(kind: PrimitiveInfoKind) -> bool {
    matches!(
        kind,
        PrimitiveInfoKind::Quantity
            | PrimitiveInfoKind::Distance
            | PrimitiveInfoKind::Frequency
            | PrimitiveInfoKind::Duration
            | PrimitiveInfoKind::Voltage
            | PrimitiveInfoKind::Temperature
            | PrimitiveInfoKind::Angle
            | PrimitiveInfoKind::Ratio
            | PrimitiveInfoKind::PixelCount
    )
}

pub(super) const fn kind_name(kind: PrimitiveInfoKind) -> &'static str {
    match kind {
        PrimitiveInfoKind::Unit => conduit_core::UNIT_INFO_ID,
        PrimitiveInfoKind::Bool => BOOL_INFO_ID,
        PrimitiveInfoKind::Text => conduit_core::TEXT_INFO_ID,
        PrimitiveInfoKind::F32 => conduit_core::F32_INFO_ID,
        PrimitiveInfoKind::Count => COUNT_INFO_ID,
        PrimitiveInfoKind::Scalar => SCALAR_INFO_ID,
        PrimitiveInfoKind::U8 => "value/u8",
        PrimitiveInfoKind::U16 => "value/u16",
        PrimitiveInfoKind::U32 => "value/u32",
        PrimitiveInfoKind::U64 => "value/u64",
        PrimitiveInfoKind::U128 => "value/u128",
        PrimitiveInfoKind::I8 => "value/i8",
        PrimitiveInfoKind::I16 => "value/i16",
        PrimitiveInfoKind::I32 => "value/i32",
        PrimitiveInfoKind::I64 => "value/i64",
        PrimitiveInfoKind::I128 => "value/i128",
        PrimitiveInfoKind::Quantity => conduit_core::QUANTITY_INFO_ID,
        PrimitiveInfoKind::ExactDecimalQuantity => conduit_core::EXACT_DECIMAL_QUANTITY_INFO_ID,
        PrimitiveInfoKind::Distance => conduit_core::DISTANCE_INFO_ID,
        PrimitiveInfoKind::Frequency => conduit_core::FREQUENCY_INFO_ID,
        PrimitiveInfoKind::Duration => conduit_core::DURATION_INFO_ID,
        PrimitiveInfoKind::Voltage => conduit_core::VOLTAGE_INFO_ID,
        PrimitiveInfoKind::Temperature => conduit_core::TEMPERATURE_INFO_ID,
        PrimitiveInfoKind::Angle => conduit_core::ANGLE_INFO_ID,
        PrimitiveInfoKind::Ratio => conduit_core::RATIO_INFO_ID,
        PrimitiveInfoKind::PixelCount => conduit_core::PIXEL_COUNT_INFO_ID,
        _ => "unsupported",
    }
}
