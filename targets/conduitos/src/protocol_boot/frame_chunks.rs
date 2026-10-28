//! Bounded raster-to-typed-input encoding; controller protocol stays in Source.
use crate::display::rgb565::{Orientation, Rgb565Error, Rgb565Raster};
use conduit_core::{
    PreparedStructuredComposer, StructuredInfoRefusal, StructuredInfoType, StructuredInfoTypeShape,
    validate_canonical_structured_value,
};

#[derive(Debug, PartialEq, Eq)]
pub enum FrameChunkRefusal {
    Schema,
    Index,
    Raster(Rgb565Error),
    Canonical(StructuredInfoRefusal),
}

/// Native preparation fixes geometry, packing, chunk size and output schema.
/// The Source request supplies only an index into that admitted immutable frame.
///
/// The schema preserves member types, not Source byte-ceiling laws. Preparation
/// must choose a chunk size fitting the checked Fore; this producer independently
/// caps packed payloads at 512 bytes (256 pixels).
pub struct PreparedFrameChunks {
    width: u32,
    height: u32,
    chunk_pixels: usize,
    orientation: Orientation,
    wire: alloc::vec::Vec<u8>,
    output: PreparedStructuredComposer,
    index: PreparedStructuredComposer,
    bytes: PreparedStructuredComposer,
}
impl PreparedFrameChunks {
    pub fn new(
        schema: &StructuredInfoType,
        width: u32,
        height: u32,
        chunk_pixels: usize,
        orientation: Orientation,
    ) -> Result<Self, FrameChunkRefusal> {
        use FrameChunkRefusal as Error;
        if width == 0
            || height == 0
            || width.checked_mul(height).is_none()
            || chunk_pixels == 0
            || chunk_pixels > 256
        {
            return Err(Error::Schema);
        }
        let StructuredInfoTypeShape::Record { fields, .. } = schema.shape() else {
            return Err(Error::Schema);
        };
        if fields.len() != 2
            || fields[0].name() != "index"
            || fields[1].name() != "wire"
            || !matches!(fields[0].value_type().shape(), StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == "value/u64")
            || !matches!(fields[1].value_type().shape(), StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == "value/bytes")
        {
            return Err(Error::Schema);
        }
        let make = |ty| PreparedStructuredComposer::new(ty, 4096).map_err(Error::Canonical);
        Ok(Self {
            width,
            height,
            chunk_pixels,
            orientation,
            wire: alloc::vec![0; chunk_pixels * 2],
            output: make(schema)?,
            index: make(fields[0].value_type())?,
            bytes: make(fields[1].value_type())?,
        })
    }
    pub fn count(&self) -> u64 {
        (u64::from(self.width) * u64::from(self.height)).div_ceil(self.chunk_pixels as u64)
    }
    pub fn encode(
        &mut self,
        raster: &Rgb565Raster<'_>,
        index: u64,
    ) -> Result<&[u8], FrameChunkRefusal> {
        use FrameChunkRefusal as Error;
        if raster.width() != self.width || raster.height() != self.height || index >= self.count() {
            return Err(Error::Index);
        }
        let offset = usize::try_from(index)
            .ok()
            .and_then(|index| index.checked_mul(self.chunk_pixels))
            .ok_or(Error::Index)?;
        let count = self.chunk_pixels.min(raster.pixel_count() - offset);
        let length = raster
            .encode_into(offset, count, self.orientation, &mut self.wire)
            .map_err(Error::Raster)?;
        let index = self
            .index
            .leaf(&index.to_le_bytes())
            .map_err(Error::Canonical)?;
        let wire = self
            .bytes
            .leaf(&self.wire[..length])
            .map_err(Error::Canonical)?;
        self.output
            .record(&[
                validate_canonical_structured_value(index).map_err(Error::Canonical)?,
                validate_canonical_structured_value(wire).map_err(Error::Canonical)?,
            ])
            .map_err(Error::Canonical)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol_source::{PreparedProtocolEntry, ProtocolSourcePackage};
    use conduit_core::{
        ConfigurationValue, StructuredInfoValue, StructuredInfoValueShape, port_id,
    };
    use conduit_plot::{PortableExpressionProgram, PreparedPortableExpressionEvaluator};

    fn entry(declaration: &str) -> PreparedProtocolEntry {
        let source = alloc::format!(
            "type Chunk = {declaration}\nplot chunk-identity (\n chunk: Chunk...| >> output: Chunk...|\n) = (.)"
        );
        let package = ProtocolSourcePackage::compile(source, &[]).unwrap();
        PreparedProtocolEntry::prepare(&serde_json::to_vec(&package).unwrap(), "chunk-identity")
            .unwrap()
    }
    fn checked() -> PreparedProtocolEntry {
        entry("{\n index: U64\n wire: Bytes <= 512B\n}")
    }
    fn schema() -> StructuredInfoType {
        checked().input_schema(&port_id("chunk")).unwrap()
    }
    fn observed(encoded: &[u8]) -> (u64, alloc::vec::Vec<u8>) {
        let value = StructuredInfoValue::from_canonical_bytes(encoded).unwrap();
        let StructuredInfoValueShape::Record(fields) = value.shape() else {
            panic!("record")
        };
        let leaf = |index: usize| {
            let StructuredInfoValueShape::Leaf(bytes) = fields[index].value().shape() else {
                panic!("leaf")
            };
            bytes.to_vec()
        };
        (u64::from_le_bytes(leaf(0).try_into().unwrap()), leaf(1))
    }
    #[test]
    fn frame_chunks_checked_source_receives_exact_native_and_rotated_chunks() {
        let entry = checked();
        let schema = entry.input_schema(&port_id("chunk")).unwrap();
        let gear = &entry.expanded().expanded.gears[0];
        let ConfigurationValue::Text(hex) = &gear.configuration[0].value else {
            panic!("program")
        };
        let program = PortableExpressionProgram::from_canonical_hex(hex).unwrap();
        let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        let pixels = [0xff0000, 0x00ff00, 0x0000ff, 0xffffff, 0, 0x080408];
        let raster = Rgb565Raster::new(&pixels, 3, 2).unwrap();
        for (orientation, expected) in [
            (
                Orientation::Native,
                alloc::vec![
                    alloc::vec![0xf8, 0, 7, 0xe0],
                    alloc::vec![0, 0x1f, 0xff, 0xff],
                    alloc::vec![0, 0, 0x08, 0x21]
                ],
            ),
            (
                Orientation::Rotate180,
                alloc::vec![
                    alloc::vec![0x08, 0x21, 0, 0],
                    alloc::vec![0xff, 0xff, 0, 0x1f],
                    alloc::vec![7, 0xe0, 0xf8, 0]
                ],
            ),
        ] {
            let mut chunks = PreparedFrameChunks::new(&schema, 3, 2, 2, orientation).unwrap();
            assert_eq!(chunks.count(), 3);
            for (index, expected) in expected.iter().enumerate() {
                let encoded = chunks.encode(&raster, index as u64).unwrap();
                assert_eq!(observed(encoded), (index as u64, expected.clone()));
                // The canonical producer value is accepted by the ordinary
                // prepared Source VM and preserved by its typed identity.
                assert_eq!(evaluator.evaluate(encoded).unwrap(), encoded);
            }
        }
    }
    #[test]
    fn frame_chunks_enforces_512_byte_chunks_and_exact_short_tail_without_growth() {
        let schema = schema();
        let pixels = [0xffffff; 513];
        let raster = Rgb565Raster::new(&pixels, 513, 1).unwrap();
        let mut chunks =
            PreparedFrameChunks::new(&schema, 513, 1, 256, Orientation::Native).unwrap();
        assert_eq!(chunks.count(), 3);
        for (index, length) in [(0, 512), (1, 512), (2, 2)] {
            let (observed_index, wire) = observed(chunks.encode(&raster, index).unwrap());
            assert_eq!(observed_index, index);
            assert_eq!(wire, alloc::vec![0xff;length]);
        }
        let allocations = crate::test_allocations::allocations(|| {
            for index in 0..chunks.count() {
                assert!(chunks.encode(&raster, index).is_ok());
            }
        });
        assert_eq!(allocations, 0);
    }
    #[test]
    fn frame_chunks_rejects_invalid_geometry_chunk_schema_and_indices() {
        let schema = schema();
        for (width, height, size) in [
            (0, 1, 1),
            (1, 0, 1),
            (u32::MAX, 2, 1),
            (1, 1, 0),
            (1, 1, 257),
        ] {
            assert!(matches!(
                PreparedFrameChunks::new(&schema, width, height, size, Orientation::Native),
                Err(FrameChunkRefusal::Schema)
            ));
        }
        for declaration in [
            "Boolean",
            "{\n index: Boolean\n wire: Bytes <= 512B\n}",
            "{\n index: U64\n wire: U64\n}",
            "{\n index: U64\n wire: Bytes <= 512B\n extra: Boolean\n}",
            "{\n ordinal: U64\n wire: Bytes <= 512B\n}",
        ] {
            let schema = entry(declaration).input_schema(&port_id("chunk")).unwrap();
            assert!(matches!(
                PreparedFrameChunks::new(&schema, 1, 1, 1, Orientation::Native),
                Err(FrameChunkRefusal::Schema)
            ));
        }
        let mut chunks = PreparedFrameChunks::new(&schema, 2, 1, 1, Orientation::Native).unwrap();
        let raster = Rgb565Raster::new(&[0, 0], 2, 1).unwrap();
        for index in [2, u64::MAX] {
            assert_eq!(chunks.encode(&raster, index), Err(FrameChunkRefusal::Index));
        }
        let wrong = Rgb565Raster::new(&[0, 0], 1, 2).unwrap();
        assert_eq!(chunks.encode(&wrong, 0), Err(FrameChunkRefusal::Index));
        // Refusals do not poison a later valid producer request.
        assert_eq!(
            observed(chunks.encode(&raster, 1).unwrap()),
            (1, alloc::vec![0, 0])
        );
    }
}
