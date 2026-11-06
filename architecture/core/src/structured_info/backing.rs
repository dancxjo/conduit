//! Actual owned schema backing, excluding inline values and allocator overhead.
use super::*;
use core::mem::size_of;
impl StructuredInfoType {
    /// Actual owned capacities, including nested boxes; excludes inline Self.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        use StructuredInfoTypeNode::*;
        match &self.0 {
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
    #[test]
    fn schema_backing_counts_nested_boxes_and_spare_vector_and_text_capacity() {
        let leaf = text("value/u8", 97);
        let mut expected = leaf.capacity();
        let mut ty = StructuredInfoType::leaf(KindId(leaf)).unwrap();
        assert_eq!(ty.retained_backing_bytes(), Some(expected));
        ty = StructuredInfoType::collection(ty, Some(2)).unwrap();
        expected += size_of::<StructuredInfoType>();
        ty = StructuredInfoType::sequence(ty, 3).unwrap();
        expected += size_of::<StructuredInfoType>();
        let schema = text("fixture/nominal", 83);
        expected += schema.capacity() + size_of::<StructuredInfoType>();
        ty = StructuredInfoType::nominal(KindId(schema), ty).unwrap();
        let name = text("input", 71);
        expected += name.capacity();
        let mut fields = Vec::with_capacity(7);
        fields.push(StructuredFieldType::new(name, ty).unwrap());
        let schema = text("fixture/record", 67);
        expected += schema.capacity() + fields.capacity() * size_of::<StructuredFieldType>();
        ty = StructuredInfoType::record(KindId(schema), fields).unwrap();
        assert_eq!(ty.retained_backing_bytes(), Some(expected));
        let tag = text("complete", 61);
        expected += tag.capacity();
        let mut cases = Vec::with_capacity(5);
        cases.push(StructuredVariantCase::new(tag, ty).unwrap());
        let schema = text("fixture/variant", 59);
        expected += schema.capacity() + cases.capacity() * size_of::<StructuredVariantCase>();
        ty = StructuredInfoType::variant(KindId(schema), cases).unwrap();
        assert_eq!(ty.retained_backing_bytes(), Some(expected));
    }
}
