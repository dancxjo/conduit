//! Stable private material wire, checked reads and operation fetchers.
use super::super::scalar_tape::{Instruction, Operation as S};
use super::super::structured::tape::Operation as T;
use super::super::*;
pub(super) const MAGIC: &[u8] = b"conduit.make.compact-expression.v1\0";
pub(super) const MAX: usize = 128 * 1024;
pub(super) struct Reader {
    pub(super) bytes: &'static [u8],
    pub(super) input_limit: u32,
}
impl Reader {
    pub(super) fn take(&mut self, n: usize) -> Result<&'static [u8], Refusal> {
        let (a, b) = self
            .bytes
            .split_at_checked(n)
            .ok_or(Refusal::InvalidProgram)?;
        self.bytes = b;
        Ok(a)
    }
    pub(super) fn b(&mut self) -> Result<u8, Refusal> {
        Ok(self.take(1)?[0])
    }
    pub(super) fn n(&mut self) -> Result<u32, Refusal> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub(super) fn blob(&mut self) -> Result<&'static [u8], Refusal> {
        let n = self.n()? as usize;
        if n > MAX {
            return Err(Refusal::InvalidProgram);
        }
        self.take(n)
    }
    pub(super) fn opt(&mut self) -> Result<Option<u32>, Refusal> {
        match self.b()? {
            0 => Ok(None),
            1 => Ok(Some(self.n()?)),
            _ => Err(Refusal::InvalidProgram),
        }
    }
    pub(super) fn flag(&mut self) -> Result<bool, Refusal> {
        match self.b()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Refusal::InvalidProgram),
        }
    }
}
pub(super) fn unsupported<T>() -> Result<T, Refusal> {
    Err(Refusal::UnsupportedType(
        "linked-make/unsupported-prepared-profile@1".into(),
    ))
}
const KINDS: [PrimitiveInfoKind; 12] = [
    PrimitiveInfoKind::Bool,
    PrimitiveInfoKind::U8,
    PrimitiveInfoKind::U16,
    PrimitiveInfoKind::U32,
    PrimitiveInfoKind::U64,
    PrimitiveInfoKind::U128,
    PrimitiveInfoKind::I8,
    PrimitiveInfoKind::I16,
    PrimitiveInfoKind::I32,
    PrimitiveInfoKind::I64,
    PrimitiveInfoKind::I128,
    PrimitiveInfoKind::Empty,
];
pub(super) fn kind_code(k: PrimitiveInfoKind) -> Result<u8, Refusal> {
    KINDS
        .iter()
        .position(|v| *v == k)
        .map(|i| i as u8)
        .ok_or_else(|| Refusal::UnsupportedType("linked-make/primitive@1".into()))
}
pub(super) fn kind(k: u8) -> Result<PrimitiveInfoKind, Refusal> {
    KINDS
        .get(k as usize)
        .copied()
        .ok_or(Refusal::InvalidProgram)
}
pub(super) fn unary_code(o: UnaryOperator) -> u32 {
    match o {
        UnaryOperator::Not => 0,
        UnaryOperator::Negate => 1,
    }
}
pub(super) fn unary(n: u32) -> Result<UnaryOperator, Refusal> {
    match n {
        0 => Ok(UnaryOperator::Not),
        1 => Ok(UnaryOperator::Negate),
        _ => Err(Refusal::InvalidProgram),
    }
}
const BINARY: [BinaryOperator; 18] = [
    BinaryOperator::Multiply,
    BinaryOperator::Divide,
    BinaryOperator::Remainder,
    BinaryOperator::Add,
    BinaryOperator::Subtract,
    BinaryOperator::ShiftLeft,
    BinaryOperator::ShiftRight,
    BinaryOperator::Less,
    BinaryOperator::LessOrEqual,
    BinaryOperator::Greater,
    BinaryOperator::GreaterOrEqual,
    BinaryOperator::Equal,
    BinaryOperator::NotEqual,
    BinaryOperator::BitAnd,
    BinaryOperator::BitXor,
    BinaryOperator::BitOr,
    BinaryOperator::BooleanAnd,
    BinaryOperator::BooleanOr,
];
pub(super) fn binary_code(o: BinaryOperator) -> u32 {
    BINARY.iter().position(|v| *v == o).unwrap() as u32
}
pub(super) fn binary(n: u32) -> Result<BinaryOperator, Refusal> {
    BINARY
        .get(n as usize)
        .copied()
        .ok_or(Refusal::InvalidProgram)
}
pub(super) fn word(r: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(r[offset..offset + 4].try_into().unwrap())
}
pub(in crate::expression_prepared) fn scalar_instruction<'a>(
    records: &'a [u8],
    literals: &'a [u8],
    pc: usize,
) -> Result<Instruction<&'a [u8]>, Refusal> {
    let r = records
        .get(pc * 24..pc * 24 + 24)
        .ok_or(Refusal::InvalidProgram)?;
    if r[2..4] != [0, 0] || word(r, 20) != 0 {
        return Err(Refusal::InvalidProgram);
    }
    let a = word(r, 8);
    let b = word(r, 12);
    let c = word(r, 16);
    let operation = match r[1] {
        0 if a == 0 && b == 0 && c == 0 => S::Input,
        1 if c == 0 => S::Literal(
            literals
                .get(
                    a as usize
                        ..(a as usize)
                            .checked_add(b as usize)
                            .ok_or(Refusal::InvalidProgram)?,
                )
                .ok_or(Refusal::InvalidProgram)?,
        ),
        2 if c == 0 => S::ProjectedInput {
            start: a,
            length: b,
        },
        3 if c == 0 => S::Unary(unary(a)?, b as usize),
        4 if b == 0 && c == 0 => S::Widen(a as usize),
        5 => S::Binary(binary(a)?, false, b as usize, c as usize),
        6 if c == 0 => S::BranchFalse(a as usize, b as usize),
        7 if b == 0 && c == 0 => S::Jump(a as usize),
        8 if b == 0 && c == 0 => S::Move(a as usize),
        _ => return Err(Refusal::InvalidProgram),
    };
    Ok(Instruction {
        kind: kind(r[0])?,
        destination: word(r, 4) as usize,
        operation,
    })
}
pub(in crate::expression_prepared) fn structured_instruction(
    records: &[u8],
    pc: usize,
) -> Result<T, Refusal> {
    let r = records
        .get(pc * 12..pc * 12 + 12)
        .ok_or(Refusal::InvalidProgram)?;
    if r[1..4] != [0, 0, 0] {
        return Err(Refusal::InvalidProgram);
    }
    let a = word(r, 4);
    let b = word(r, 8);
    let short = |v: u32| u16::try_from(v).map_err(|_| Refusal::InvalidProgram);
    Ok(match r[0] {
        0 => T::Bytes {
            start: a,
            length: b,
        },
        1 => T::InputNode {
            start: a,
            length: b,
        },
        2 if b == 0 => T::Value(short(a)?),
        3 => T::BranchFalse {
            value: short(a)?,
            target: short(b)?,
        },
        4 if b == 0 => T::Jump(short(a)?),
        _ => return Err(Refusal::InvalidProgram),
    })
}

pub(in crate::expression_prepared) fn next_member(
    bytes: &mut &'static [u8],
) -> Result<member_selection::Member, Refusal> {
    let mut r = Reader {
        bytes,
        input_limit: 65536,
    };
    use member_selection::Member;
    let member = match r.b()? {
        0 => Member::Field(Cow::Borrowed(
            core::str::from_utf8(r.blob()?).map_err(|_| Refusal::InvalidProgram)?,
        )),
        1 => Member::Index(r.n()?.try_into().map_err(|_| Refusal::InvalidProgram)?),
        2 => Member::Variant(Cow::Borrowed(
            core::str::from_utf8(r.blob()?).map_err(|_| Refusal::InvalidProgram)?,
        )),
        _ => return Err(Refusal::InvalidProgram),
    };
    *bytes = r.bytes;
    Ok(member)
}
