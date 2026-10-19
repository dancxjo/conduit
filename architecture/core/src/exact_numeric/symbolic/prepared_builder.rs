//! Reuse canonical construction and encoding with all vector capacity prepared.
use super::*;

/// Bounded construction of exact mathematical data, not executable code or a
/// floating-point algorithm witness. Canonical normalization is unchanged.
#[derive(Debug)]
pub struct PreparedMathematicalValueBuilder {
    builder: MathematicalValueBuilder,
    canonical: Vec<MathematicalNode>,
    encoding: Vec<u8>,
    retained_payload_bytes: usize,
}
impl PreparedMathematicalValueBuilder {
    /// Allocate the existing 32-node/depth-16 profile and a finite encoding
    /// buffer before execution. The conservative payload bound excludes
    /// allocator overhead, stack frames and caller-owned output storage.
    pub fn new(
        maximum_retained_payload_bytes: u32,
        maximum_encoded_bytes: usize,
    ) -> Result<Self, MathematicalValueRefusal> {
        let extent = || MathematicalValueRefusal::Numeric(ExactNumericRefusal::ExtentExceeded);
        if !(4..=MATHEMATICAL_VALUE_MAX_ENCODED_BYTES).contains(&maximum_encoded_bytes) {
            return Err(extent());
        }
        let retained_payload_bytes = Self::payload_bytes(
            MATHEMATICAL_VALUE_MAX_NODES,
            MATHEMATICAL_VALUE_MAX_NODES,
            MATHEMATICAL_VALUE_MAX_NODES,
            MATHEMATICAL_VALUE_MAX_NODES,
            maximum_encoded_bytes,
        )
        .ok_or_else(extent)?;
        if retained_payload_bytes > maximum_retained_payload_bytes as usize {
            return Err(extent());
        }
        let mut value = Self {
            builder: MathematicalValueBuilder {
                nodes: Vec::with_capacity(MATHEMATICAL_VALUE_MAX_NODES),
                depths: Vec::with_capacity(MATHEMATICAL_VALUE_MAX_NODES),
                digests: Vec::with_capacity(MATHEMATICAL_VALUE_MAX_NODES),
            },
            canonical: Vec::with_capacity(MATHEMATICAL_VALUE_MAX_NODES),
            encoding: alloc::vec![0; maximum_encoded_bytes],
            retained_payload_bytes,
        };
        // The allocator may reserve more vector capacity than requested. Account
        // for the actual recorded capacities before exposing the prepared state.
        value.retained_payload_bytes = Self::payload_bytes(
            value.builder.nodes.capacity(),
            value.canonical.capacity(),
            value.builder.depths.capacity(),
            value.builder.digests.capacity(),
            value.encoding.capacity(),
        )
        .ok_or_else(extent)?;
        if value.retained_payload_bytes > maximum_retained_payload_bytes as usize {
            return Err(extent());
        }
        Ok(value)
    }
    fn payload_bytes(
        nodes: usize,
        canonical: usize,
        depths: usize,
        digests: usize,
        encoding: usize,
    ) -> Option<usize> {
        let graph = nodes
            .checked_add(canonical)?
            .checked_mul(core::mem::size_of::<MathematicalNode>())?;
        graph
            .checked_add(depths.checked_mul(core::mem::size_of::<usize>())?)?
            .checked_add(digests.checked_mul(core::mem::size_of::<[u8; 32]>())?)?
            .checked_add(encoding)?
            .checked_add(core::mem::size_of::<Self>())
    }
    pub const fn retained_payload_bytes(&self) -> usize {
        self.retained_payload_bytes
    }
    /// Clear the current construction while retaining every admitted buffer.
    /// IDs are checked against the current graph's index and semantic digest;
    /// they are value references, not builder-instance authority tokens.
    pub fn reset(&mut self) {
        self.builder.nodes.clear();
        self.builder.depths.clear();
        self.builder.digests.clear();
        self.canonical.clear();
    }
    pub fn insert(
        &mut self,
        node: MathematicalNode,
        work: &mut ExactNumericWork,
    ) -> Result<MathematicalNodeId, MathematicalValueRefusal> {
        self.builder.insert(node, work)
    }
    /// Produce exactly the ordinary builder's canonical bytes. Errors leave
    /// caller output untouched. Encoding never grows a prepared vector.
    pub fn encode(
        &mut self,
        root: MathematicalNodeId,
        output: &mut [u8],
        work: &mut ExactNumericWork,
    ) -> Result<usize, MathematicalValueRefusal> {
        let length = if let MathematicalNode::Rational(q) = self.builder.node(root)? {
            codec::encode_rational(&q, &mut self.encoding)?
        } else {
            self.canonical.clear();
            let mut ids = [MathematicalNodeId {
                index: 0,
                digest: [0; 32],
            }; MATHEMATICAL_VALUE_MAX_NODES];
            let mut seen = [false; MATHEMATICAL_VALUE_MAX_NODES];
            self.builder
                .visit(root, 1, &mut self.canonical, &mut ids, &mut seen, work)?;
            codec::encode_nodes(&self.canonical, &mut self.encoding)?
        };
        let target = output
            .get_mut(..length)
            .ok_or(ExactNumericRefusal::EncodingBufferTooSmall)?;
        target.copy_from_slice(&self.encoding[..length]);
        Ok(length)
    }
}
