//! Borrowed qualification only: the existing decoder remains validity authority.
//! This scanner recognizes exactly the canonical unpredicated Some(Ref Input)
//! program, before any untrusted Type, node, String or byte Vec is allocated.
use super::owned_storage::{QualifiedExpressionStorage, QualifiedExpressionStorageRefusal as E};
use crate::{PortableExpressionNode, PortableExpressionProgram};
use conduit_core::{
    StructuredInfoType, StructuredVariantCase, EMPTY_INFO_ID, RESOURCE_REFERENCE_INFO_ID,
};
use core::mem::size_of;
const OPTIONAL: &str = "conduit.conduitese.optional.v1";
const HEADER: &[u8] = b"conduit.pure-expression.program.v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QualifiedExpressionProgramStorage {
    /// Peak requested material for decode OR evaluator preparation, including
    /// the decoded program retained during evaluator preparation. Excludes the
    /// caller's existing configuration Text and allocator metadata.
    pub preparation_bytes: usize,
    /// Inline Program plus every decoded Type/name/case/node allocation.
    pub decoded_program_bytes: usize,
    pub evaluator: QualifiedExpressionStorage,
    pub maximum_input_bytes: u32,
    pub maximum_output_bytes: u32,
}
struct Hex<'a> {
    remaining: &'a [u8],
}
impl Hex<'_> {
    fn byte(&mut self) -> Result<u8, E> {
        let (pair, rest) = self
            .remaining
            .split_at_checked(2)
            .ok_or(E::UnsupportedProgram)?;
        self.remaining = rest;
        let digit = |b| match b {
            b'0'..=b'9' => Ok(b - b'0'),
            b'a'..=b'f' => Ok(b - b'a' + 10),
            _ => Err(E::UnsupportedProgram),
        };
        Ok(digit(pair[0])? * 16 + digit(pair[1])?)
    }
    fn expect(&mut self, bytes: &[u8]) -> Result<(), E> {
        for &byte in bytes {
            if self.byte()? != byte {
                return Err(E::UnsupportedProgram);
            }
        }
        Ok(())
    }
    fn text32(&mut self, text: &str) -> Result<(), E> {
        self.expect(&(text.len() as u32).to_le_bytes())?;
        self.expect(text.as_bytes())
    }
    fn leaf(&mut self, name: &str) -> Result<(), E> {
        self.expect(&[0])?;
        self.text32(name)
    }
    fn ty(&mut self, optional: bool) -> Result<(), E> {
        let length = if optional {
            optional_prefix()
        } else {
            leaf_prefix()
        };
        self.expect(&(length as u64).to_le_bytes())?;
        if optional {
            self.expect(&[3])?;
            self.text32(OPTIONAL)?;
            self.expect(&2u32.to_le_bytes())?;
            self.text32("none")?;
            self.leaf(EMPTY_INFO_ID)?;
            self.text32("some")?;
        }
        self.leaf(RESOURCE_REFERENCE_INFO_ID)
    }
}
fn leaf_prefix() -> usize {
    5 + RESOURCE_REFERENCE_INFO_ID.len()
}
fn optional_prefix() -> usize {
    9 + OPTIONAL.len() + 8 + 5 + EMPTY_INFO_ID.len() + 8 + leaf_prefix()
}
impl QualifiedExpressionProgramStorage {
    pub fn resource_reference_some_hex(encoded: &str) -> Result<Self, E> {
        let mut cursor = Hex {
            remaining: encoded.as_bytes(),
        };
        cursor.expect(&HEADER[..HEADER.len() - 1])?;
        let maximum = match cursor.byte()? {
            b'2' => None,
            b'3' => {
                let mut bytes = [0; 4];
                for byte in &mut bytes {
                    *byte = cursor.byte()?;
                }
                let maximum = u32::from_le_bytes(bytes);
                if maximum as usize > conduit_core::MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES {
                    return Err(E::UnsupportedProgram);
                }
                Some(maximum)
            }
            _ => return Err(E::UnsupportedProgram),
        };
        cursor.ty(false)?;
        cursor.ty(true)?;
        cursor.ty(true)?;
        cursor.expect(&[9])?;
        cursor.expect(&4u64.to_le_bytes())?;
        cursor.expect(b"some")?;
        cursor.ty(false)?;
        cursor.expect(&[0])?;
        if !cursor.remaining.is_empty() {
            return Err(E::UnsupportedProgram);
        }
        let evaluator = QualifiedExpressionStorage::resource_reference_some_model(maximum);
        let optional_heap = StructuredInfoType::compound_backing_bytes()
            + OPTIONAL.len()
            + 2 * size_of::<StructuredVariantCase>()
            + 8
            + EMPTY_INFO_ID.len()
            + RESOURCE_REFERENCE_INFO_ID.len();
        let decoded_program_bytes = size_of::<PortableExpressionProgram>()
            + 2 * RESOURCE_REFERENCE_INFO_ID.len()
            + 2 * optional_heap
            + 4
            + size_of::<PortableExpressionNode>();
        // Hex decoder byte Vec, canonical reserialization Vec, and its largest
        // single temporary Type-prefix serialization all coexist with Program.
        let decode = decoded_program_bytes + encoded.len() + optional_prefix();
        let prepare = decoded_program_bytes + evaluator.preparation_bytes;
        Ok(Self {
            preparation_bytes: decode.max(prepare),
            decoded_program_bytes,
            evaluator,
            maximum_input_bytes: maximum
                .unwrap_or(conduit_core::MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES as u32),
            maximum_output_bytes: (optional_prefix()
                + 14
                + maximum.map_or(
                    conduit_core::MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES,
                    |n| n as usize,
                )) as u32,
        })
    }
}
impl PortableExpressionProgram {
    pub fn from_qualified_resource_reference_some_hex(
        encoded: &str,
        admitted_bytes: usize,
    ) -> Result<Self, E> {
        let receipt = QualifiedExpressionProgramStorage::resource_reference_some_hex(encoded)?;
        if admitted_bytes < receipt.preparation_bytes {
            return Err(E::InsufficientPreparation);
        }
        let program = Self::from_canonical_hex(encoded).map_err(|_| E::InvalidPreparedStorage)?;
        QualifiedExpressionStorage::for_resource_reference_some(&program)?;
        if size_of::<Self>() + program.qualified_retained_backing_bytes()?
            > receipt.decoded_program_bytes
        {
            return Err(E::InvalidPreparedStorage);
        }
        Ok(program)
    }
}

impl PortableExpressionProgram {
    /// Actual reachable decoded capacities excluding inline Self. Shared Type
    /// aliases are conservatively charged in full, following Core accounting.
    pub fn qualified_retained_backing_bytes(&self) -> Result<usize, E> {
        QualifiedExpressionStorage::for_resource_reference_some(self)?;
        let crate::PortableExpressionOperation::Variant { tag, payload } = &self.root.operation
        else {
            return Err(E::UnsupportedProgram);
        };
        self.input_type
            .retained_backing_bytes()
            .and_then(|n| n.checked_add(self.output_type.retained_backing_bytes()?))
            .and_then(|n| n.checked_add(self.root.value_type.retained_backing_bytes()?))
            .and_then(|n| n.checked_add(payload.value_type.retained_backing_bytes()?))
            .and_then(|n| n.checked_add(size_of::<PortableExpressionNode>()))
            .and_then(|n| n.checked_add(tag.capacity()))
            .ok_or(E::InvalidPreparedStorage)
    }
}
