#![cfg(feature = "fixed-numeric-owners")]
use conduit_ai::{
    fixed_numeric_catalog::fixed_numeric_type,
    fixed_numeric_codec::FixedF32VectorCodec,
    fixed_numeric_float_integer::{FLOAT_INTEGER_IMPLEMENTATION, FixedFloatIntegerBack},
    fixed_numeric_i16_codec::FixedI16VectorCodec,
    fixed_numeric_index_codec::FixedU16IndexCodec,
};
#[path = "../src/numeric_allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[test]
fn codec_inventory_counts_actual_capacity_without_allocation() {
    let f32ty = fixed_numeric_type("NumericF32Vector160").unwrap();
    let i16ty = fixed_numeric_type("NumericI16Vector160").unwrap();
    let (result, o) = allocation_probe::observe(|| FixedF32VectorCodec::<160>::prepare(&f32ty));
    let codec = result.unwrap();
    assert_eq!(o.live_bytes, codec.local_accounted_heap_bytes());
    let (value, o) = allocation_probe::observe(|| codec.local_accounted_heap_bytes());
    assert!(value > 0);
    assert_eq!((o.allocations, o.reallocations), (0, 0));
    let (result, o) = allocation_probe::observe(|| FixedI16VectorCodec::<160>::prepare(&i16ty));
    let codec = result.unwrap();
    assert_eq!(o.live_bytes, codec.local_accounted_heap_bytes());
    let ty = conduit_core::StructuredInfoType::leaf(conduit_core::kind_id("value/u16")).unwrap();
    let (result, o) = allocation_probe::observe(|| FixedU16IndexCodec::<1>::prepare(&ty));
    let codec = result.unwrap();
    assert_eq!(o.live_bytes, codec.local_accounted_heap_bytes());
}
#[test]
#[ignore = "requires exact archived FARGAN artifact root"]
fn actual_archived_float_integer_back_inventory_includes_box_root_separately() {
    let project = std::path::PathBuf::from(
        std::env::var_os("CONDUIT_FARGAN_PREPARATION_ARTIFACT_ROOT")
            .expect("set exact archived FARGAN artifact root"),
    );
    let plan: conduit_core::Plan = serde_json::from_slice(
        &std::fs::read(
            project.join("outputs/committed-common-fargan-greeting/sealed-epoch-plan.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let gear = plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .find(|g| g.implementation_id.as_str() == FLOAT_INTEGER_IMPLEMENTATION)
        .unwrap();
    let (result, o) = allocation_probe::observe(|| {
        FixedFloatIntegerBack::prepare_planned::<16>(gear, 4, true).map(Box::new)
    });
    let back = result.unwrap();
    let local = back.local_accounted_heap_bytes();
    assert_eq!(
        o.live_bytes,
        local + std::mem::size_of::<FixedFloatIntegerBack>()
    );
    let (_, getter) = allocation_probe::observe(|| back.local_accounted_heap_bytes());
    assert_eq!((getter.allocations, getter.reallocations), (0, 0));
    println!(
        "actual original float-i16 local={local} box={} observed={}",
        std::mem::size_of::<FixedFloatIntegerBack>(),
        o.live_bytes
    );
}
#[test]
fn full_tensor_descriptor_inventory_counts_spare_sequences_and_child_strings() {
    use conduit_data::{TensorAxis, TensorAxisRole, TensorBacking, TensorElement, TensorValue};
    use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence};
    let (result, o) = allocation_probe::observe(|| {
        let mut identity = String::with_capacity(53);
        identity.push_str("axis");
        let mut role = String::with_capacity(59);
        role.push_str("custom");
        let mut axes = Vec::with_capacity(7);
        axes.push(TensorAxis {
            identity: Some(identity),
            role: TensorAxisRole::other(role).unwrap(),
            unit: None,
        });
        let mut dimensions = Vec::with_capacity(6);
        dimensions.push(1);
        TensorValue {
            axes: BoundedSequence::try_from_iter(axes).unwrap(),
            dimensions: BoundedSequence::try_from_iter(dimensions).unwrap(),
            backing: TensorBacking::Inline(BoundedBytes::new(&[0, 0, 0, 0]).unwrap()),
            element: TensorElement::F32,
            content_digest: conduit_data::tensor_content_digest(&[0, 0, 0, 0]),
        }
    });
    let tensor = result;
    let bytes = conduit_ai::fixed_tensor_resource::tensor_descriptor_owned_heap_bytes(&tensor);
    assert_eq!(bytes, o.live_bytes);
    let (_, getter) = allocation_probe::observe(|| {
        conduit_ai::fixed_tensor_resource::tensor_descriptor_owned_heap_bytes(&tensor)
    });
    assert_eq!((getter.allocations, getter.reallocations), (0, 0));
    println!("tensor descriptor actual requested retained={bytes}");
}
