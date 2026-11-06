//! Conservative reachable schema backing, excluding inline values and allocator overhead.
use super::*;
use core::mem::size_of;
impl StructuredInfoType {
    /// Reachable capacities, including nested boxes and atomic ownership headers.
    /// Shared aliases are charged in full, so separate owners never undercharge
    /// shared backing. This is conservative rather than exclusive ownership.
    /// Excludes inline Self and allocator overhead.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        use StructuredInfoTypeNode::*;
        #[cfg(target_has_atomic = "ptr")]
        let node = match &self.0 {
            StructuredInfoTypeStorage::Leaf(kind) => return Some(kind.0.capacity()),
            StructuredInfoTypeStorage::Compound(node) => &**node,
        };
        #[cfg(not(target_has_atomic = "ptr"))]
        let node = &self.0;
        let contents = match node {
            Leaf(kind) => Some(kind.0.capacity()),
            Nominal {
                schema,
                representation,
            } => schema
                .0
                .capacity()
                .checked_add(size_of::<Self>())?
                .checked_add(representation.retained_backing_bytes()?),
            Collection { element, .. } | Sequence { element, .. } => {
                size_of::<Self>().checked_add(element.retained_backing_bytes()?)
            }
            Record { schema, fields } => {
                let mut bytes = schema.0.capacity().checked_add(
                    fields
                        .capacity()
                        .checked_mul(size_of::<StructuredFieldType>())?,
                )?;
                for field in fields {
                    bytes = bytes
                        .checked_add(field.name.capacity())?
                        .checked_add(field.value_type.retained_backing_bytes()?)?;
                }
                Some(bytes)
            }
            Variant { schema, cases } => {
                let mut bytes = schema.0.capacity().checked_add(
                    cases
                        .capacity()
                        .checked_mul(size_of::<StructuredVariantCase>())?,
                )?;
                for case in cases {
                    bytes = bytes
                        .checked_add(case.tag.capacity())?
                        .checked_add(case.payload_type.retained_backing_bytes()?)?;
                }
                Some(bytes)
            }
        }?;
        contents.checked_add(Self::node_backing_bytes())
    }

    fn node_backing_bytes() -> usize {
        #[cfg(target_has_atomic = "ptr")]
        {
            size_of::<StructuredInfoTypeNode>() + 2 * size_of::<usize>()
        }
        #[cfg(not(target_has_atomic = "ptr"))]
        {
            0
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn text(s: &str, capacity: usize) -> String {
        let mut v = String::with_capacity(capacity);
        v.push_str(s);
        v
    }
    #[cfg(target_has_atomic = "ptr")]
    #[test]
    fn immutable_schema_clones_share_storage_but_keep_conservative_custody() {
        let original = StructuredInfoType::collection(
            StructuredInfoType::leaf(crate::kind_id("value/u8")).unwrap(),
            Some(4),
        )
        .unwrap();
        let cloned = original.clone();
        let (StructuredInfoTypeStorage::Compound(left), StructuredInfoTypeStorage::Compound(right)) =
            (&original.0, &cloned.0)
        else {
            panic!("collection must use compound storage");
        };
        assert!(alloc::sync::Arc::ptr_eq(left, right));
        assert_eq!(original.canonical_bytes(), cloned.canonical_bytes());
        assert_eq!(
            original.retained_backing_bytes(),
            cloned.retained_backing_bytes()
        );
        drop(original);
        assert!(cloned.profile().is_ok());
    }

    #[test]
    fn schema_backing_counts_nested_boxes_and_spare_vector_and_text_capacity() {
        let leaf = text("value/u8", 97);
        let mut expected = leaf.capacity();
        let mut ty = StructuredInfoType::leaf(KindId(leaf)).unwrap();
        assert_eq!(ty.retained_backing_bytes(), Some(expected));
        ty = StructuredInfoType::collection(ty, Some(2)).unwrap();
        expected += size_of::<StructuredInfoType>() + StructuredInfoType::node_backing_bytes();
        ty = StructuredInfoType::sequence(ty, 3).unwrap();
        expected += size_of::<StructuredInfoType>() + StructuredInfoType::node_backing_bytes();
        let schema = text("fixture/nominal", 83);
        expected += schema.capacity()
            + size_of::<StructuredInfoType>()
            + StructuredInfoType::node_backing_bytes();
        ty = StructuredInfoType::nominal(KindId(schema), ty).unwrap();
        let name = text("input", 71);
        expected += name.capacity();
        let mut fields = Vec::with_capacity(7);
        fields.push(StructuredFieldType::new(name, ty).unwrap());
        let schema = text("fixture/record", 67);
        expected += schema.capacity()
            + fields.capacity() * size_of::<StructuredFieldType>()
            + StructuredInfoType::node_backing_bytes();
        ty = StructuredInfoType::record(KindId(schema), fields).unwrap();
        assert_eq!(ty.retained_backing_bytes(), Some(expected));
        let tag = text("complete", 61);
        expected += tag.capacity();
        let mut cases = Vec::with_capacity(5);
        cases.push(StructuredVariantCase::new(tag, ty).unwrap());
        let schema = text("fixture/variant", 59);
        expected += schema.capacity()
            + cases.capacity() * size_of::<StructuredVariantCase>()
            + StructuredInfoType::node_backing_bytes();
        ty = StructuredInfoType::variant(KindId(schema), cases).unwrap();
        assert_eq!(ty.retained_backing_bytes(), Some(expected));
    }
}
