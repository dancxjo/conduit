//! One complete shared CPE frame in two bounded primitive Bytes leaves.
use conduit_core::*;
use conduit_semantic_catalog::{
    quantity_coordinate_evidence_type, QUANTITY_COORDINATES_EVIDENCE_MAX_BYTES,
};
pub(super) struct CoordinateEncoding {
    head: PreparedStructuredComposer,
    tail: PreparedStructuredComposer,
    record: PreparedStructuredComposer,
}
impl CoordinateEncoding {
    pub fn new() -> Result<Self, StructuredInfoRefusal> {
        let ty = quantity_coordinate_evidence_type();
        let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
            unreachable!()
        };
        let leaf = |field: &StructuredFieldType| {
            PreparedStructuredComposer::new(
                field.value_type(),
                field.value_type().canonical_bytes()?.len() + 5 + 65536,
            )
        };
        Ok(Self {
            head: leaf(&fields[0])?,
            tail: leaf(&fields[1])?,
            record: PreparedStructuredComposer::new(
                &ty,
                QUANTITY_COORDINATES_EVIDENCE_MAX_BYTES as usize,
            )?,
        })
    }
    pub fn retained_bytes(&self) -> usize {
        self.head.prepared_heap_bytes()
            + self.tail.prepared_heap_bytes()
            + self.record.prepared_heap_bytes()
    }
    pub fn encode(&mut self, frame: &[u8]) -> Result<(), StructuredInfoRefusal> {
        if frame.len() > 131072 {
            return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
        }
        let split = frame.len().min(65536);
        let head = self.head.leaf(&frame[..split])?;
        let tail = self.tail.leaf(&frame[split..])?;
        self.record.record(&[
            validate_canonical_structured_value(head)?,
            validate_canonical_structured_value(tail)?,
        ])?;
        Ok(())
    }
    pub fn encoded(&self) -> &[u8] {
        self.record.encoded()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coordinate_transport_splits_one_frame_at_the_primitive_bound_without_growth() {
        let mut encoding = CoordinateEncoding::new().unwrap();
        let retained = encoding.retained_bytes();
        // Arbitrary transport specimen, deliberately not a semantic CPE claim.
        let bytes = vec![0x5a; 131073];
        let guard = crate::allocation_probe::begin();
        for n in [0, 65535, 65536, 65537, 131072] {
            encoding.encode(&bytes[..n]).unwrap();
            let value = validate_canonical_structured_value(encoding.encoded()).unwrap();
            let head = value
                .record_field("projection-evidence-head")
                .unwrap()
                .unwrap()
                .primitive_representation_bytes("value/bytes")
                .unwrap();
            let tail = value
                .record_field("projection-evidence-tail")
                .unwrap()
                .unwrap()
                .primitive_representation_bytes("value/bytes")
                .unwrap();
            assert_eq!(head, &bytes[..n.min(65536)]);
            assert_eq!(tail, &bytes[n.min(65536)..n]);
        }
        assert!(encoding.encode(&bytes).is_err());
        assert_eq!(guard.finish(), 0);
        assert_eq!(encoding.retained_bytes(), retained);
    }
}
