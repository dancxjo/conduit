//! Mechanical canonical record composition with pre-admitted preparation work.
//! Only the exact prepared Native descriptor chooses a schema. The resulting
//! bytes still require full family admission before any target consumption.
use alloc::{string::String, vec::Vec};
use conduit_core::{
    PreparedStructuredComposer, StructuredInfoType, StructuredInfoTypeShape,
    ValidatedCanonicalStructuredValue,
};
use conduit_plot::rust_binding::{PreparedNativeFamily, PreparedNativeRustBinding};

#[derive(Clone, Copy, Debug)]
pub(crate) struct ParserCompositionLimits {
    pub maximum_output_bytes: usize,
    pub maximum_preparation_requested_bytes: usize,
    pub maximum_retained_requested_bytes: usize,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct ParserCompositionReceipt {
    pub preparation_requested_bytes_bound: usize,
    pub retained_requested_bytes_bound: usize,
}
#[derive(Debug)]
pub(crate) enum ParserCompositionRefusal {
    Descriptor,
    Type,
    Pressure,
    Composition,
}
pub(crate) struct PreparedParserCanonicalComposer {
    composer: PreparedStructuredComposer,
    receipt: ParserCompositionReceipt,
}
fn add(a: usize, b: usize) -> Result<usize, ParserCompositionRefusal> {
    a.checked_add(b).ok_or(ParserCompositionRefusal::Pressure)
}
fn mul(a: usize, b: usize) -> Result<usize, ParserCompositionRefusal> {
    a.checked_mul(b).ok_or(ParserCompositionRefusal::Pressure)
}
// Core's encoder/collector use geometric Vec growth. All cumulative requested
// capacities are below four times the final logical length, including Vec's
// minimum nonzero capacity (eight for bytes, four for these nonbyte fields).
fn encoded_requests(bytes: usize) -> Result<usize, ParserCompositionRefusal> {
    mul(bytes.max(8), 4)
}
pub(crate) fn composer_requests(
    ty: &StructuredInfoType,
    output: usize,
) -> Result<usize, ParserCompositionRefusal> {
    let mut bytes = add(
        output,
        encoded_requests(
            ty.canonical_byte_length()
                .map_err(|_| ParserCompositionRefusal::Type)?,
        )?,
    )?;
    match ty.shape() {
        StructuredInfoTypeShape::Leaf(kind) => bytes = add(bytes, kind.as_str().len())?,
        StructuredInfoTypeShape::Record { fields, .. } => {
            let slot = add(
                add(
                    core::mem::size_of::<String>(),
                    core::mem::size_of::<Vec<u8>>(),
                )?,
                core::mem::align_of::<Vec<u8>>(),
            )?;
            bytes = add(bytes, mul(mul(fields.len().max(4), 4)?, slot)?)?;
            for field in fields {
                bytes = add(bytes, field.name().len())?;
                bytes = add(
                    bytes,
                    encoded_requests(
                        field
                            .value_type()
                            .canonical_byte_length()
                            .map_err(|_| ParserCompositionRefusal::Type)?,
                    )?,
                )?;
            }
        }
        StructuredInfoTypeShape::Variant { cases, .. } => {
            let slot = add(
                add(
                    core::mem::size_of::<String>(),
                    core::mem::size_of::<Vec<u8>>(),
                )?,
                core::mem::align_of::<Vec<u8>>(),
            )?;
            bytes = add(bytes, mul(mul(cases.len().max(4), 4)?, slot)?)?;
            for case in cases {
                bytes = add(bytes, case.tag().len())?;
                bytes = add(
                    bytes,
                    encoded_requests(
                        case.payload_type()
                            .canonical_byte_length()
                            .map_err(|_| ParserCompositionRefusal::Type)?,
                    )?,
                )?;
            }
        }
        _ => return Err(ParserCompositionRefusal::Type),
    }
    Ok(bytes)
}
pub(crate) fn encoded_composer_requests(
    encoded: &[u8],
    output: usize,
) -> Result<usize, ParserCompositionRefusal> {
    use crate::parser_canonical_schema::{shape, Shape};
    use ParserCompositionRefusal as R;
    let mut bytes = add(output, encoded_requests(encoded.len())?)?;
    match shape(encoded).map_err(|_| R::Type)? {
        Shape::Leaf(kind) => bytes = add(bytes, kind.len())?,
        Shape::Record(fields) | Shape::Variant(fields) => {
            let slot = add(
                add(
                    core::mem::size_of::<String>(),
                    core::mem::size_of::<Vec<u8>>(),
                )?,
                core::mem::align_of::<Vec<u8>>(),
            )?;
            bytes = add(bytes, mul(mul(fields.len().max(4), 4)?, slot)?)?;
            for field in fields {
                let (name, ty) = field.map_err(|_| R::Type)?;
                bytes = add(bytes, name.len())?;
                bytes = add(bytes, encoded_requests(ty.len())?)?;
            }
        }
        _ => return Err(R::Type),
    }
    Ok(bytes)
}
impl PreparedParserCanonicalComposer {
    pub(crate) fn prepare<T: PreparedNativeRustBinding>(
        family: &PreparedNativeFamily,
        limits: ParserCompositionLimits,
    ) -> Result<Self, ParserCompositionRefusal> {
        Self::prepare_field::<T>(family, &[], limits)
    }
    /// Primitive leaf schemas come from an exact admitted record field; they
    /// cannot be supplied as an unrelated caller Type or Native descriptor.
    pub(crate) fn prepare_field<T: PreparedNativeRustBinding>(
        family: &PreparedNativeFamily,
        field_path: &[&str],
        limits: ParserCompositionLimits,
    ) -> Result<Self, ParserCompositionRefusal> {
        use ParserCompositionRefusal as R;
        if limits.maximum_output_bytes == 0
            || limits.maximum_output_bytes > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
        {
            return Err(R::Pressure);
        }
        if !family.contains_descriptor(T::PREPARED_DESCRIPTOR) {
            return Err(R::Descriptor);
        }
        let selected_bytes = crate::parser_canonical_schema::select_field(
            T::PREPARED_DESCRIPTOR.type_bytes,
            field_path,
        )
        .map_err(|_| R::Type)?;
        let decode = StructuredInfoType::canonical_decode_storage_bound(selected_bytes)
            .map_err(|_| R::Type)?;
        let retained = encoded_composer_requests(selected_bytes, limits.maximum_output_bytes)?;
        let preparation = add(decode, retained)?;
        // The entire preparation envelope is known and admitted before the first
        // Type allocation. Root pointer readiness still precedes schema selection.
        if retained > limits.maximum_retained_requested_bytes
            || preparation > limits.maximum_preparation_requested_bytes
        {
            return Err(R::Pressure);
        }
        let selected =
            StructuredInfoType::from_canonical_bytes(selected_bytes).map_err(|_| R::Type)?;
        if composer_requests(&selected, limits.maximum_output_bytes)? != retained {
            return Err(R::Type);
        }
        let composer = PreparedStructuredComposer::new(&selected, limits.maximum_output_bytes)
            .map_err(|_| R::Composition)?;
        Ok(Self {
            composer,
            receipt: ParserCompositionReceipt {
                preparation_requested_bytes_bound: preparation,
                retained_requested_bytes_bound: retained,
            },
        })
    }
    pub(crate) fn receipt(&self) -> ParserCompositionReceipt {
        self.receipt
    }
    pub(crate) fn record(
        &mut self,
        fields: &[ValidatedCanonicalStructuredValue<'_>],
    ) -> Result<&[u8], ParserCompositionRefusal> {
        self.composer
            .record(fields)
            .map_err(|_| ParserCompositionRefusal::Composition)
    }
    pub(crate) fn leaf(&mut self, bytes: &[u8]) -> Result<&[u8], ParserCompositionRefusal> {
        self.composer
            .leaf(bytes)
            .map_err(|_| ParserCompositionRefusal::Composition)
    }
    pub(crate) fn variant(
        &mut self,
        tag: &str,
        payload: ValidatedCanonicalStructuredValue<'_>,
    ) -> Result<&[u8], ParserCompositionRefusal> {
        self.composer
            .variant(tag, payload)
            .map_err(|_| ParserCompositionRefusal::Composition)
    }
}
