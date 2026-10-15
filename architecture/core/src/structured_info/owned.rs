//! Consuming access to an admitted structured value without cloning its tree.
use super::{
    StructuredFieldValue, StructuredInfoType, StructuredInfoValue, StructuredInfoValueNode,
};
use alloc::{boxed::Box, string::String, vec::Vec};

/// The owned counterpart of `StructuredInfoValueShape`.
///
/// Every component retains the exact Type and value already admitted by Core.
/// Moving these parts grants no unchecked construction entrance.
pub enum OwnedStructuredInfoValueShape {
    Leaf(Vec<u8>),
    Collection(Vec<StructuredInfoValue>),
    Record(Vec<StructuredFieldValue>),
    Variant {
        tag: String,
        payload: Box<StructuredInfoValue>,
    },
}

impl StructuredInfoValue {
    /// Move the exact Type and admitted payload out of this value.
    pub fn into_parts(self) -> (StructuredInfoType, OwnedStructuredInfoValueShape) {
        let shape = match self.node {
            StructuredInfoValueNode::Leaf(bytes) => OwnedStructuredInfoValueShape::Leaf(bytes),
            StructuredInfoValueNode::Collection(values) => {
                OwnedStructuredInfoValueShape::Collection(values)
            }
            StructuredInfoValueNode::Record(fields) => {
                OwnedStructuredInfoValueShape::Record(fields)
            }
            StructuredInfoValueNode::Variant { tag, payload } => {
                OwnedStructuredInfoValueShape::Variant { tag, payload }
            }
        };
        (self.value_type, shape)
    }
}

impl StructuredFieldValue {
    /// Move the admitted field name and value without cloning either.
    pub fn into_parts(self) -> (String, StructuredInfoValue) {
        (self.name, self.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kind_id, TEXT_INFO_ID};

    #[test]
    fn consuming_parts_move_admitted_bytes_and_keep_the_exact_type() {
        let bytes = "🦀".as_bytes().to_vec();
        let allocation = bytes.as_ptr();
        let ty = StructuredInfoType::leaf(kind_id(TEXT_INFO_ID)).unwrap();
        let value = StructuredInfoValue::leaf(ty.clone(), bytes).unwrap();
        let (actual_type, shape) = value.into_parts();
        assert_eq!(actual_type, ty);
        let OwnedStructuredInfoValueShape::Leaf(bytes) = shape else {
            panic!("leaf")
        };
        assert_eq!(bytes.as_ptr(), allocation);
        assert_eq!(bytes, "🦀".as_bytes());
    }
}
