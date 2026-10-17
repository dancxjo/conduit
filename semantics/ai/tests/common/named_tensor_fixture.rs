use conduit_data::{
    tensor_content_digest, NamedTensor, TensorAxis, TensorAxisRole, TensorBacking, TensorElement,
    TensorValue,
};
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence};

pub fn tensor(name: &str, element: TensorElement, bytes: &[u8]) -> NamedTensor {
    NamedTensor {
        name: name.into(),
        value: TensorValue {
            element,
            dimensions: BoundedSequence::try_from_iter([bytes.len() as u64 / element.byte_width()])
                .unwrap(),
            axes: BoundedSequence::try_from_iter([axis()]).unwrap(),
            content_digest: tensor_content_digest(bytes),
            backing: TensorBacking::Inline(BoundedBytes::new(bytes).unwrap()),
        },
    }
}
fn axis() -> TensorAxis {
    TensorAxis {
        role: TensorAxisRole::Feature,
        identity: None,
        unit: None,
    }
}
