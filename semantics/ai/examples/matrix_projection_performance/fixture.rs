//! Synthetic owned bytes and validated tensor content, not a Host authority grant.
use conduit_ai::fixed_tensor::FixedMatrixOrder;
use conduit_core::*;
use conduit_data::*;
use conduit_plot::rust_binding::BoundedSequence;
pub(super) fn make<const I: usize, const O: usize>(
    order: FixedMatrixOrder,
) -> (TensorValue, Vec<u8>, [f32; I]) {
    let mut seed = 0x913a6579_u32;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as i32 as f32) / i32::MAX as f32 * 0.1
    };
    let bytes: Vec<_> = (0..I * O).flat_map(|_| next().to_le_bytes()).collect();
    let input = core::array::from_fn(|_| next());
    let dimensions = match order {
        FixedMatrixOrder::InputMajor => [I as u64, O as u64],
        FixedMatrixOrder::OutputMajor => [O as u64, I as u64],
    };
    let digest = tensor_content_digest(&bytes);
    let tensor = TensorValue {
        element: TensorElement::F32,
        dimensions: BoundedSequence::try_from_iter(dimensions).unwrap(),
        axes: BoundedSequence::try_from_iter(dimensions.iter().map(|_| TensorAxis {
            role: TensorAxisRole::Feature,
            identity: None,
            unit: None,
        }))
        .unwrap(),
        content_digest: digest,
        backing: TensorBacking::Resource(BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest(digest),
            content_profile: kind_id("tensor/elements-ieee754-f32-le@1"),
            access_class: ResourceClassId::from("benchmark/owned-bytes@1"),
            extent: ResourceExtent {
                bytes: bytes.len() as u64,
                items: Some(dimensions.iter().product()),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([1; 32]),
                expires_at: None,
            },
        }),
    };
    (tensor, bytes, input)
}
