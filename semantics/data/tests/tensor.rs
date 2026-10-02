use conduit_core::{
    kind_id, BoundedResourceRef, QuantityUnit, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};
use conduit_data::*;
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};

fn axes() -> Vec<TensorAxis> {
    vec![
        TensorAxis {
            role: TensorAxisRole::Time,
            identity: Some("speech-frame".into()),
            unit: Some(QuantityUnit::Millisecond),
        },
        TensorAxis {
            role: TensorAxisRole::SpatialCoordinate,
            identity: Some("articulatory-coordinate".into()),
            unit: Some(QuantityUnit::Millimeter),
        },
    ]
}

fn inline() -> TensorValue {
    let payload: Vec<u8> = [0.0_f32, 1.25, -2.5, 3.0, 4.5, 5.75]
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect();
    let content_digest = tensor_content_digest(&payload);
    TensorValue {
        element: TensorElement::F32,
        dimensions: BoundedSequence::try_from_iter([3, 2]).unwrap(),
        axes: BoundedSequence::try_from_iter(axes()).unwrap(),
        content_digest,
        backing: TensorBacking::Inline(BoundedBytes::new(&payload).unwrap()),
    }
}

fn reference(bytes: u64, items: u64, profile: &str, digest: [u8; 32]) -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest(digest),
        content_profile: kind_id(profile),
        access_class: ResourceClassId::from("content/read@1"),
        extent: ResourceExtent {
            bytes,
            items: Some(items),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([2; 32]),
            expires_at: None,
        },
    }
}

#[test]
fn time_by_articulatory_coordinate_float_tensor_round_trips_canonically() {
    let tensor = inline();
    let encoded = tensor.encode().unwrap();
    assert_eq!(TensorValue::decode(&encoded), Ok(tensor.clone()));
    assert_eq!(tensor.summary().unwrap().bytes, 24);
    assert_ne!(tensor.semantic_digest().unwrap(), [0; 32]);
}

#[test]
fn resource_backing_preserves_tensor_meaning_without_host_placement() {
    let direct = inline();
    let external = TensorValue {
        backing: TensorBacking::Resource(reference(
            24,
            6,
            direct.resource_profile(),
            direct.content_digest,
        )),
        ..direct.clone()
    };
    external.validate().unwrap();
    assert_eq!(
        external.summary().unwrap().dimensions,
        direct.summary().unwrap().dimensions
    );
    assert_eq!(external.semantic_digest(), direct.semantic_digest());
    assert!(
        external.encode().unwrap().len()
            < usize::try_from(external.byte_count().unwrap()).unwrap() + 512
    );
    let debug = format!("{external:?}");
    assert!(!debug.contains("cuda"));
    assert!(!debug.contains("cpu"));
    assert!(!debug.contains("file://"));
}

#[test]
fn malformed_shape_axis_payload_and_reference_cases_refuse_exactly() {
    let mut value = inline();
    value.dimensions = BoundedSequence::new();
    assert_eq!(value.validate(), Err(TensorRefusal::RankOutOfBounds));
    value = inline();
    value.dimensions[0] = 0;
    assert_eq!(value.validate(), Err(TensorRefusal::ZeroDimension));
    value = inline();
    value.dimensions = BoundedSequence::try_from_iter([u64::MAX, 2]).unwrap();
    assert_eq!(value.validate(), Err(TensorRefusal::ShapeOverflow));
    value = inline();
    value.axes.pop();
    assert_eq!(value.validate(), Err(TensorRefusal::AxisCountMismatch));
    value = inline();
    value.backing = TensorBacking::Inline(BoundedBytes::new(&[0; 23]).unwrap());
    assert_eq!(value.validate(), Err(TensorRefusal::PayloadLengthMismatch));
    value = inline();
    value.backing =
        TensorBacking::Resource(reference(24, 6, "tensor/wrong@1", value.content_digest));
    assert_eq!(
        value.validate(),
        Err(TensorRefusal::ResourceProfileMismatch)
    );
    value = inline();
    value.backing = TensorBacking::Resource(reference(
        20,
        5,
        value.resource_profile(),
        value.content_digest,
    ));
    assert_eq!(value.validate(), Err(TensorRefusal::ResourceExtentMismatch));
    let mut encoded = inline().encode().unwrap();
    encoded[0] = 99;
    assert_eq!(
        TensorValue::decode(&encoded),
        Err(TensorRefusal::UnsupportedEncodingVersion)
    );
}

#[test]
fn tensor_ports_survive_an_ordinary_checked_and_expanded_plot() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_tensor_catalogs(&mut startup, &mut profile).unwrap();
    let source = "plot trajectory {\n  source: data/tensor-fixture\n  identity: data/tensor-identity\n  source >> identity\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_plot(&checked, "trajectory", &profile).unwrap();
    assert_eq!(expanded.connections.len(), 1);
    assert_eq!(expanded.connections[0].value_kind.as_str(), TENSOR_INFO_ID);
    assert_eq!(
        inline().axes[1].identity.as_deref(),
        Some("articulatory-coordinate")
    );
}
