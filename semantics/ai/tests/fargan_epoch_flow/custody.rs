//! Private signal-epoch model custody; admission is separate from startup/utterance execution.
use super::fixed_numeric_types;
use conduit_ai::{fixed_tensor_resource::AdmittedFixedTensorResource, *};
use conduit_core::*;
use conduit_data::*;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path, sync::Arc};

pub struct RetainedTensor {
    pub value_type: StructuredInfoType,
    pub resource: Arc<AdmittedFixedTensorResource>,
    pub binding: ResourceReferenceBinding,
}
pub struct RetainedSignalModel {
    pub model: Arc<AdmittedModelResource>,
    pub model_binding: ResourceReferenceBinding,
    pub resources: BTreeMap<String, RetainedTensor>,
    pub layout: Vec<u8>,
    pub raw_blob_sha256: String,
}
fn port(name: &str, width: u64, element: TensorElement, bytes: u64) -> ModelPortConstraint {
    ModelPortConstraint::from_parts(
        name.into(),
        "data/tensor@1".into(),
        ModelPortPresence::Required,
        ModelValueConstraint::tensor(
            ModelTensorConstraint::from_parts(
                vec![element],
                vec![ModelAxisConstraint {
                    role: TensorAxisRole::Feature,
                    dimension: ModelDimensionConstraint::fixed(width).unwrap(),
                }],
                width * bytes,
            )
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}
fn digest(hex: &str) -> [u8; 32] {
    assert_eq!(hex.len(), 64);
    std::array::from_fn(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
}
fn binding(
    reference: &BoundedResourceRef,
    authority: &str,
    name: &str,
) -> ResourceReferenceBinding {
    ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        handle: ResourceHandleId::from(format!("private-fargan/{name}")),
        authority_contract: AuthorityContractId::from(authority),
        authority_grant: AuthorityGrantId::from(format!("private-fargan-read/{name}")),
        maximum_bytes: reference.extent.bytes,
        maximum_items: reference.extent.items,
        availability: ResourceReferenceAvailability::Available,
    }
}
impl RetainedSignalModel {
    pub fn load(root: &Path) -> Self {
        let layout = include_bytes!("../../../../proof/fargan/signal-layout-f32.json").to_vec();
        let data: serde_json::Value = serde_json::from_slice(&layout).unwrap();
        assert_eq!(data["profile"], "speech/fargan-signal-epoch-layout-f32@1");
        assert_eq!(
            data["upstream_revision"],
            "503d81b138d76621aae4b12786e90de48aa8db3a"
        );
        assert_eq!(
            data["generated_c_sha256"],
            "68ba6e8731007f7184be862d7d1afe4cef096a7c9bcbfa115962340f725e004c"
        );
        assert_eq!(
            data["precision"],
            "number/ieee754-f32-libm-source-pcm16-nearest-away@1"
        );
        let bytes: Arc<[u8]> = Arc::from(std::fs::read(root.join("resources/f32.bin")).unwrap());
        assert_eq!(bytes.len(), 3_272_868);
        let raw_blob_sha256 = format!("{:x}", Sha256::digest(&bytes));
        assert_eq!(data["raw_blob_sha256"], raw_blob_sha256);
        // This logical signature describes the checked Source signal-epoch
        // wrapper, not the conditioner or the entire utterance/startup protocol.
        let signature = ModelSignature::from_parts(
            "speech/fargan-source-signal-epoch-i16@1".into(),
            1,
            vec![ModelOperation::Infer],
            vec![
                port("condition", 320, TensorElement::F32, 4),
                port("conditioned_period", 1, TensorElement::U16, 2),
                port("next_period", 1, TensorElement::U16, 2),
                port("state", 837, TensorElement::F32, 4),
            ],
            vec![
                port("pcm_i16", 160, TensorElement::I16, 2),
                port("next_state", 837, TensorElement::F32, 4),
                port("next_period", 1, TensorElement::U16, 2),
            ],
        )
        .unwrap();
        let version = ResourceVersionIdentity::from_digest(digest(
            data["generated_c_sha256"].as_str().unwrap(),
        ));
        let artifact = ModelArtifact {
            architecture_profile: "speech/fargan-xiph-503d81b1@1".into(),
            format_profile: "model/fargan-array-bundle-f32-le@1".into(),
            precision_profile: data["precision"].as_str().unwrap().into(),
            state_schema_version: 1,
            signature_identity: signature.semantic_digest().unwrap(),
            content: BoundedResourceRef {
                identity: ResourceSemanticIdentity::from_digest(model_content_digest(&bytes)),
                content_profile: kind_id("model/fargan-array-bundle-f32-le@1"),
                access_class: ResourceClassId::from("private-development-model/read@1"),
                extent: ResourceExtent {
                    bytes: bytes.len() as u64,
                    items: None,
                },
                lifetime: ResourceLifetime {
                    version,
                    expires_at: None,
                },
            },
        };
        let access = binding(&artifact.content, MODEL_READ_AUTHORITY, "model");
        let model =
            Arc::new(AdmittedModelResource::adopt(artifact, signature, bytes, &access).unwrap());
        let types = fixed_numeric_types().unwrap();
        let source = conduit_plot::parse_syntax_document(include_str!(
            "../../../speech/fargan_epoch_flow.conduit"
        ));
        let entry = source
            .plots
            .iter()
            .find(|p| p.name.text == "speech/flow-fargan-float-epoch")
            .unwrap();
        let expected: BTreeMap<_, _> = entry
            .front
            .runtime_ports
            .iter()
            .filter(|p| {
                p.value_type.text.starts_with("NumericF32MatrixRef")
                    || p.value_type.text.starts_with("NumericF32BiasRef")
            })
            .map(|p| (p.name.text.clone(), p.value_type.text.clone()))
            .collect();
        assert_eq!(expected.len(), 26);
        let mut resources = BTreeMap::new();
        for row in data["resources"].as_array().unwrap() {
            let name = row["port"].as_str().unwrap();
            let shape: Vec<u64> = row["shape"]
                .as_array()
                .unwrap()
                .iter()
                .map(|n| n.as_u64().unwrap())
                .collect();
            let offset = usize::try_from(row["offset"].as_u64().unwrap()).unwrap();
            let length = usize::try_from(row["bytes"].as_u64().unwrap()).unwrap();
            let end = offset.checked_add(length).unwrap();
            assert!(end <= model.bytes().len());
            assert_eq!(
                length,
                usize::try_from(shape.iter().product::<u64>()).unwrap() * 4
            );
            assert_eq!(
                format!("{:x}", Sha256::digest(&model.bytes()[offset..end])),
                row["raw_sha256"].as_str().unwrap()
            );
            let type_name = if shape.len() == 2 {
                format!("NumericF32MatrixRef{}x{}", shape[0], shape[1])
            } else {
                assert_eq!(shape.len(), 1);
                format!("NumericF32BiasRef{}", shape[0])
            };
            assert_eq!(expected[name], type_name, "Source resource slot {name}");
            let value_type = types
                .iter()
                .find(|t| t.name == type_name)
                .unwrap()
                .value_type
                .clone();
            let mut descriptor = super::fixtures::tensor(&shape, &model.bytes()[offset..end]);
            let TensorBacking::Resource(reference) = &mut descriptor.backing else {
                panic!("resource")
            };
            reference.lifetime.version = version;
            reference.access_class = ResourceClassId::from("private-development-tensor/read@1");
            let access = binding(
                reference,
                conduit_ai::fixed_numeric_preparation::TENSOR_READ_AUTHORITY,
                name,
            );
            let resource = Arc::new(
                AdmittedFixedTensorResource::adopt_shared_slice(
                    Arc::new(descriptor),
                    model.shared_storage(),
                    offset..end,
                    &access,
                )
                .unwrap(),
            );
            assert_eq!(
                resource.bytes().as_ptr(),
                model.bytes().as_ptr().wrapping_add(offset)
            );
            assert!(resources
                .insert(
                    name.into(),
                    RetainedTensor {
                        value_type,
                        resource,
                        binding: access,
                    }
                )
                .is_none());
        }
        assert_eq!(resources.len(), 26);
        Self {
            model,
            model_binding: access,
            resources,
            layout,
            raw_blob_sha256,
        }
    }
}

#[test]
#[ignore = "private pinned model bytes; set CONDUIT_FARGAN_MODEL_FIXTURE"]
fn pinned_model_retains_full_artifact_signature_and_one_shared_blob() {
    std::thread::Builder::new().stack_size(32*1024*1024).spawn(|| {
        let root=std::env::var("CONDUIT_FARGAN_MODEL_FIXTURE").unwrap();
        let retained=RetainedSignalModel::load(Path::new(&root));
        let first=retained.resources.values().next().unwrap();
        for resource in retained.resources.values() {conduit_ai::fixed_numeric_binding::FixedTensorPortBinding::prepare(&resource.value_type,resource.resource.tensor()).unwrap();assert!(first.resource.shares_storage_with(&resource.resource));assert_eq!(resource.resource.retained_storage_bytes(),3_272_868);}
        assert_eq!(retained.model.artifact().content_identity(),model_content_digest(retained.model.bytes()));
        assert_eq!(retained.model.artifact().descriptor_digest(retained.model.signature()).unwrap(),retained.model.descriptor_identity());
        assert_eq!(retained.raw_blob_sha256,"d35f0510da476716183a33caafe45cc095e48d496f06de2db7655e780a385e47");
        assert!(!retained.layout.is_empty());
        let base=retained.basis_material("template","handoff".as_bytes(),"feature".as_bytes());
        assert_ne!(base,retained.basis_material("changed template","handoff".as_bytes(),"feature".as_bytes()));
        assert_ne!(base,retained.basis_material("template","changed handoff".as_bytes(),"feature".as_bytes()));
        assert_ne!(base,retained.basis_material("template","handoff".as_bytes(),"changed feature".as_bytes()));
        assert_eq!(retained.model_binding.authority_contract.as_str(),MODEL_READ_AUTHORITY);
        for resource in retained.resources.values() {assert_eq!(resource.binding.maximum_bytes,resource.resource.bytes().len() as u64);}

        eprintln!("private signal model: single shared blob={}B, selected parameter bytes={}B, TensorValue inline descriptor={}B × {}; dynamic metadata/Arc headers and execution storage additional; custody only",retained.model.bytes().len(),retained.resources.values().map(|r|r.resource.bytes().len()).sum::<usize>(),core::mem::size_of::<TensorValue>(),retained.resources.len());
    }).unwrap().join().unwrap();
}

fn grant_value(grant: &ResourceReferenceBinding) -> serde_json::Value {
    serde_json::json!({"identity":grant.identity.digest(),"version":grant.version.digest(),"content_profile":grant.content_profile.as_str(),"access_class":grant.access_class.as_str(),"handle":grant.handle.as_str(),"authority_contract":grant.authority_contract.as_str(),"authority_grant":grant.authority_grant.as_str(),"maximum_bytes":grant.maximum_bytes,"maximum_items":grant.maximum_items,"availability":match grant.availability {ResourceReferenceAvailability::Available=>"available",ResourceReferenceAvailability::Lost=>"lost",ResourceReferenceAvailability::Stale=>"stale"}})
}
impl RetainedSignalModel {
    pub fn basis_material(
        &self,
        unbound_source: &str,
        handoff: &[u8],
        feature_evidence: &[u8],
    ) -> Vec<u8> {
        use conduit_plot::rust_binding::NativeRustBinding;
        let artifact = self.model.artifact();
        let mut material = Vec::new();
        let mut add = |name: &str, value: &[u8]| {
            material.extend_from_slice(&(name.len() as u64).to_le_bytes());
            material.extend_from_slice(name.as_bytes());
            material.extend_from_slice(&(value.len() as u64).to_le_bytes());
            material.extend_from_slice(value);
        };
        add("unbound_template", unbound_source.as_bytes());
        add("retained_native_handoff_and_linguistic_receipts", handoff);
        add("actual_feature_execution_receipt", feature_evidence);
        add("reviewed_layout_and_precision", &self.layout);
        add(
            "complete_model_signature",
            &self.model.signature().clone().encode().unwrap(),
        );
        add(
            "complete_model_resource_reference",
            &artifact.content.encode().unwrap(),
        );
        let descriptor = serde_json::json!({"architecture":artifact.architecture_profile,"format":artifact.format_profile,"precision":artifact.precision_profile,"state_schema_version":artifact.state_schema_version,"signature_identity":artifact.signature_identity,"model_descriptor_identity":self.model.descriptor_identity(),"raw_blob_sha256":self.raw_blob_sha256});
        add(
            "complete_artifact_descriptor",
            &serde_json::to_vec(&descriptor).unwrap(),
        );
        add(
            "full_model_read_grant",
            &serde_json::to_vec(&grant_value(&self.model_binding)).unwrap(),
        );
        for (name, resource) in &self.resources {
            add(
                name,
                &serde_json::to_vec(&grant_value(&resource.binding)).unwrap(),
            );
            add(
                "exact_resource_type",
                &resource.value_type.canonical_bytes().unwrap(),
            );
            let TensorBacking::Resource(reference) = &resource.resource.tensor().backing else {
                panic!("resource")
            };
            add("exact_tensor_reference", &reference.encode().unwrap());
        }
        add("scope",b"signal-epoch/source-pcm16; retained caller basis is not native or linguistic admission; joint-committed-session=false; caller-admitted-warm-seed; 16kHz/160sample epochs");
        material
    }
}

pub fn anchor_literal(model: &RetainedSignalModel, basis: [u8; 32]) -> String {
    let array = |bytes: [u8; 32]| {
        format!(
            "[{}]",
            bytes
                .iter()
                .map(u8::to_string)
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    format!("{{artifact_identity:{},model_descriptor_identity:{},session_basis_identity:{},precision:reference_float32(\"\")}}",array(model.model.artifact().content_identity()),array(model.model.descriptor_identity()),array(basis))
}
