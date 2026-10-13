//! Bounded admission of the ordered computation wire graph.
use super::*;
use alloc::boxed::Box;

pub(super) fn decode(bytes: &[u8]) -> Result<NumericComputation, NumericComputationRefusal> {
    if bytes.len() > MAX_NUMERIC_COMPUTATION_BYTES {
        return Err(NumericComputationRefusal::StorageBudget);
    }
    let mut reader = Reader { bytes, offset: 0 };
    if reader.take(b"conduit.numeric-computation.v1".len())? != b"conduit.numeric-computation.v1" {
        return Err(NumericComputationRefusal::MalformedEncoding);
    }
    let length = usize::from(reader.u16()?);
    if length == 0 || length > MAX_NUMERIC_COMPUTATION_NODES {
        return Err(NumericComputationRefusal::NodeBudget);
    }
    let root = reader.u16()?;
    let mut nodes = Vec::with_capacity(length);
    for _ in 0..length {
        let length = usize::from(reader.u16()?);
        if length == 0 || length > 256 {
            return Err(NumericComputationRefusal::InvalidSemanticKind);
        }
        let semantic_kind = core::str::from_utf8(reader.take(length)?)
            .map_err(|_| NumericComputationRefusal::MalformedEncoding)?
            .into();
        let unit = match reader.byte()? {
            0 => None,
            1 => {
                let length = usize::try_from(reader.u32()?)
                    .map_err(|_| NumericComputationRefusal::StorageBudget)?;
                Some(Box::new(
                    Unit::decode(reader.take(length)?)
                        .map_err(|_| NumericComputationRefusal::InvalidUnit)?,
                ))
            }
            _ => return Err(NumericComputationRefusal::MalformedEncoding),
        };
        let reference_unit = match reader.byte()? {
            0 => None,
            1 => {
                let length = usize::try_from(reader.u32()?)
                    .map_err(|_| NumericComputationRefusal::StorageBudget)?;
                Some(Box::new(
                    Unit::decode(reader.take(length)?)
                        .map_err(|_| NumericComputationRefusal::InvalidUnit)?,
                ))
            }
            _ => return Err(NumericComputationRefusal::MalformedEncoding),
        };
        let operation = match reader.byte()? {
            0 => {
                let length = usize::try_from(reader.u32()?)
                    .map_err(|_| NumericComputationRefusal::StorageBudget)?;
                let bytes = reader.take(length)?;
                MathValueRef::validate(bytes).map_err(NumericComputationRefusal::InvalidLiteral)?;
                NumericComputationOperation::Literal(bytes.to_vec())
            }
            1 => NumericComputationOperation::Parameter(reader.u16()?),
            3 => NumericComputationOperation::IeeeLiteral {
                bits: u64::from_be_bytes(reader.take(8)?.try_into().unwrap()),
            },
            4 => {
                let length = reader.u32()? as usize;
                let source = reader.take(length)?.to_vec();
                let bits = u64::from_be_bytes(reader.take(8)?.try_into().unwrap());
                let flags = super::ieee::decode_flags(reader.byte()?)?;
                NumericComputationOperation::IeeeRoundedLiteral {
                    source,
                    bits,
                    flags,
                }
            }
            2 => {
                let operation = operation(reader.byte()?)?;
                if usize::from(reader.byte()?) != operation.arity() {
                    return Err(NumericComputationRefusal::WrongArity);
                }
                let mut operands = Vec::with_capacity(operation.arity());
                for _ in 0..operation.arity() {
                    operands.push(reader.u16()?);
                }
                NumericComputationOperation::Apply {
                    operation,
                    operands,
                }
            }
            _ => return Err(NumericComputationRefusal::MalformedEncoding),
        };
        nodes.push(NumericComputationNode {
            semantic_kind,
            unit,
            reference_unit,
            operation,
        });
    }
    if reader.offset != bytes.len() {
        return Err(NumericComputationRefusal::MalformedEncoding);
    }
    NumericComputation::new(nodes, root)
}

fn operation(tag: u8) -> Result<NumericOperation, NumericComputationRefusal> {
    use NumericOperation::*;
    [
        Negate,
        Add,
        Subtract,
        Multiply,
        Divide,
        Power,
        Root,
        SquareRoot,
        Sin,
        Cos,
        Tan,
        Asin,
        Acos,
        Atan,
        Atan2,
        Equal,
        NotEqual,
        Less,
        LessOrEqual,
        Greater,
        GreaterOrEqual,
        CyclicToAngularFrequency,
        AngularToCyclicFrequency,
        QuantityConstruction,
    ]
    .get(usize::from(tag))
    .copied()
    .ok_or(NumericComputationRefusal::MalformedEncoding)
}
struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], NumericComputationRefusal> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(NumericComputationRefusal::MalformedEncoding)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(NumericComputationRefusal::MalformedEncoding)?;
        self.offset = end;
        Ok(value)
    }
    fn byte(&mut self) -> Result<u8, NumericComputationRefusal> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, NumericComputationRefusal> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, NumericComputationRefusal> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
}
