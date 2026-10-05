//! Canonical fixed-storage wire-frame fixture shared by descriptor contracts.
use conduit_core::{
    StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue, kind_id,
};
pub(super) fn frame(ty: &StructuredInfoType, wire: &[u8], actual: u64) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("frame record")
    };
    let wire_type = fields
        .iter()
        .find(|f| f.name() == "wire")
        .unwrap()
        .value_type();
    let octet = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
    let wire = StructuredInfoValue::collection(
        wire_type.clone(),
        wire.iter()
            .map(|byte| StructuredInfoValue::leaf(octet.clone(), vec![*byte]).unwrap())
            .collect(),
    )
    .unwrap();
    let actual = StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),
        actual.to_le_bytes().to_vec(),
    )
    .unwrap();
    StructuredInfoValue::record(
        ty.clone(),
        vec![
            StructuredFieldValue::new("wire", wire).unwrap(),
            StructuredFieldValue::new("actual", actual).unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
