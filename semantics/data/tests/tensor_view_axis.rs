//! Axis meaning remains borrowed and exact, including owner-defined identities.
use conduit_data::*;
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence, NativeRustBinding};
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

#[test]
fn every_axis_role_and_other_identity_matches_without_allocation() {
    let roles = [
        TensorAxisRole::Batch,
        TensorAxisRole::Time,
        TensorAxisRole::Feature,
        TensorAxisRole::Sensor,
        TensorAxisRole::SpatialCoordinate,
        TensorAxisRole::Frequency,
        TensorAxisRole::Channel,
        TensorAxisRole::other("coordinate-a".into()).unwrap(),
    ];
    let other = TensorAxisRole::other("coordinate-b".into()).unwrap();
    let reader = PreparedNamedTensorBundleView::prepare().unwrap();
    for role in &roles {
        let bytes = [0_u8];
        let owned = NamedTensorBundle::from_values(vec![NamedTensor {
            name: "input".into(),
            value: TensorValue {
                element: TensorElement::I8,
                dimensions: BoundedSequence::try_from_iter([1]).unwrap(),
                axes: BoundedSequence::try_from_iter([TensorAxis {
                    role: role.clone(),
                    identity: None,
                    unit: None,
                }])
                .unwrap(),
                content_digest: tensor_content_digest(&bytes),
                backing: TensorBacking::Inline(BoundedBytes::new(&bytes).unwrap()),
            },
        }])
        .unwrap()
        .encode()
        .unwrap();
        let view = reader.read(&owned).unwrap();
        let tensor = view.values().next().unwrap();
        let (_, storage) = allocation_probe::observe(|| {
            let actual = tensor.axis_role(0).unwrap();
            for expected in &roles {
                assert_eq!(actual.matches(expected), expected == role);
            }
            assert!(!actual.matches(&other));
            assert!(tensor.axis_role(1).is_err());
            assert!(tensor.axis_role(usize::MAX).is_err());
        });
        assert_eq!((storage.allocations, storage.reallocations), (0, 0));
    }
}
