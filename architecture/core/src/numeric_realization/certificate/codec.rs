//! Bounded untrusted claim transport. Decoding alone never verifies a claim.
use super::*;
const MAX_BYTES: usize = 262_144;
fn flags(value: Binary64Flags) -> u8 {
    u8::from(value.invalid)
        | u8::from(value.division_by_zero) << 1
        | u8::from(value.overflow) << 2
        | u8::from(value.underflow) << 3
        | u8::from(value.inexact) << 4
}
fn decode_flags(value: u8) -> Result<Binary64Flags, NumericEngineRefusal> {
    if value & !31 != 0 {
        return Err(NumericRefusal::NonCanonical.into());
    }
    Ok(Binary64Flags {
        invalid: value & 1 != 0,
        division_by_zero: value & 2 != 0,
        overflow: value & 4 != 0,
        underflow: value & 8 != 0,
        inexact: value & 16 != 0,
    })
}
fn append_interval(
    bytes: &mut Vec<u8>,
    value: &RationalInterval,
) -> Result<(), NumericEngineRefusal> {
    for endpoint in [value.lower(), value.upper()] {
        let encoded = endpoint.canonical_bytes()?;
        bytes.extend_from_slice(&(encoded.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&encoded);
    }
    Ok(())
}
impl NumericCertificateClaim {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, NumericEngineRefusal> {
        if self.steps.len() > crate::MAX_NUMERIC_COMPUTATION_NODES
            || self.boolean_steps.len() > crate::MAX_NUMERIC_COMPUTATION_NODES
            || self.integer_steps.len() > crate::MAX_NUMERIC_COMPUTATION_NODES
        {
            return Err(NumericRefusal::NodeBudget.into());
        }
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"conduit.numeric-certificate.v1");
        bytes.extend_from_slice(&self.engine_identity);
        bytes.extend_from_slice(&self.graph_identity);
        bytes.extend_from_slice(&self.requirement_identity);
        bytes.push(match self.algorithm {
            NumericAlgorithm::OrderedRationalV1 => 0,
            NumericAlgorithm::OrderedSoftwareBinary64V1 => 1,
            NumericAlgorithm::CertifiedRationalIntervalV1 => 2,
            NumericAlgorithm::ExactComparisonV1 => 3,
            NumericAlgorithm::OrderedIeeeSoftwareBinary64V1 => 4,
            NumericAlgorithm::OrderedCheckedI64V1 => 5,
            NumericAlgorithm::SymbolicRetentionV1 => 6,
        });
        match &self.result {
            NumericResultEvidence::ExactPhysical(value) => {
                bytes.push(4);
                let encoded = value.encoded();
                bytes.extend_from_slice(&(encoded.len() as u32).to_be_bytes());
                bytes.extend_from_slice(encoded);
            }
            NumericResultEvidence::ExactMathematical(value) => {
                bytes.push(3);
                let encoded = value.canonical_bytes()?;
                bytes.extend_from_slice(&(encoded.len() as u32).to_be_bytes());
                bytes.extend_from_slice(&encoded);
            }
            NumericResultEvidence::Enclosure(value) => {
                bytes.push(0);
                append_interval(&mut bytes, value)?;
            }
            NumericResultEvidence::ExactBoolean(value) => {
                bytes.push(1);
                bytes.push(u8::from(*value));
            }
            NumericResultEvidence::IeeeBits(value) => {
                bytes.push(2);
                bytes.extend_from_slice(&value.bits().to_be_bytes());
            }
        }
        append_interval(&mut bytes, &self.error)?;
        bytes.push(flags(self.flags));
        bytes.extend_from_slice(&self.work_steps.to_be_bytes());
        bytes.extend_from_slice(&self.storage_bytes.to_be_bytes());
        bytes.extend_from_slice(&self.required_limb_capacity.to_be_bytes());
        bytes.extend_from_slice(&self.required_symbolic_nodes.to_be_bytes());
        bytes.extend_from_slice(&(self.steps.len() as u16).to_be_bytes());
        for step in &self.steps {
            append_operands(&mut bytes, step.node, &step.operands)?;
            bytes.extend_from_slice(&step.output.to_be_bytes());
            bytes.push(flags(step.flags));
        }
        bytes.extend_from_slice(&(self.boolean_steps.len() as u16).to_be_bytes());
        for step in &self.boolean_steps {
            append_operands(&mut bytes, step.node, &step.operands)?;
            bytes.push(u8::from(step.output));
            bytes.push(flags(step.flags));
        }
        bytes.extend_from_slice(&(self.integer_steps.len() as u16).to_be_bytes());
        for step in &self.integer_steps {
            let bits = step
                .operands
                .iter()
                .map(|value| *value as u64)
                .collect::<Vec<_>>();
            append_operands(&mut bytes, step.node, &bits)?;
            bytes.extend_from_slice(&step.output.to_be_bytes());
        }
        if bytes.len() > MAX_BYTES {
            return Err(NumericRefusal::StorageBudget.into());
        }
        Ok(bytes)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, NumericEngineRefusal> {
        if bytes.len() > MAX_BYTES {
            return Err(NumericRefusal::StorageBudget.into());
        }
        let mut cursor = Cursor { bytes, offset: 0 };
        if cursor.take(b"conduit.numeric-certificate.v1".len())?
            != b"conduit.numeric-certificate.v1"
        {
            return Err(NumericRefusal::UnsupportedVersion.into());
        }
        let engine_identity = cursor.array()?;
        let graph_identity = cursor.array()?;
        let requirement_identity = cursor.array()?;
        let algorithm = match cursor.byte()? {
            0 => NumericAlgorithm::OrderedRationalV1,
            1 => NumericAlgorithm::OrderedSoftwareBinary64V1,
            2 => NumericAlgorithm::CertifiedRationalIntervalV1,
            3 => NumericAlgorithm::ExactComparisonV1,
            4 => NumericAlgorithm::OrderedIeeeSoftwareBinary64V1,
            5 => NumericAlgorithm::OrderedCheckedI64V1,
            6 => NumericAlgorithm::SymbolicRetentionV1,
            _ => return Err(NumericRefusal::UnsupportedVersion.into()),
        };
        let result = match cursor.byte()? {
            0 => NumericResultEvidence::Enclosure(cursor.interval()?),
            1 => NumericResultEvidence::ExactBoolean(cursor.boolean()?),
            3 => NumericResultEvidence::ExactMathematical(cursor.number()?),
            4 => {
                let length = u32::from_be_bytes(cursor.array()?) as usize;
                if length > MAX_BYTES {
                    return Err(NumericRefusal::StorageBudget.into());
                }
                let value = crate::Quantity::decode(cursor.take(length)?)
                    .map_err(|_| NumericEngineRefusal::InvalidComputation)?;
                NumericResultEvidence::ExactPhysical(alloc::boxed::Box::new(value))
            }
            2 => NumericResultEvidence::IeeeBits(Binary64Value::from_bits(u64::from_be_bytes(
                cursor.array()?,
            ))),
            _ => return Err(NumericRefusal::MalformedEncoding.into()),
        };
        let error = cursor.interval()?;
        let flags = decode_flags(cursor.byte()?)?;
        let work_steps = u64::from_be_bytes(cursor.array()?);
        let storage_bytes = u32::from_be_bytes(cursor.array()?);
        let required_limb_capacity = u16::from_be_bytes(cursor.array()?);
        let required_symbolic_nodes = u16::from_be_bytes(cursor.array()?);
        let count = cursor.count()?;
        let mut steps = Vec::with_capacity(count);
        for _ in 0..count {
            let (node, operands) = cursor.operands()?;
            steps.push(Binary64StepEvidence {
                node,
                operands,
                output: u64::from_be_bytes(cursor.array()?),
                flags: decode_flags(cursor.byte()?)?,
            });
        }
        let count = cursor.count()?;
        let mut boolean_steps = Vec::with_capacity(count);
        for _ in 0..count {
            let (node, operands) = cursor.operands()?;
            boolean_steps.push(BooleanStepEvidence {
                node,
                operands,
                output: cursor.boolean()?,
                flags: decode_flags(cursor.byte()?)?,
            });
        }
        let count = cursor.count()?;
        let mut integer_steps = Vec::with_capacity(count);
        for _ in 0..count {
            let (node, bits) = cursor.operands()?;
            integer_steps.push(I64StepEvidence {
                node,
                operands: bits.into_iter().map(|bits| bits as i64).collect(),
                output: i64::from_be_bytes(cursor.array()?),
            });
        }
        if cursor.offset != bytes.len() {
            return Err(NumericRefusal::NonCanonical.into());
        }
        Ok(Self {
            engine_identity,
            graph_identity,
            requirement_identity,
            algorithm,
            result,
            error,
            steps,
            boolean_steps,
            integer_steps,
            flags,
            work_steps,
            storage_bytes,
            required_limb_capacity,
            required_symbolic_nodes,
        })
    }
}
fn append_operands(
    bytes: &mut Vec<u8>,
    node: u16,
    operands: &[u64],
) -> Result<(), NumericEngineRefusal> {
    if operands.len() > 2 {
        return Err(NumericRefusal::MalformedEncoding.into());
    }
    bytes.extend_from_slice(&node.to_be_bytes());
    bytes.push(operands.len() as u8);
    for bits in operands {
        bytes.extend_from_slice(&bits.to_be_bytes());
    }
    Ok(())
}
struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], NumericEngineRefusal> {
        let end = self
            .offset
            .checked_add(n)
            .ok_or(NumericRefusal::MalformedEncoding)?;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or(NumericRefusal::MalformedEncoding)?;
        self.offset = end;
        Ok(slice)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], NumericEngineRefusal> {
        self.take(N)?
            .try_into()
            .map_err(|_| NumericRefusal::MalformedEncoding.into())
    }
    fn byte(&mut self) -> Result<u8, NumericEngineRefusal> {
        Ok(self.take(1)?[0])
    }
    fn boolean(&mut self) -> Result<bool, NumericEngineRefusal> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(NumericRefusal::NonCanonical.into()),
        }
    }
    fn count(&mut self) -> Result<usize, NumericEngineRefusal> {
        let n = usize::from(u16::from_be_bytes(self.array()?));
        if n > crate::MAX_NUMERIC_COMPUTATION_NODES {
            return Err(NumericRefusal::NodeBudget.into());
        }
        Ok(n)
    }
    fn number(&mut self) -> Result<MathValue, NumericEngineRefusal> {
        let n = u32::from_be_bytes(self.array()?) as usize;
        if n > crate::MAX_NUMERIC_ENCODED_BYTES {
            return Err(NumericRefusal::StorageBudget.into());
        }
        Ok(MathValue::decode(self.take(n)?)?)
    }
    fn interval(&mut self) -> Result<RationalInterval, NumericEngineRefusal> {
        Ok(RationalInterval::new(self.number()?, self.number()?)?)
    }
    fn operands(&mut self) -> Result<(u16, Vec<u64>), NumericEngineRefusal> {
        let node = u16::from_be_bytes(self.array()?);
        let n = self.byte()?;
        if n > 2 {
            return Err(NumericRefusal::MalformedEncoding.into());
        }
        let mut operands = Vec::with_capacity(usize::from(n));
        for _ in 0..n {
            operands.push(u64::from_be_bytes(self.array()?));
        }
        Ok((node, operands))
    }
}
