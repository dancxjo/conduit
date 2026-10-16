//! Independent refusal and allocation checks for the shared admitted signature path.
use conduit_ai::{ModelTensorIoRefusal as R, PreparedModelTensorIo};
use conduit_data::{
    NamedTensor, NamedTensorBundle, PreparedNamedTensorBundleView, TensorAxis, TensorAxisRole,
    TensorElement,
};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "common/model_signature_fixture.rs"]
mod fixture;
#[path = "common/named_tensor_fixture.rs"]
mod values;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn input() -> Vec<NamedTensor> {
    vec![
        values::tensor("identifiers", TensorElement::I64, &[0; 16]),
        values::tensor("gain", TensorElement::F32, &[0; 4]),
    ]
}
#[test]
fn admitted_mixed_named_inputs_and_outputs_reuse_signature_without_allocation() {
    let owner = fixture::signature();
    let signature = PreparedModelTensorIo::prepare(&owner).unwrap();
    let reader = PreparedNamedTensorBundleView::prepare().unwrap();
    let mut inputs = input();
    inputs.reverse();
    let bytes = NamedTensorBundle::from_values(inputs)
        .unwrap()
        .encode()
        .unwrap();
    let output = NamedTensorBundle::from_values(vec![values::tensor(
        "prediction",
        TensorElement::F32,
        &[0; 16],
    )])
    .unwrap()
    .encode()
    .unwrap();
    let (_, storage) = allocation_probe::observe(|| {
        for _ in 0..100 {
            signature
                .validate_inputs(&reader.read(&bytes).unwrap())
                .unwrap();
            signature
                .validate_outputs(&reader.read(&output).unwrap())
                .unwrap();
        }
    });
    assert_eq!((storage.allocations, storage.reallocations), (0, 0));
    assert_eq!(signature.signature(), &owner);
}
#[test]
fn owned_and_borrowed_paths_preserve_exact_refusal_categories() {
    let owner = fixture::signature();
    let signature = PreparedModelTensorIo::prepare(&owner).unwrap();
    let reader = PreparedNamedTensorBundleView::prepare().unwrap();
    let cases = [
        (
            {
                let mut v = input();
                v.pop();
                v
            },
            R::MissingPort,
        ),
        (
            {
                let mut v = input();
                v[0].name = "unknown".into();
                v
            },
            R::UnknownPort,
        ),
        (
            {
                let mut v = input();
                v[0] = values::tensor("identifiers", TensorElement::F32, &[0; 8]);
                v
            },
            R::Dtype,
        ),
        (
            {
                let mut v = input();
                v[0] = values::tensor("identifiers", TensorElement::I64, &[0; 264]);
                v
            },
            R::Dimension,
        ),
        (
            {
                let mut v = input();
                v[0].value.axes = BoundedSequence::try_from_iter([TensorAxis {
                    role: TensorAxisRole::Time,
                    identity: None,
                    unit: None,
                }])
                .unwrap();
                v
            },
            R::AxisRole,
        ),
        (
            {
                let mut v = input();
                v[0].value.dimensions = BoundedSequence::try_from_iter([1, 2]).unwrap();
                v[0].value.axes = BoundedSequence::try_from_iter([
                    TensorAxis {
                        role: TensorAxisRole::Feature,
                        identity: None,
                        unit: None,
                    },
                    TensorAxis {
                        role: TensorAxisRole::Feature,
                        identity: None,
                        unit: None,
                    },
                ])
                .unwrap();
                v
            },
            R::Rank,
        ),
    ];
    for (values, expected) in cases {
        let owned = NamedTensorBundle::from_values(values).unwrap();
        assert_eq!(owner.validate_named_inputs(&owned), Err(expected.clone()));
        let bytes = owned.encode().unwrap();
        let view = reader.read(&bytes).unwrap();
        let (actual, storage) = allocation_probe::observe(|| signature.validate_inputs(&view));
        assert_eq!(actual, Err(expected));
        assert_eq!((storage.allocations, storage.reallocations), (0, 0));
    }
}
