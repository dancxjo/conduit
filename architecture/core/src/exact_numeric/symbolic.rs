//! Finite exact mathematical denotation. The ordinary rational case stays inline;
//! symbolic graphs are prepared explicitly and borrowed unchanged during play.
use super::{ExactNumericRefusal, ExactNumericWork, ExactRational};
use alloc::vec::Vec;

mod codec;
mod enclosure;
pub use codec::MathematicalValueView;
pub use enclosure::*;

pub const MATHEMATICAL_VALUE_ID: &str = "numeric/mathematical-value@1";
pub const MATHEMATICAL_VALUE_MAX_NODES: usize = 32;
pub const MATHEMATICAL_VALUE_MAX_DEPTH: usize = 16;
pub const MATHEMATICAL_VALUE_MAX_ENCODED_BYTES: usize =
    3 + MATHEMATICAL_VALUE_MAX_NODES * (3 + super::EXACT_RATIONAL_MAX_ENCODED_BYTES);
/// Runtime validation is bounded and allocation-free. More demanding encodings
/// need another admitted profile, not an unbounded validator at a Port.
pub const MATHEMATICAL_VALUE_VALIDATION_WORK: u64 = 20_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathematicalValueRefusal {
    Numeric(ExactNumericRefusal),
    NodeBound,
    DepthBound,
    InvalidReference,
    NonCanonicalEncoding,
    UnsupportedVersion,
    NegativeRadicand,
    UnsupportedDomainProof,
}
impl From<ExactNumericRefusal> for MathematicalValueRefusal {
    fn from(value: ExactNumericRefusal) -> Self {
        Self::Numeric(value)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathematicalComparison {
    ProvedEqual,
    ProvedDifferent,
    Unknown,
}

/// References are local to one builder. Decoding admits only strict backward
/// edges, so cycles and unresolved references cannot enter the value contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MathematicalNodeId {
    index: u8,
    digest: [u8; 32],
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// A decoded node uses fixed scratch; boxing its rational would allocate during
// borrowed Port validation. Graph storage is preparation-only and bounded.
#[allow(clippy::large_enum_variant)]
pub enum MathematicalNode {
    Rational(ExactRational),
    Pi,
    /// Exact constructor for 2π; it normalizes into the canonical product.
    Tau,
    Negate(MathematicalNodeId),
    PrincipalSquareRoot(MathematicalNodeId),
    Add(MathematicalNodeId, MathematicalNodeId),
    Subtract(MathematicalNodeId, MathematicalNodeId),
    Multiply(MathematicalNodeId, MathematicalNodeId),
    Divide(MathematicalNodeId, MathematicalNodeId),
}
impl MathematicalNode {
    pub(super) fn references(self) -> [Option<MathematicalNodeId>; 2] {
        match self {
            Self::Rational(_) | Self::Pi | Self::Tau => [None, None],
            Self::Negate(a) | Self::PrincipalSquareRoot(a) => [Some(a), None],
            Self::Add(a, b) | Self::Subtract(a, b) | Self::Multiply(a, b) | Self::Divide(a, b) => {
                [Some(a), Some(b)]
            }
        }
    }
    fn structural_digest(self) -> [u8; 32] {
        let mut bytes = [0; 65];
        bytes[0] = match self {
            Self::Rational(_) => 0,
            Self::Pi => 1,
            Self::Tau => 8,
            Self::Negate(_) => 2,
            Self::PrincipalSquareRoot(_) => 3,
            Self::Add(..) => 4,
            Self::Subtract(..) => 5,
            Self::Multiply(..) => 6,
            Self::Divide(..) => 7,
        };
        if let Self::Rational(q) = self {
            bytes[1..33].copy_from_slice(&q.semantic_digest());
        }
        for (index, id) in self.references().into_iter().flatten().enumerate() {
            bytes[1 + 32 * index..33 + 32 * index].copy_from_slice(&id.digest);
        }
        crate::semantic_digest("numeric/mathematical-node@1", &bytes)
    }
    fn remap(self, ids: &[MathematicalNodeId; MATHEMATICAL_VALUE_MAX_NODES]) -> Self {
        match self {
            Self::Rational(_) | Self::Pi | Self::Tau => self,
            Self::Negate(a) => Self::Negate(ids[a.index as usize]),
            Self::PrincipalSquareRoot(a) => Self::PrincipalSquareRoot(ids[a.index as usize]),
            Self::Add(a, b) => Self::Add(ids[a.index as usize], ids[b.index as usize]),
            Self::Subtract(a, b) => Self::Subtract(ids[a.index as usize], ids[b.index as usize]),
            Self::Multiply(a, b) => Self::Multiply(ids[a.index as usize], ids[b.index as usize]),
            Self::Divide(a, b) => Self::Divide(ids[a.index as usize], ids[b.index as usize]),
        }
    }
}

/// Construction happens before Play. The node/edge/depth profile is finite;
/// interning shares structural subexpressions without a general theorem prover.
#[derive(Debug, Default)]
pub struct MathematicalValueBuilder {
    nodes: Vec<MathematicalNode>,
    depths: Vec<usize>,
    digests: Vec<[u8; 32]>,
}
impl MathematicalValueBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn insert(
        &mut self,
        node: MathematicalNode,
        work: &mut ExactNumericWork,
    ) -> Result<MathematicalNodeId, MathematicalValueRefusal> {
        work.charge((self.nodes.len() + 1) as u64)?;
        if node == MathematicalNode::Tau {
            let two = self.insert(
                MathematicalNode::Rational(ExactRational::from_i128(2)),
                work,
            )?;
            let pi = self.insert(MathematicalNode::Pi, work)?;
            return self.insert(MathematicalNode::Multiply(two, pi), work);
        }
        for id in node.references().into_iter().flatten() {
            self.node(id)?;
        }
        let node = self.normalize(node, work)?;
        if let Some(index) = self.nodes.iter().position(|existing| *existing == node) {
            return Ok(MathematicalNodeId {
                index: index as u8,
                digest: self.digests[index],
            });
        }
        if self.nodes.len() == MATHEMATICAL_VALUE_MAX_NODES {
            return Err(MathematicalValueRefusal::NodeBound);
        }
        let mut depth = 1;
        for child in node.references().into_iter().flatten() {
            depth = depth.max(1 + self.depths[child.index as usize]);
        }
        if depth > MATHEMATICAL_VALUE_MAX_DEPTH {
            return Err(MathematicalValueRefusal::DepthBound);
        }
        self.depths.push(depth);
        let digest = node.structural_digest();
        let id = MathematicalNodeId {
            index: self.nodes.len() as u8,
            digest,
        };
        self.digests.push(digest);
        self.nodes.push(node);
        Ok(id)
    }
    fn node(&self, id: MathematicalNodeId) -> Result<MathematicalNode, MathematicalValueRefusal> {
        if self.digests.get(id.index as usize) != Some(&id.digest) {
            return Err(MathematicalValueRefusal::InvalidReference);
        }
        self.nodes
            .get(id.index as usize)
            .copied()
            .ok_or(MathematicalValueRefusal::InvalidReference)
    }
    fn rational(&self, id: MathematicalNodeId) -> Option<ExactRational> {
        match self.nodes.get(id.index as usize)? {
            MathematicalNode::Rational(q) => Some(*q),
            _ => None,
        }
    }
    fn normalize(
        &self,
        node: MathematicalNode,
        work: &mut ExactNumericWork,
    ) -> Result<MathematicalNode, MathematicalValueRefusal> {
        use MathematicalNode as N;
        match node {
            N::Negate(a) => {
                if let Some(q) = self.rational(a) {
                    return Ok(N::Rational(q.negated()));
                }
            }
            N::PrincipalSquareRoot(a) => {
                let q = self
                    .rational(a)
                    .ok_or(MathematicalValueRefusal::UnsupportedDomainProof)?;
                if q.is_negative() {
                    return Err(MathematicalValueRefusal::NegativeRadicand);
                }
                if q == ExactRational::ZERO || q == ExactRational::ONE {
                    return Ok(N::Rational(q));
                }
            }
            N::Add(a, b) | N::Subtract(a, b) | N::Multiply(a, b) | N::Divide(a, b) => {
                if matches!(node, N::Divide(..)) {
                    let q = self
                        .rational(b)
                        .ok_or(MathematicalValueRefusal::UnsupportedDomainProof)?;
                    if q == ExactRational::ZERO {
                        return Err(ExactNumericRefusal::DivisionByZero.into());
                    }
                }
                if let (Some(left), Some(right)) = (self.rational(a), self.rational(b)) {
                    return Ok(N::Rational(match node {
                        N::Add(..) => left.checked_add(right, work)?,
                        N::Subtract(..) => left.checked_subtract(right, work)?,
                        N::Multiply(..) => left.checked_multiply(right, work)?,
                        N::Divide(..) => left.checked_divide(right, work)?,
                        _ => unreachable!(),
                    }));
                }
            }
            _ => {}
        }
        Ok(node)
    }
    pub fn finish(
        &self,
        root: MathematicalNodeId,
        work: &mut ExactNumericWork,
    ) -> Result<MathematicalValue, MathematicalValueRefusal> {
        if let MathematicalNode::Rational(q) = self.node(root)? {
            return Ok(MathematicalValue::rational(q));
        }
        let mut nodes = Vec::new();
        let mut ids = [MathematicalNodeId {
            index: 0,
            digest: [0; 32],
        }; MATHEMATICAL_VALUE_MAX_NODES];
        let mut seen = [false; MATHEMATICAL_VALUE_MAX_NODES];
        self.visit(root, 1, &mut nodes, &mut ids, &mut seen, work)?;
        Ok(MathematicalValue {
            representation: Representation::Expression(nodes),
        })
    }
    fn visit(
        &self,
        id: MathematicalNodeId,
        depth: usize,
        output: &mut Vec<MathematicalNode>,
        ids: &mut [MathematicalNodeId; MATHEMATICAL_VALUE_MAX_NODES],
        seen: &mut [bool; MATHEMATICAL_VALUE_MAX_NODES],
        work: &mut ExactNumericWork,
    ) -> Result<(), MathematicalValueRefusal> {
        work.charge(1)?;
        if depth > MATHEMATICAL_VALUE_MAX_DEPTH {
            return Err(MathematicalValueRefusal::DepthBound);
        }
        let node = self.node(id)?;
        if seen[id.index as usize] {
            return Ok(());
        }
        for child in node.references().into_iter().flatten() {
            self.visit(child, depth + 1, output, ids, seen, work)?;
        }
        ids[id.index as usize] = MathematicalNodeId {
            index: output.len() as u8,
            digest: id.digest,
        };
        output.push(node.remap(ids));
        seen[id.index as usize] = true;
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
// The rational-only case deliberately needs no heap allocation or graph.
#[allow(clippy::large_enum_variant)]
enum Representation {
    Rational(ExactRational),
    Expression(Vec<MathematicalNode>),
}
/// Structural equality is deliberately not a general mathematical decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MathematicalValue {
    representation: Representation,
}
impl MathematicalValue {
    pub const fn rational(value: ExactRational) -> Self {
        Self {
            representation: Representation::Rational(value),
        }
    }
    /// Closed literals only; expression construction retains its checked DAG.
    pub fn parse_literal(
        literal: &str,
        work: &mut ExactNumericWork,
    ) -> Result<Self, MathematicalValueRefusal> {
        if matches!(literal, "pi" | "π" | "tau" | "τ") {
            let mut builder = MathematicalValueBuilder::new();
            let node = if matches!(literal, "tau" | "τ") {
                MathematicalNode::Tau
            } else {
                MathematicalNode::Pi
            };
            let root = builder.insert(node, work)?;
            builder.finish(root, work)
        } else {
            Ok(Self::rational(ExactRational::parse_decimal(literal, work)?))
        }
    }
    pub fn as_rational(&self) -> Option<ExactRational> {
        match self.representation {
            Representation::Rational(q) => Some(q),
            _ => None,
        }
    }
    pub fn compare(
        &self,
        other: &Self,
        work: &mut ExactNumericWork,
    ) -> Result<MathematicalComparison, MathematicalValueRefusal> {
        if self == other {
            return Ok(MathematicalComparison::ProvedEqual);
        }
        match (self.as_rational(), other.as_rational()) {
            (Some(a), Some(b)) => Ok(if a.checked_compare(b, work)?.is_eq() {
                MathematicalComparison::ProvedEqual
            } else {
                MathematicalComparison::ProvedDifferent
            }),
            _ => Ok(MathematicalComparison::Unknown),
        }
    }
    pub fn encode(&self, output: &mut [u8]) -> Result<usize, MathematicalValueRefusal> {
        codec::encode(self, output)
    }
    pub fn decode(
        bytes: &[u8],
        work: &mut ExactNumericWork,
    ) -> Result<Self, MathematicalValueRefusal> {
        let view = MathematicalValueView::decode(bytes, work)?;
        if let Some(q) = view.as_rational(work)? {
            return Ok(Self::rational(q));
        }
        let mut nodes = Vec::with_capacity(view.node_count());
        for index in 0..view.node_count() {
            nodes.push(view.node(index, work)?);
        }
        Ok(Self {
            representation: Representation::Expression(nodes),
        })
    }
    pub fn semantic_digest(&self) -> [u8; 32] {
        // Preparation-only owned serialization; primitive transport inspection
        // uses the borrowed view and never allocates a graph.
        let mut bytes = alloc::vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
        let length = self.encode(&mut bytes).expect("bounded symbolic codec");
        crate::semantic_digest(MATHEMATICAL_VALUE_ID, &bytes[..length])
    }
}
