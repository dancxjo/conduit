//! Pair exact primitive or structured transport values without erasing schemas.
use super::{
    tuple_info_type, validate_canonical_structured_value, PreparedStructuredComposer,
    StructuredInfoRefusal as Refusal, StructuredInfoType, StructuredInfoTypeShape,
    ValidatedCanonicalStructuredValue, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
    MAXIMUM_STRUCTURED_INFO_NODES,
};
use alloc::{vec, vec::Vec};

struct Member {
    schema: Vec<u8>,
    maximum: usize,
    primitive: Option<PreparedStructuredComposer>,
}

impl Member {
    fn new(ty: &StructuredInfoType, maximum: u32) -> Result<Self, Refusal> {
        let schema = ty.canonical_bytes()?;
        let maximum = usize::try_from(maximum).map_err(|_| Refusal::CanonicalEncodingTooLarge)?;
        if maximum > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
            return Err(Refusal::CanonicalEncodingTooLarge);
        }
        let primitive = if matches!(ty.shape(), StructuredInfoTypeShape::Leaf(_)) {
            Some(PreparedStructuredComposer::new(
                ty,
                schema.len() + 5 + maximum,
            )?)
        } else {
            None
        };
        Ok(Self {
            schema,
            maximum,
            primitive,
        })
    }

    fn encode<'a>(
        &'a mut self,
        input: &'a [u8],
    ) -> Result<ValidatedCanonicalStructuredValue<'a>, Refusal> {
        if input.len() > self.maximum {
            return Err(Refusal::CanonicalEncodingTooLarge);
        }
        let encoded = if let Some(primitive) = &mut self.primitive {
            primitive.leaf(input)?
        } else {
            input
        };
        let value = validate_canonical_structured_value(encoded)?;
        if value.type_bytes() != self.schema {
            return Err(Refusal::WrongType);
        }
        Ok(value)
    }
}

/// Preparation retains exact member schemas and finite scratch buffers.
/// Plain primitive members use their raw transport bytes; structured and
/// nominal members use their complete canonical encoding. No schema is
/// inferred from runtime bytes and no allocation occurs during `encode`.
pub struct PreparedTypedTuplePairEncoder {
    left: Member,
    right: Member,
    value_type: StructuredInfoType,
    composer: PreparedStructuredComposer,
    maximum_bytes: u32,
}

impl PreparedTypedTuplePairEncoder {
    pub fn new(
        left_type: StructuredInfoType,
        left_maximum: u32,
        right_type: StructuredInfoType,
        right_maximum: u32,
    ) -> Result<Self, Refusal> {
        let value_type = tuple_info_type(vec![left_type.clone(), right_type.clone()])?;
        // Valid members must not become an unsupported aggregate only after
        // they are consumed. Admit the complete maximum value traversal now.
        maximum_value_nodes(&value_type)?;
        // The pair Type prefix already contains both member Types. Structured
        // member transport includes its own prefix, which is absent from the
        // nested node body; primitives instead need five bytes of leaf framing.
        let node_bound = |ty: &StructuredInfoType, maximum: u32| {
            let maximum =
                usize::try_from(maximum).map_err(|_| Refusal::CanonicalEncodingTooLarge)?;
            if matches!(ty.shape(), StructuredInfoTypeShape::Leaf(_)) {
                maximum
                    .checked_add(5)
                    .ok_or(Refusal::CanonicalEncodingTooLarge)
            } else {
                maximum
                    .checked_sub(ty.canonical_bytes()?.len())
                    .ok_or(Refusal::CanonicalEncodingTooLarge)
            }
        };
        let left_node_bound = node_bound(&left_type, left_maximum)?;
        let right_node_bound = node_bound(&right_type, right_maximum)?;
        let maximum = value_type
            .canonical_bytes()?
            .len()
            .checked_add(5 + 2 * (4 + "item-00000".len()))
            .and_then(|size| size.checked_add(left_node_bound))
            .and_then(|size| size.checked_add(right_node_bound))
            .filter(|size| *size <= MAXIMUM_STRUCTURED_CANONICAL_BYTES)
            .ok_or(Refusal::CanonicalEncodingTooLarge)?;
        let composer = PreparedStructuredComposer::new(&value_type, maximum)?;
        Ok(Self {
            left: Member::new(&left_type, left_maximum)?,
            right: Member::new(&right_type, right_maximum)?,
            value_type,
            composer,
            maximum_bytes: maximum as u32,
        })
    }

    pub fn value_type(&self) -> &StructuredInfoType {
        &self.value_type
    }
    pub fn maximum_bytes(&self) -> u32 {
        self.maximum_bytes
    }
    pub fn encoded(&self) -> &[u8] {
        self.composer.encoded()
    }

    pub fn encode(&mut self, left: &[u8], right: &[u8]) -> Result<&[u8], Refusal> {
        let left = self.left.encode(left)?;
        let right = self.right.encode(right)?;
        self.composer.record(&[left, right])
    }
}

fn maximum_value_nodes(ty: &StructuredInfoType) -> Result<usize, Refusal> {
    let children = match ty.shape() {
        StructuredInfoTypeShape::Leaf(_) => 0,
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            maximum_value_nodes(representation)?
        }
        StructuredInfoTypeShape::Record { fields, .. } => {
            fields.iter().try_fold(0_usize, |sum, field| {
                sum.checked_add(maximum_value_nodes(field.value_type())?)
                    .ok_or(Refusal::TooManyNodes)
            })?
        }
        StructuredInfoTypeShape::Variant { cases, .. } => {
            cases.iter().try_fold(0_usize, |maximum, case| {
                Ok::<usize, Refusal>(maximum.max(maximum_value_nodes(case.payload_type())?))
            })?
        }
        StructuredInfoTypeShape::Collection { element, length } => maximum_value_nodes(element)?
            .checked_mul(usize::from(length))
            .ok_or(Refusal::TooManyNodes)?,
        StructuredInfoTypeShape::Sequence {
            element,
            maximum_items,
            ..
        } => maximum_value_nodes(element)?
            .checked_mul(usize::from(maximum_items))
            .ok_or(Refusal::TooManyNodes)?,
    };
    children
        .checked_add(1)
        .filter(|count| *count <= MAXIMUM_STRUCTURED_INFO_NODES)
        .ok_or(Refusal::TooManyNodes)
}

mod storage;
