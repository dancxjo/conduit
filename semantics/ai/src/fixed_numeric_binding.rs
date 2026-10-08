//! Exact queue-visible tensor view of an already admitted immutable resource.
//! This does not resolve a resource, confer authority, or admit a ModelArtifact.
use alloc::vec::Vec;
use conduit_core::{
    StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape as Shape, StructuredInfoValue,
};
use conduit_data::{TensorBacking, TensorElement, TensorValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedBindingRefusal {
    Shape,
    Element,
    ResourceRequired,
    Encoding,
}
pub struct FixedTensorPortBinding {
    encoded: Vec<u8>,
}
impl FixedTensorPortBinding {
    pub fn prepare(
        profile: &StructuredInfoType,
        tensor: &TensorValue,
    ) -> Result<Self, FixedBindingRefusal> {
        // Admit only exact owned profiles and their fixed dimensions. Structural
        // encoding alone cannot enforce nominal singleton refinements.
        let (name, element_tag) = match (tensor.element, tensor.dimensions.as_slice()) {
            (TensorElement::F32, [columns, rows]) => {
                (alloc::format!("NumericF32MatrixRef{columns}x{rows}"), "f32")
            }
            (TensorElement::F32, [length]) => (alloc::format!("NumericF32BiasRef{length}"), "f32"),
            (TensorElement::I8, [columns, rows]) => (
                alloc::format!("NumericI8TiledMatrixRef{columns}x{rows}"),
                "i8",
            ),
            _ => return Err(FixedBindingRefusal::Element),
        };
        let admitted = crate::fixed_numeric_catalog::fixed_numeric_type(&name)
            .is_ok_and(|owned| owned == *profile)
            || (tensor.element == TensorElement::I8
                && crate::fixed_numeric_compact_catalog::fixed_compact_type(&name)
                    .is_ok_and(|owned| owned == *profile))
            || (tensor.element == TensorElement::F32
                && tensor.dimensions.len() == 1
                && crate::fixed_numeric_compact_catalog::fixed_compact_type(&alloc::format!(
                    "NumericF32ScaleRef{}",
                    tensor.dimensions[0]
                ))
                .is_ok_and(|owned| owned == *profile))
            || (tensor.element == TensorElement::F32
                && tensor.dimensions.as_slice() == [224, 12]
                && crate::fixed_numeric_catalog::fixed_numeric_type("NumericEmbedding224x12")
                    .is_ok_and(|owned| owned == *profile));
        if !admitted {
            return Err(FixedBindingRefusal::Shape);
        }
        tensor.validate().map_err(|_| FixedBindingRefusal::Shape)?;
        let TensorBacking::Resource(resource) = &tensor.backing else {
            return Err(FixedBindingRefusal::ResourceRequired);
        };
        let Shape::Record { fields, .. } = profile.shape() else {
            return Err(FixedBindingRefusal::Shape);
        };
        let mut values = Vec::with_capacity(fields.len());
        for field in fields {
            let ty = field.value_type();
            let value = match field.name() {
                "resource" => leaf(
                    ty,
                    &resource
                        .encode()
                        .map_err(|_| FixedBindingRefusal::Encoding)?,
                )?,
                "content_digest" => digest(ty, &tensor.content_digest)?,
                "element" => {
                    let Shape::Variant { cases, .. } = ty.shape() else {
                        return Err(FixedBindingRefusal::Shape);
                    };
                    let case = cases
                        .iter()
                        .find(|case| case.tag() == element_tag)
                        .ok_or(FixedBindingRefusal::Element)?;
                    StructuredInfoValue::variant(
                        ty.clone(),
                        element_tag,
                        leaf(case.payload_type(), &[])?,
                    )
                    .map_err(|_| FixedBindingRefusal::Encoding)?
                }
                "columns" if tensor.dimensions.len() == 2 => leaf(
                    ty,
                    &u16::try_from(tensor.dimensions[0])
                        .map_err(|_| FixedBindingRefusal::Shape)?
                        .to_le_bytes(),
                )?,
                "rows" if tensor.dimensions.len() == 2 => leaf(
                    ty,
                    &u16::try_from(tensor.dimensions[1])
                        .map_err(|_| FixedBindingRefusal::Shape)?
                        .to_le_bytes(),
                )?,
                "length" if tensor.dimensions.len() == 1 => leaf(
                    ty,
                    &u16::try_from(tensor.dimensions[0])
                        .map_err(|_| FixedBindingRefusal::Shape)?
                        .to_le_bytes(),
                )?,
                _ => return Err(FixedBindingRefusal::Shape),
            };
            values.push(
                StructuredFieldValue::new(field.name(), value)
                    .map_err(|_| FixedBindingRefusal::Encoding)?,
            );
        }
        let encoded = StructuredInfoValue::record(profile.clone(), values)
            .and_then(|value| value.canonical_bytes())
            .map_err(|_| FixedBindingRefusal::Encoding)?;
        Ok(Self { encoded })
    }
    pub fn encoded(&self) -> &[u8] {
        &self.encoded
    }
    pub fn matches(&self, bytes: &[u8]) -> bool {
        bytes == self.encoded
    }
    pub fn allocation_capacity(&self) -> usize {
        self.encoded.capacity()
    }
}
fn leaf(ty: &StructuredInfoType, bytes: &[u8]) -> Result<StructuredInfoValue, FixedBindingRefusal> {
    match ty.shape() {
        Shape::Nominal { representation, .. } => {
            StructuredInfoValue::nominal(ty.clone(), leaf(representation, bytes)?)
        }
        Shape::Leaf(_) => StructuredInfoValue::leaf(ty.clone(), bytes.to_vec()),
        _ => return Err(FixedBindingRefusal::Shape),
    }
    .map_err(|_| FixedBindingRefusal::Encoding)
}
fn digest(
    ty: &StructuredInfoType,
    bytes: &[u8; 32],
) -> Result<StructuredInfoValue, FixedBindingRefusal> {
    match ty.shape() {
        Shape::Nominal { representation, .. } => {
            StructuredInfoValue::nominal(ty.clone(), digest(representation, bytes)?)
        }
        Shape::Collection {
            element,
            length: 32,
        } => StructuredInfoValue::collection(
            ty.clone(),
            bytes
                .iter()
                .map(|b| leaf(element, &[*b]))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        _ => return Err(FixedBindingRefusal::Shape),
    }
    .map_err(|_| FixedBindingRefusal::Encoding)
}

impl FixedTensorPortBinding {
    /// Retained requested local payload capacity. Shared tensor/model Arc owners,
    /// Box root, Arc headers, allocator bookkeeping and stack are separate charges.
    pub fn local_accounted_heap_bytes(&self) -> usize {
        self.encoded.capacity()
    }
}
