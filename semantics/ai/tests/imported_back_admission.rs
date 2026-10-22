use conduit_ai::{
    imported_back_admission::*,
    imported_model_descriptor::*,
    imported_onnx::*,
    onnx_metadata::{inspect_onnx_operators, OnnxMetadataRefusal, OnnxOperator},
    *,
};
use sha2::{Digest, Sha256};
#[path = "common/onnx_fixture.rs"]
mod fixture;
#[path = "common/onnx_resource_fixture.rs"]
mod resource_fixture;
#[path = "common/model_signature_fixture.rs"]
mod signature_fixture;

fn imported() -> ImportedOnnxResource {
    let signature = signature_fixture::signature();
    let graph = fixture::model(&[]);
    let binding = |semantic: &str, graph: &str| ImportedPortBinding {
        semantic_port: semantic.into(),
        graph_name: graph.into(),
    };
    let sidecar = OnnxSidecar {
        graph_sha256: Sha256::digest(&graph).into(),
        graph_name: "tiny-mixed-input".into(),
        signature_identity: signature.semantic_digest().unwrap(),
        inputs: vec![binding("identifiers", "tokens"), binding("gain", "scale")],
        outputs: vec![binding("prediction", "result")],
    };
    ImportedOnnxResource::bind(resource_fixture::resource(signature, graph), &sidecar).unwrap()
}
fn profile(resource: &ImportedOnnxResource) -> ExactImportedBackProfile {
    ExactImportedBackProfile {
        profile_identity: "fixture/add-exact@1".into(),
        implementation_identity: "fixture/add".into(),
        adapter_artifact_identity: "fixture:add:sha256:123".into(),
        runtime: ModelComputeRuntimeIdentity {
            provider_name: "fixture".into(),
            runtime_name: "bounded-add".into(),
            runtime_version: "1".into(),
            runtime_build_identity: "source:fixture".into(),
            adapter_artifact_identity: "fixture:add:sha256:123".into(),
            device_evidence: "cpu:fixture".into(),
            precision_profile: "f32".into(),
        },
        execution_profile: "model/fixture@1".into(),
        checkpoint: ImportedCheckpointIdentity::from_resource(resource),
        minimum_ir: 9,
        maximum_ir: 9,
        opsets: vec![ImportedOpsetRange {
            domain: "".into(),
            minimum: 18,
            maximum: 18,
        }],
        operators: vec![OnnxOperator {
            domain: "".into(),
            name: "Add".into(),
        }],
    }
}
fn admit(
    resource: &ImportedOnnxResource,
    profile: &ExactImportedBackProfile,
) -> Result<ImportedBackAdmissionReceipt, ImportedBackRefusal> {
    admit_imported_back(
        resource,
        profile,
        &profile.implementation_identity,
        &profile.adapter_artifact_identity,
        &profile.runtime,
        &profile.execution_profile,
    )
}
#[test]
fn exact_checkpoint_receipt_and_readable_native_property_round_trip() {
    let resource = imported();
    let profile = profile(&resource);
    let receipt = admit(&resource, &profile).unwrap();
    let descriptor = ImportedModelInspection::from_resource(&resource, &receipt).unwrap();
    let property = descriptor.to_property().unwrap();
    assert_eq!(
        ImportedModelInspection::from_property(&property).unwrap(),
        descriptor
    );
    assert_eq!(descriptor.inputs[0].semantic_port, "identifiers");
    assert_eq!(descriptor.inputs[0].graph_name, "tokens");
    assert_eq!(descriptor.outputs[0].semantic_port, "prediction");
    assert_eq!(descriptor.outputs[0].graph_name, "result");
    assert_eq!(descriptor.admission_identity, receipt.identity());
    assert_eq!(descriptor.checkpoint.graph, resource.metadata().sha256);
    let native =
        conduit_core::StructuredInfoValue::from_canonical_bytes(property.canonical_value())
            .unwrap();
    let conduit_core::StructuredInfoValueShape::Record(fields) = native.shape() else {
        panic!("record");
    };
    assert!(fields.iter().any(|f| f.name() == "inputs"));
    assert!(!fields.iter().any(|f| f.name() == "tensor-payload"));
}
#[test]
fn typed_profile_refuses_versions_operators_and_unreviewed_checkpoint_maps() {
    let resource = imported();
    let original = profile(&resource);
    let cases = [
        (
            {
                let mut p = original.clone();
                p.minimum_ir = 10;
                p.maximum_ir = 10;
                p
            },
            ImportedBackRefusal::UnsupportedIr,
        ),
        (
            {
                let mut p = original.clone();
                p.opsets[0].maximum = 17;
                p.opsets[0].minimum = 17;
                p
            },
            ImportedBackRefusal::UnsupportedOpset {
                domain: "".into(),
                version: 18,
            },
        ),
        (
            {
                let mut p = original.clone();
                p.operators[0].name = "Identity".into();
                p
            },
            ImportedBackRefusal::UnsupportedOperator(OnnxOperator {
                domain: "".into(),
                name: "Add".into(),
            }),
        ),
        (
            {
                let mut p = original.clone();
                p.checkpoint.graph = [1; 32];
                p
            },
            ImportedBackRefusal::Graph,
        ),
        (
            {
                let mut p = original.clone();
                p.checkpoint.signature = [1; 32];
                p
            },
            ImportedBackRefusal::Signature,
        ),
        (
            {
                let mut p = original.clone();
                p.checkpoint.sidecar = [1; 32];
                p
            },
            ImportedBackRefusal::Sidecar,
        ),
        (
            {
                let mut p = original.clone();
                p.checkpoint.descriptor = [1; 32];
                p
            },
            ImportedBackRefusal::Checkpoint,
        ),
    ];
    for (p, expected) in cases {
        assert_eq!(admit(&resource, &p).unwrap_err(), expected);
    }
}
#[test]
fn receipt_checks_actual_selected_back_artifact_runtime_and_execution_profile() {
    let resource = imported();
    let profile = profile(&resource);
    let receipt = admit(&resource, &profile).unwrap();
    let check = |implementation: &str,
                 artifact: &str,
                 runtime: &ModelComputeRuntimeIdentity,
                 execution: &str| {
        receipt.verify_selected(implementation, artifact, runtime, execution)
    };
    assert_eq!(
        check(
            "other",
            &profile.adapter_artifact_identity,
            &profile.runtime,
            &profile.execution_profile
        ),
        Err(ImportedBackRefusal::BackIdentity)
    );
    assert_eq!(
        check(
            &profile.implementation_identity,
            "other",
            &profile.runtime,
            &profile.execution_profile
        ),
        Err(ImportedBackRefusal::AdapterArtifact)
    );
    let mut runtime = profile.runtime.clone();
    runtime.runtime_build_identity = "other".into();
    assert_eq!(
        check(
            &profile.implementation_identity,
            &profile.adapter_artifact_identity,
            &runtime,
            &profile.execution_profile
        ),
        Err(ImportedBackRefusal::RuntimeIdentity)
    );
    assert_eq!(
        check(
            &profile.implementation_identity,
            &profile.adapter_artifact_identity,
            &profile.runtime,
            "other"
        ),
        Err(ImportedBackRefusal::ExecutionProfile)
    );
}
#[test]
fn malformed_profile_and_inspection_names_refuse_without_panicking() {
    let resource = imported();
    let mut p = profile(&resource);
    p.opsets.push(p.opsets[0].clone());
    assert_eq!(
        admit(&resource, &p).unwrap_err(),
        ImportedBackRefusal::InvalidProfile
    );
    let p = profile(&resource);
    let receipt = admit(&resource, &p).unwrap();
    let mut d = ImportedModelInspection::from_resource(&resource, &receipt).unwrap();
    d.inputs[1].graph_name = d.inputs[0].graph_name.clone();
    assert_eq!(
        d.to_property().unwrap_err(),
        ImportedInspectionRefusal::DuplicateName
    );
    d.inputs[1].graph_name = "x".repeat(129);
    assert_eq!(
        d.to_property().unwrap_err(),
        ImportedInspectionRefusal::Bound
    );
}
#[test]
fn bounded_operator_inspection_rejects_missing_duplicate_and_nested_operator_fields() {
    let graph = fixture::model(&[]);
    assert_eq!(
        inspect_onnx_operators(&graph, Sha256::digest(&graph).into()).unwrap(),
        [OnnxOperator {
            domain: "".into(),
            name: "Add".into()
        }]
    );
    for (node, expected) in [
        (
            fixture::bytes(1, b"tokens"),
            OnnxMetadataRefusal::MissingField,
        ),
        (
            [fixture::bytes(4, b"Add"), fixture::bytes(4, b"Sub")].concat(),
            OnnxMetadataRefusal::DuplicateField,
        ),
        (
            [fixture::bytes(4, b"Add"), fixture::bytes(5, &[])].concat(),
            OnnxMetadataRefusal::Unsupported,
        ),
    ] {
        let bytes = fixture::model(&fixture::bytes(1, &node));
        assert_eq!(
            inspect_onnx_operators(&bytes, Sha256::digest(&bytes).into()),
            Err(expected)
        );
    }
}

#[test]
fn a_different_valid_structured_property_is_not_an_imported_inspection() {
    use conduit_core::{
        kind_id, StructuredConfigurationValue, StructuredInfoType, StructuredInfoValue,
        TEXT_INFO_ID,
    };
    let ty = StructuredInfoType::leaf(kind_id(TEXT_INFO_ID)).unwrap();
    let bytes = StructuredInfoValue::leaf(ty.clone(), b"tokens are not a checked map".to_vec())
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let other =
        StructuredConfigurationValue::new(ty.profile().unwrap().value_kind().clone(), bytes)
            .unwrap();
    assert_eq!(
        ImportedModelInspection::from_property(&other),
        Err(ImportedInspectionRefusal::Schema)
    );
}
