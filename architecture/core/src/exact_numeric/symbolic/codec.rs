//! Canonical postorder DAG codec and allocation-free borrowed validation.
use super::MathematicalValueRefusal as R;
use super::*;

pub(super) fn encode(value: &MathematicalValue, output: &mut [u8]) -> Result<usize, R> {
    let nodes: &[MathematicalNode] = match &value.representation {
        Representation::Rational(q) => {
            return encode_rational(q, output);
        }
        Representation::Expression(nodes) => nodes,
    };
    encode_nodes(nodes, output)
}
pub(super) fn encode_rational(q: &ExactRational, output: &mut [u8]) -> Result<usize, R> {
    if output.len() < 2 {
        return Err(ExactNumericRefusal::EncodingBufferTooSmall.into());
    }
    output[..2].copy_from_slice(&[1, 0]);
    Ok(2 + q.encode(&mut output[2..])?)
}
pub(super) fn encode_nodes(nodes: &[MathematicalNode], output: &mut [u8]) -> Result<usize, R> {
    if output.len() < 3 {
        return Err(ExactNumericRefusal::EncodingBufferTooSmall.into());
    }
    output[..3].copy_from_slice(&[1, 1, nodes.len() as u8]);
    let mut offset = 3;
    for node in nodes {
        let mut bytes = [0; 3 + super::super::EXACT_RATIONAL_MAX_ENCODED_BYTES];
        let length = match *node {
            MathematicalNode::Rational(q) => {
                bytes[0] = 0;
                let length = q.encode(&mut bytes[3..])?;
                bytes[1..3].copy_from_slice(&(length as u16).to_le_bytes());
                length + 3
            }
            MathematicalNode::Tau => unreachable!("tau constructor normalizes to 2π"),
            MathematicalNode::Pi => {
                bytes[0] = 1;
                1
            }
            MathematicalNode::SineTurns(a)
            | MathematicalNode::CosineTurns(a)
            | MathematicalNode::TangentTurns(a)
            | MathematicalNode::ArctangentTurns(a)
            | MathematicalNode::ArcsineTurns(a)
            | MathematicalNode::ArccosineTurns(a) => {
                let tag = match node {
                    MathematicalNode::SineTurns(_) => 9,
                    MathematicalNode::CosineTurns(_) => 10,
                    MathematicalNode::TangentTurns(_) => 11,
                    MathematicalNode::ArctangentTurns(_) => 12,
                    MathematicalNode::ArcsineTurns(_) => 13,
                    _ => 14,
                };
                bytes[..2].copy_from_slice(&[tag, a.index]);
                2
            }
            MathematicalNode::Negate(a) => {
                bytes[..2].copy_from_slice(&[2, a.index]);
                2
            }
            MathematicalNode::PrincipalSquareRoot(a) => {
                bytes[..2].copy_from_slice(&[3, a.index]);
                2
            }
            MathematicalNode::Add(a, b) => {
                bytes[..3].copy_from_slice(&[4, a.index, b.index]);
                3
            }
            MathematicalNode::Subtract(a, b) => {
                bytes[..3].copy_from_slice(&[5, a.index, b.index]);
                3
            }
            MathematicalNode::Multiply(a, b) => {
                bytes[..3].copy_from_slice(&[6, a.index, b.index]);
                3
            }
            MathematicalNode::Divide(a, b) => {
                bytes[..3].copy_from_slice(&[7, a.index, b.index]);
                3
            }
        };
        let dest = output
            .get_mut(offset..offset + length)
            .ok_or(ExactNumericRefusal::EncodingBufferTooSmall)?;
        dest.copy_from_slice(&bytes[..length]);
        offset += length;
    }
    Ok(offset)
}

