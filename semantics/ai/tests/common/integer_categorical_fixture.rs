use conduit_ai::{integer_categorical::*, integer_categorical_step::*, *};
use conduit_core::*;
use conduit_data::{TensorAxisRole, TensorElement};
use std::sync::Arc;
pub fn fixture() -> (Arc<AdmittedModelResource>, ResourceContentOffer) {
    let mut bytes = b"CI16SUM1".to_vec();
    for d in [3u32, 2, 2] {
        bytes.extend_from_slice(&d.to_le_bytes());
    }
    for w in [-1i16, 2, 4, 5, -2, 3] {
        bytes.extend_from_slice(&w.to_le_bytes());
    }
    fixture_model(bytes, 2, 2)
}
pub fn fixture_model(
    bytes: Vec<u8>,
    lookups: u64,
    outputs: u64,
) -> (Arc<AdmittedModelResource>, ResourceContentOffer) {
    let port = |name: &str, element, count| {
        ModelPortConstraint::new(
            ModelPortIdentity::new(name.into()).unwrap(),
            ModelPortPresence::Required,
            ModelSemanticKind::new(format!("test/numeric/{name}@1")).unwrap(),
            ModelValueConstraint::tensor(
                ModelTensorConstraint::from_parts(
                    vec![element],
                    vec![ModelAxisConstraint::new(
                        ModelDimensionConstraint::fixed(count).unwrap(),
                        TensorAxisRole::Feature,
                    )
                    .unwrap()],
                    count * 8,
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
    };
    let signature = ModelSignature::from_parts(
        "test/numeric-categorical@1".into(),
        1,
        vec![ModelOperation::Infer],
        vec![port("features", TensorElement::U64, lookups)],
        vec![port("scores", TensorElement::I64, outputs)],
    )
    .unwrap();
    let digest = model_content_digest(&bytes);
    let reference = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest(digest),
        content_profile: CATEGORICAL_I16_FORMAT.into(),
        access_class: "test/model/read@1".into(),
        extent: ResourceExtent {
            bytes: bytes.len() as u64,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest(digest),
            expires_at: None,
        },
    };
    let artifact = ModelArtifact {
        architecture_profile: CATEGORICAL_I16_ARCHITECTURE.into(),
        format_profile: CATEGORICAL_I16_FORMAT.into(),
        precision_profile: CATEGORICAL_I16_PRECISION.into(),
        state_schema_version: 1,
        signature_identity: signature.semantic_digest().unwrap(),
        content: reference.clone(),
    };
    let binding = ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        handle: "test/model-handle".into(),
        authority_contract: MODEL_READ_AUTHORITY.into(),
        authority_grant: "test/model-grant".into(),
        maximum_bytes: reference.extent.bytes,
        maximum_items: Some(1),
        availability: ResourceReferenceAvailability::Available,
    };
    let admitted = Arc::new(
        AdmittedModelResource::adopt(artifact, signature, Arc::from(bytes), &binding).unwrap(),
    );
    let residence = ResourceContentOffer {
        contract: ResourceContentRequirement {
            identity: reference.identity,
            version: reference.lifetime.version,
            content_profile: reference.content_profile,
            maximum_bytes: reference.extent.bytes as u32,
            maximum_items: 1,
            retention: ResourceRetention::Play,
            sharing: ResourceSharing::ImmutableReadMany,
            access: ResourceAccessMode::ReadPublished,
            generation_slots: 1,
            reader_leases: 16,
            publication_slots: 0,
            sensitive: false,
        },
        owner_host: "categorical-test".into(),
        owner_boot: "categorical-test-boot".into(),
        base_id: "categorical-test-base".into(),
        residence_profile: "test/owned-immutable-memory@1".into(),
    };
    (admitted, residence)
}
pub fn profile() -> Arc<PreparedCategoricalStep> {
    let (model, residence) = fixture();
    Arc::new(PreparedCategoricalStep::prepare(model, "test/model-pool".into(), residence).unwrap())
}
pub fn indices(profile: &PreparedCategoricalStep, values: &[u64]) -> Vec<u8> {
    let StructuredInfoTypeShape::Collection { element, .. } = profile.indices_type().shape() else {
        panic!("collection")
    };
    StructuredInfoValue::collection(
        profile.indices_type().clone(),
        values
            .iter()
            .map(|v| StructuredInfoValue::leaf(element.clone(), v.to_le_bytes().to_vec()).unwrap())
            .collect(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
pub fn scores(bytes: &[u8]) -> Vec<i64> {
    let value = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
    let StructuredInfoValueShape::Collection(values) = value.shape() else {
        panic!("collection")
    };
    values
        .iter()
        .map(|v| {
            let StructuredInfoValueShape::Leaf(bytes) = v.shape() else {
                panic!("leaf")
            };
            i64::from_le_bytes(bytes.try_into().unwrap())
        })
        .collect()
}
