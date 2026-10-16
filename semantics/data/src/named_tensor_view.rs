//! Prepared read-only admission of canonical named tensor descriptors.
//! Resource inspection never grants access to referenced content.
use crate::{
    tensor_content_digest, TensorElement, TensorRefusal, MAXIMUM_NAMED_TENSORS,
    MAXIMUM_TENSOR_BYTES,
};
use alloc::{format, string::String, vec::Vec};
use conduit_core::{
    validate_canonical_structured_value, ValidatedCanonicalStructuredValue as View,
};
use conduit_plot::{rust_binding::validate_native_contracts_borrowed, NativeTypeValueContract};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedTensorViewRefusal {
    Encoding,
    Name,
    DuplicateName,
    Tensor(TensorRefusal),
}

/// All schema and Source refinement ownership is established before Play.
pub struct PreparedNamedTensorBundleView {
    type_bytes: Vec<u8>,
    contracts: Vec<NativeTypeValueContract>,
}
impl PreparedNamedTensorBundleView {
    pub fn prepare() -> Result<Self, String> {
        let ty = crate::named_tensor_catalog::checked_named_tensor_type()?;
        if !ty.invariants.is_empty() {
            return Err("borrowed tensor admission requires prepared Source invariants".into());
        }
        Ok(Self {
            type_bytes: ty
                .value_type
                .canonical_bytes()
                .map_err(|e| format!("tensor Type: {e:?}"))?,
            contracts: ty.value_contracts,
        })
    }
    /// Borrows canonical input and uses fixed-capacity descriptor storage.
    pub fn read<'a>(
        &self,
        canonical: &'a [u8],
    ) -> Result<NamedTensorBundleView<'a>, NamedTensorViewRefusal> {
        let value = validate_canonical_structured_value(canonical)
            .map_err(|_| NamedTensorViewRefusal::Encoding)?;
        if value.type_bytes() != self.type_bytes {
            return Err(NamedTensorViewRefusal::Encoding);
        }
        validate_native_contracts_borrowed(value, &self.contracts)
            .map_err(|_| NamedTensorViewRefusal::Encoding)?;
        let count = value
            .collection_length()
            .map_err(|_| NamedTensorViewRefusal::Encoding)? as usize;
        if count > MAXIMUM_NAMED_TENSORS {
            return Err(NamedTensorViewRefusal::Encoding);
        }
        let mut result = NamedTensorBundleView {
            canonical,
            values: [None; MAXIMUM_NAMED_TENSORS],
            count,
            tensor_bytes: 0,
        };
        for index in 0..count {
            let item = value
                .collection_index(index as u16)
                .map_err(|_| NamedTensorViewRefusal::Encoding)?
                .ok_or(NamedTensorViewRefusal::Encoding)?;
            let named = read_tensor(item)?;
            if result.values[..index]
                .iter()
                .flatten()
                .any(|v| v.name == named.name)
            {
                return Err(NamedTensorViewRefusal::DuplicateName);
            }
            result.tensor_bytes = result
                .tensor_bytes
                .checked_add(named.byte_count)
                .ok_or(NamedTensorViewRefusal::Tensor(TensorRefusal::ShapeOverflow))?;
            result.values[index] = Some(named);
        }
        Ok(result)
    }
}

