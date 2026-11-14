//! Private backing may be smaller than the selected contract envelope only
//! where complete schema validation enforces a tighter finite byte ceiling.
use conduit_core::{
    fixed_primitive_info_payload_bytes, StructuredInfoType, StructuredInfoTypeShape,
};

pub(crate) fn bytes(schema: &StructuredInfoType, selected_maximum: u32) -> usize {
    let bound = match schema.shape() {
        StructuredInfoTypeShape::Leaf(kind) => fixed_primitive_info_payload_bytes(kind.as_str()),
        _ => schema
            .canonical_value_upper_bound(|kind| fixed_primitive_info_payload_bytes(kind.as_str())),
    };
    bound.map_or(selected_maximum as usize, |bound| {
        bound.min(selected_maximum as usize)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{kind_id, StructuredInfoValue};

    #[test]
    fn maximum_fixed_collection_fits_while_unknown_leaves_keep_selected_capacity() {
        let leaf = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
        let schema = StructuredInfoType::collection(leaf.clone(), Some(1024)).unwrap();
        let value = StructuredInfoValue::collection(
            schema.clone(),
            (0..1024)
                .map(|_| StructuredInfoValue::leaf(leaf.clone(), vec![255]).unwrap())
                .collect(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        assert_eq!(bytes(&schema, 16384), value.len());
        assert_eq!(bytes(&schema, 128), 128);
        let unknown = StructuredInfoType::leaf(kind_id("domain/unbounded-here")).unwrap();
        assert_eq!(bytes(&unknown, 4096), 4096);
        let aggregate = StructuredInfoType::collection(unknown, Some(2)).unwrap();
        assert_eq!(bytes(&aggregate, 4096), 4096);
    }
}
