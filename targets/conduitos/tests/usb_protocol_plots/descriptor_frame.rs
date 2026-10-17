//! Canonical bounded wire-frame fixture shared by descriptor and HID contracts.
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
    let elements = wire
        .iter()
        .map(|byte| StructuredInfoValue::leaf(octet.clone(), vec![*byte]).unwrap())
        .collect();
    let wire = match wire_type.shape() {
        StructuredInfoTypeShape::Collection { .. } => {
            StructuredInfoValue::collection(wire_type.clone(), elements)
        }
        StructuredInfoTypeShape::Sequence { .. } => {
            StructuredInfoValue::sequence(wire_type.clone(), elements)
        }
        _ => panic!("bounded octet storage"),
    }
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