/// The encoded graph remains borrowed. Its bounded tables contain offsets, not
/// allocated numerical nodes. Decoding proves normalized literals, checked
/// domains, strict backward references, sharing and one canonical traversal.
#[derive(Debug, Clone, Copy)]
pub struct MathematicalValueView<'a> {
    bytes: &'a [u8],
    count: usize,
    offsets: [u16; MATHEMATICAL_VALUE_MAX_NODES],
    lengths: [u16; MATHEMATICAL_VALUE_MAX_NODES],
    digests: [[u8; 32]; MATHEMATICAL_VALUE_MAX_NODES],
}
impl<'a> MathematicalValueView<'a> {
    pub fn decode(bytes: &'a [u8], work: &mut ExactNumericWork) -> Result<Self, R> {
        if bytes.len() < 2 || bytes.len() > MATHEMATICAL_VALUE_MAX_ENCODED_BYTES {
            return Err(R::NonCanonicalEncoding);
        }
        if bytes[0] != 1 {
            return Err(R::UnsupportedVersion);
        }
        let mut value = Self {
            bytes,
            count: 1,
            offsets: [0; MATHEMATICAL_VALUE_MAX_NODES],
            lengths: [0; MATHEMATICAL_VALUE_MAX_NODES],
            digests: [[0; 32]; MATHEMATICAL_VALUE_MAX_NODES],
        };
        if bytes[1] == 0 {
            ExactRational::decode(&bytes[2..], work)?;
            value.offsets[0] = 2;
            value.lengths[0] = (bytes.len() - 2) as u16;
            return Ok(value);
        }
        if bytes[1] != 1 {
            return Err(R::NonCanonicalEncoding);
        }
        value.count = usize::from(*bytes.get(2).ok_or(R::NonCanonicalEncoding)?);
        if value.count == 0 || value.count > MATHEMATICAL_VALUE_MAX_NODES {
            return Err(R::NodeBound);
        }
        let mut offset = 3;
        let mut depths = [0; MATHEMATICAL_VALUE_MAX_NODES];
        for index in 0..value.count {
            work.charge((index + 1) as u64)?;
            let tag = *bytes.get(offset).ok_or(R::NonCanonicalEncoding)?;
            let length = match tag {
                0 => {
                    let b = bytes
                        .get(offset + 1..offset + 3)
                        .ok_or(R::NonCanonicalEncoding)?;
                    3 + usize::from(u16::from_le_bytes([b[0], b[1]]))
                }
                1 => 1,
                2 | 3 | 9..=14 => 2,
                4..=7 => 3,
                _ => return Err(R::NonCanonicalEncoding),
            };
            bytes
                .get(offset..offset + length)
                .ok_or(R::NonCanonicalEncoding)?;
            value.offsets[index] = offset as u16;
            value.lengths[index] = length as u16;
            let node = value.node(index, work)?;
            let mut depth = 1;
            for child in node.references().into_iter().flatten() {
                if child.index as usize >= index {
                    return Err(R::InvalidReference);
                }
                depth = depth.max(1 + depths[child.index as usize]);
            }
            if depth > MATHEMATICAL_VALUE_MAX_DEPTH {
                return Err(R::DepthBound);
            }
            depths[index] = depth;
            value.validate_normal_form(node, work)?;
            value.digests[index] = node.structural_digest();
            for prior in 0..index {
                if value.encoded_node(prior) == value.encoded_node(index) {
                    return Err(R::NonCanonicalEncoding);
                }
            }
            offset += length;
        }
        if offset != bytes.len()
            || matches!(
                value.node(value.count - 1, work)?,
                MathematicalNode::Rational(_)
            )
        {
            return Err(R::NonCanonicalEncoding);
        }
        let mut seen = [false; MATHEMATICAL_VALUE_MAX_NODES];
        let mut next = 0;
        value.visit(value.count - 1, &mut seen, &mut next, work)?;
        if next != value.count {
            return Err(R::NonCanonicalEncoding);
        }
        Ok(value)
    }
    fn encoded_node(&self, index: usize) -> &'a [u8] {
        let start = self.offsets[index] as usize;
        &self.bytes[start..start + self.lengths[index] as usize]
    }
    fn visit(
        &self,
        index: usize,
        seen: &mut [bool; MATHEMATICAL_VALUE_MAX_NODES],
        next: &mut usize,
        work: &mut ExactNumericWork,
    ) -> Result<(), R> {
        work.charge(1)?;
        if seen[index] {
            return Ok(());
        }
        for child in self.node(index, work)?.references().into_iter().flatten() {
            self.visit(child.index as usize, seen, next, work)?;
        }
        if *next != index {
            return Err(R::NonCanonicalEncoding);
        }
        *next += 1;
        seen[index] = true;
        Ok(())
    }
    fn rational_node(
        &self,
        id: MathematicalNodeId,
        work: &mut ExactNumericWork,
    ) -> Result<Option<ExactRational>, R> {
        Ok(match self.node(id.index as usize, work)? {
            MathematicalNode::Rational(q) => Some(q),
            _ => None,
        })
    }
    fn validate_normal_form(
        &self,
        node: MathematicalNode,
        work: &mut ExactNumericWork,
    ) -> Result<(), R> {
        use MathematicalNode as N;
        match node {
            N::ArctangentTurns(a) | N::ArcsineTurns(a) | N::ArccosineTurns(a) => {
                let q = self
                    .rational_node(a, work)?
                    .ok_or(R::UnsupportedDomainProof)?;
                if super::inverse_trigonometry::cardinal(node, q, work)?.is_some() {
                    return Err(R::NonCanonicalEncoding);
                }
            }
            N::SineTurns(a) | N::CosineTurns(a) | N::TangentTurns(a) => {
                if let Some(q) = self.rational_node(a, work)? {
                    if q.reduced_turn(work)? != q
                        || super::trigonometry::cardinal(node, q, work)?.is_some()
                    {
                        return Err(R::NonCanonicalEncoding);
                    }
                } else if matches!(node, N::TangentTurns(_)) {
                    return Err(R::UnsupportedDomainProof);
                }
            }
            N::Negate(a) if self.rational_node(a, work)?.is_some() => {
                return Err(R::NonCanonicalEncoding)
            }
            N::PrincipalSquareRoot(a) => {
                let q = self
                    .rational_node(a, work)?
                    .ok_or(R::UnsupportedDomainProof)?;
                if q.is_negative() {
                    return Err(R::NegativeRadicand);
                }
                if q == ExactRational::ZERO || q == ExactRational::ONE {
                    return Err(R::NonCanonicalEncoding);
                }
            }
            N::Add(a, b) | N::Subtract(a, b) | N::Multiply(a, b) | N::Divide(a, b) => {
                let left = self.rational_node(a, work)?;
                let right = self.rational_node(b, work)?;
                if matches!(node, N::Divide(..)) {
                    let pi = right.is_none() && matches!(self.node(b.index as usize, work)?, N::Pi);
                    require_divisor(right, pi)?;
                }
                if left.is_some() && right.is_some() {
                    return Err(R::NonCanonicalEncoding);
                }
            }
            _ => {}
        }
        Ok(())
    }
    pub fn as_rational(&self, work: &mut ExactNumericWork) -> Result<Option<ExactRational>, R> {
        if self.bytes[1] == 0 {
            Ok(Some(ExactRational::decode(&self.bytes[2..], work)?))
        } else {
            Ok(None)
        }
    }
    pub const fn node_count(&self) -> usize {
        self.count
    }
    pub fn node(&self, index: usize, work: &mut ExactNumericWork) -> Result<MathematicalNode, R> {
        if index >= self.count {
            return Err(R::InvalidReference);
        }
        if self.bytes[1] == 0 {
            return Ok(MathematicalNode::Rational(ExactRational::decode(
                &self.bytes[2..],
                work,
            )?));
        }
        let bytes = self.encoded_node(index);
        Ok(match bytes[0] {
            0 => MathematicalNode::Rational(ExactRational::decode(&bytes[3..], work)?),
            1 => MathematicalNode::Pi,
            9 => MathematicalNode::SineTurns(self.reference(bytes[1])?),
            10 => MathematicalNode::CosineTurns(self.reference(bytes[1])?),
            11 => MathematicalNode::TangentTurns(self.reference(bytes[1])?),
            12 => MathematicalNode::ArctangentTurns(self.reference(bytes[1])?),
            13 => MathematicalNode::ArcsineTurns(self.reference(bytes[1])?),
            14 => MathematicalNode::ArccosineTurns(self.reference(bytes[1])?),
            2 => MathematicalNode::Negate(self.reference(bytes[1])?),
            3 => MathematicalNode::PrincipalSquareRoot(self.reference(bytes[1])?),
            4 => MathematicalNode::Add(self.reference(bytes[1])?, self.reference(bytes[2])?),
            5 => MathematicalNode::Subtract(self.reference(bytes[1])?, self.reference(bytes[2])?),
            6 => MathematicalNode::Multiply(self.reference(bytes[1])?, self.reference(bytes[2])?),
            7 => MathematicalNode::Divide(self.reference(bytes[1])?, self.reference(bytes[2])?),
            _ => return Err(R::NonCanonicalEncoding),
        })
    }
    fn reference(&self, index: u8) -> Result<MathematicalNodeId, R> {
        if index as usize >= self.count {
            return Err(R::InvalidReference);
        }
        Ok(MathematicalNodeId {
            index,
            digest: self.digests[index as usize],
        })
    }
    pub const fn encoded(&self) -> &'a [u8] {
        self.bytes
    }
    pub fn semantic_digest(&self) -> [u8; 32] {
        crate::semantic_digest(MATHEMATICAL_VALUE_ID, self.bytes)
    }
    pub fn compare(
        &self,
        other: &Self,
        work: &mut ExactNumericWork,
    ) -> Result<MathematicalComparison, R> {
        if self.bytes == other.bytes {
            return Ok(MathematicalComparison::ProvedEqual);
        }
        match (self.as_rational(work)?, other.as_rational(work)?) {
            (Some(a), Some(b)) => Ok(if a.checked_compare(b, work)?.is_eq() {
                MathematicalComparison::ProvedEqual
            } else {
                MathematicalComparison::ProvedDifferent
            }),
            _ => Ok(MathematicalComparison::Unknown),
        }
    }
}