pub struct NamedTensorBundleView<'a> {
    canonical: &'a [u8],
    values: [Option<NamedTensorView<'a>>; MAXIMUM_NAMED_TENSORS],
    count: usize,
    tensor_bytes: u64,
}
impl<'a> NamedTensorBundleView<'a> {
    pub fn values(&self) -> impl ExactSizeIterator<Item = &NamedTensorView<'a>> {
        self.values[..self.count]
            .iter()
            .map(|v| v.as_ref().expect("admitted descriptor"))
    }
    pub fn tensor_bytes(&self) -> u64 {
        self.tensor_bytes
    }
    pub fn semantic_digest(&self) -> [u8; 32] {
        conduit_core::semantic_digest(crate::NAMED_TENSOR_BUNDLE_INFO_ID, self.canonical)
    }
}
#[derive(Clone, Copy)]
pub struct NamedTensorView<'a> {
    pub name: &'a str,
    pub element: TensorElement,
    dimensions: [u64; 8],
    rank: usize,
    axes: View<'a>,
    pub content_digest: [u8; 32],
    pub backing: TensorBackingView<'a>,
    byte_count: u64,
}
#[derive(Clone, Copy)]
pub enum TensorBackingView<'a> {
    Inline(&'a [u8]),
    Resource(conduit_core::EncodedResourceReference<'a>),
}
impl<'a> NamedTensorView<'a> {
    pub fn byte_count(&self) -> u64 {
        self.byte_count
    }
    pub fn dimensions(&self) -> &[u64] {
        &self.dimensions[..self.rank]
    }
    /// Exact typed axis descriptors, including roles, identities and units.
    pub fn axes(&self) -> View<'a> {
        self.axes
    }
}
fn field<'a>(value: View<'a>, name: &str) -> Result<View<'a>, NamedTensorViewRefusal> {
    value
        .representation()
        .and_then(|v| v.record_field(name))
        .map_err(|_| NamedTensorViewRefusal::Encoding)?
        .ok_or(NamedTensorViewRefusal::Encoding)
}
fn read_tensor(value: View<'_>) -> Result<NamedTensorView<'_>, NamedTensorViewRefusal> {
    let name = core::str::from_utf8(
        field(value, "name")?
            .primitive_representation_bytes(conduit_core::TEXT_INFO_ID)
            .map_err(encoding)?,
    )
    .map_err(encoding)?;
    if name.is_empty() || name.len() > 128 || name.as_bytes().contains(&0) {
        return Err(NamedTensorViewRefusal::Name);
    }
    let value = field(value, "value")?;
    let element = match field(value, "element")?.variant_tag().map_err(encoding)? {
        "i8" => TensorElement::I8,
        "u8" => TensorElement::U8,
        "i16" => TensorElement::I16,
        "u16" => TensorElement::U16,
        "i24" => TensorElement::I24,
        "i32" => TensorElement::I32,
        "u32" => TensorElement::U32,
        "i64" => TensorElement::I64,
        "u64" => TensorElement::U64,
        "f32" => TensorElement::F32,
        "f64" => TensorElement::F64,
        _ => return Err(NamedTensorViewRefusal::Encoding),
    };
    let dimensions_view = field(value, "dimensions")?;
    let rank = dimensions_view.collection_length().map_err(encoding)? as usize;
    if !(1..=8).contains(&rank) {
        return Err(NamedTensorViewRefusal::Tensor(
            TensorRefusal::RankOutOfBounds,
        ));
    }
    let mut dimensions = [0; 8];
    let mut elements = 1_u64;
    for (index, dimension) in dimensions[..rank].iter_mut().enumerate() {
        let v = dimensions_view
            .collection_index(index as u16)
            .map_err(encoding)?
            .ok_or(NamedTensorViewRefusal::Encoding)?;
        *dimension = u64::from_le_bytes(
            v.primitive_representation_bytes("value/u64")
                .map_err(encoding)?
                .try_into()
                .map_err(encoding)?,
        );
        if *dimension == 0 {
            return Err(NamedTensorViewRefusal::Tensor(TensorRefusal::ZeroDimension));
        }
        elements = elements
            .checked_mul(*dimension)
            .ok_or(NamedTensorViewRefusal::Tensor(TensorRefusal::ShapeOverflow))?;
    }
    let bytes = elements
        .checked_mul(element.byte_width())
        .ok_or(NamedTensorViewRefusal::Tensor(TensorRefusal::ShapeOverflow))?;
    if bytes > MAXIMUM_TENSOR_BYTES {
        return Err(NamedTensorViewRefusal::Tensor(
            TensorRefusal::ByteBoundExceeded,
        ));
    }
    let axes = field(value, "axes")?;
    if axes.collection_length().map_err(encoding)? as usize != rank {
        return Err(NamedTensorViewRefusal::Tensor(
            TensorRefusal::AxisCountMismatch,
        ));
    }
    let mut content_digest = [0; 32];
    if field(value, "content-digest")?
        .copy_octet_collection("value/u8", &mut content_digest)
        .map_err(encoding)?
        != 32
    {
        return Err(NamedTensorViewRefusal::Encoding);
    }
    let backing = field(value, "backing")?
        .representation()
        .map_err(encoding)?;
    let backing = if let Some(inline) = backing.variant_payload("inline").map_err(encoding)? {
        let payload = inline
            .primitive_representation_bytes(conduit_core::BYTES_INFO_ID)
            .map_err(encoding)?;
        if payload.len() as u64 != bytes {
            return Err(NamedTensorViewRefusal::Tensor(
                TensorRefusal::PayloadLengthMismatch,
            ));
        }
        if tensor_content_digest(payload) != content_digest {
            return Err(NamedTensorViewRefusal::Tensor(
                TensorRefusal::ContentIdentityMismatch,
            ));
        }
        TensorBackingView::Inline(payload)
    } else {
        let resource = backing
            .variant_payload("resource")
            .map_err(encoding)?
            .ok_or(NamedTensorViewRefusal::Encoding)?;
        let encoded = resource
            .primitive_representation_bytes(conduit_core::RESOURCE_REFERENCE_INFO_ID)
            .map_err(encoding)?;
        let reference = conduit_core::BoundedResourceRef::validate_encoded(encoded)
            .map_err(|_| NamedTensorViewRefusal::Tensor(TensorRefusal::InvalidResource))?;
        if reference.content_profile != element.resource_profile() {
            return Err(NamedTensorViewRefusal::Tensor(
                TensorRefusal::ResourceProfileMismatch,
            ));
        }
        if reference.extent.bytes != bytes || reference.extent.items != Some(elements) {
            return Err(NamedTensorViewRefusal::Tensor(
                TensorRefusal::ResourceExtentMismatch,
            ));
        }
        if reference.identity.digest() != content_digest {
            return Err(NamedTensorViewRefusal::Tensor(
                TensorRefusal::ContentIdentityMismatch,
            ));
        }
        TensorBackingView::Resource(reference)
    };
    Ok(NamedTensorView {
        name,
        element,
        dimensions,
        rank,
        axes,
        content_digest,
        backing,
        byte_count: bytes,
    })
}

fn encoding<T>(_: T) -> NamedTensorViewRefusal {
    NamedTensorViewRefusal::Encoding
}
