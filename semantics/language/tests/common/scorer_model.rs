#![allow(dead_code)]
use conduit_ai::integer_categorical::*;
use conduit_ai::*;
use conduit_core::*;
use conduit_data::*;
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence};
use conduit_std_host::{hosted_integer_categorical::HostedIntegerCategorical, hosted_model::*};
pub const BYTES: &[u8] = include_bytes!("../../training/ewt_four_token/ewt_four_token.i16");
pub struct Store(pub &'static [u8]);
impl ModelArtifactStore for Store {
    fn load(&self, reference: &BoundedResourceRef) -> Result<Vec<u8>, HostedModelRefusal> {
        if reference.identity.digest() != model_content_digest(self.0) {
            return Err(HostedModelRefusal::ResourceUnavailable);
        }
        Ok(self.0.to_vec())
    }
}
pub struct Scorer {
    pub artifact: ModelArtifact,
    pub signature: ModelSignature,
    pub adapter: HostedIntegerCategorical,
    bytes: &'static [u8],
    lookups: u64,
}
impl Scorer {
    pub fn new() -> Self {
        Self::prepare(
            BYTES,
            include_str!("../../training/ewt_four_token/manifest.json"),
            "gold-upos",
        )
    }
    pub fn prepare(bytes: &'static [u8], manifest: &str, profile: &str) -> Self {
        let encoding = semantic_digest(
            "language/parser-scorer-encoding@1",
            include_bytes!("../../parser_scorer.conduit"),
        );
        Self::prepare_encoding(bytes, manifest, profile, encoding)
    }
    pub fn prepare_v2(bytes: &'static [u8], manifest: &str, profile: &str) -> Self {
        let encoding = semantic_digest(
            "language/parser-v2-scorer-encoding@1",
            include_bytes!("../../parser_scorer_v2.conduit"),
        );
        Self::prepare_encoding(bytes, manifest, profile, encoding)
    }
    fn prepare_encoding(
        bytes: &'static [u8],
        manifest: &str,
        profile: &str,
        encoding: [u8; 32],
    ) -> Self {
        let encoding = encoding
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let manifest: serde_json::Value = serde_json::from_str(manifest).unwrap();
        assert_eq!(manifest["feature_class_contract_identity"], encoding);
        let lookups = manifest["lookups"].as_u64().unwrap();
        assert!((1..=64).contains(&lookups));
        let input_bytes = lookups * 8;
        let port = |name: &str, element, count| {
            ModelPortConstraint::new(
                ModelPortIdentity::new(name.into()).unwrap(),
                ModelPortPresence::Required,
                ModelSemanticKind::new(format!("language/parser-{name}/{encoding}@1")).unwrap(),
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
        let identity = if profile == "gold-upos" {
            format!("language/parser-ewt-four-token-gold-upos/{encoding}@1")
        } else {
            let contract = semantic_digest(
                "language/parser-model-contract@1",
                format!("{encoding}/{profile}").as_bytes(),
            );
            let contract = contract
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            format!("language/parser-joint/{contract}@1")
        };
        let signature = ModelSignature::from_parts(
            identity,
            1,
            vec![ModelOperation::Infer],
            vec![port("features", TensorElement::U64, lookups)],
            vec![port("scores", TensorElement::I64, 76)],
        )
        .unwrap();
        let digest = model_content_digest(bytes);
        let artifact = ModelArtifact {
            architecture_profile: CATEGORICAL_I16_ARCHITECTURE.into(),
            format_profile: CATEGORICAL_I16_FORMAT.into(),
            precision_profile: CATEGORICAL_I16_PRECISION.into(),
            state_schema_version: 1,
            signature_identity: signature.semantic_digest().unwrap(),
            content: BoundedResourceRef {
                identity: ResourceSemanticIdentity::from_digest(digest),
                content_profile: CATEGORICAL_I16_FORMAT.into(),
                access_class: "model/pinned-training-artifact/read@1".into(),
                extent: ResourceExtent {
                    bytes: bytes.len() as u64,
                    items: None,
                },
                lifetime: ResourceLifetime {
                    version: ResourceVersionIdentity::from_digest(digest),
                    expires_at: None,
                },
            },
        };
        let requirement = ModelComputeRequirement {
            operation: ModelComputeOperation::Inference,
            model_format: CATEGORICAL_I16_FORMAT.into(),
            element: TensorElement::U64,
            rank: 1,
            model_bytes: bytes.len() as u64,
            working_memory_bytes: 1024 * 1024,
            device_memory_bytes: 0,
            input_bytes,
            output_bytes: 608,
            batch_items: 1,
            compute_class: PortableComputeClass::GeneralCpu,
            minimum_lanes: 1,
            preferred_lanes: 1,
            maximum_lanes: 1,
            minimum_service: ComputeServiceGuarantee::Shared,
            solver_profile: None,
            determinism_profile: "deterministic/i16-i64@1".into(),
            requires_checkpoint_load: false,
            requires_checkpoint_write: false,
        };
        let offer = ModelComputeOffer {
            identity: "hosted/categorical-i16@1".into(),
            supported_operations: vec![ModelComputeOperation::Inference],
            accepted_formats: vec![CATEGORICAL_I16_FORMAT.into()],
            supported_elements: vec![TensorElement::U64, TensorElement::I16, TensorElement::I64],
            solver_profiles: vec![],
            determinism_profiles: vec!["deterministic/i16-i64@1".into()],
            checkpoint_loading: false,
            checkpoint_writing: false,
            limits: ModelComputeLimits {
                maximum_model_bytes: 65536,
                maximum_working_memory_bytes: 1024 * 1024,
                maximum_device_memory_bytes: 0,
                maximum_input_bytes: input_bytes,
                maximum_output_bytes: 608,
                maximum_batch_items: 1,
                maximum_rank: 1,
                maximum_in_flight: 1,
                maximum_queue_items: 1,
                maximum_queue_bytes: input_bytes,
                cancellation_supported: true,
                compute: ComputeCapacity {
                    class: PortableComputeClass::GeneralCpu,
                    minimum_lanes: 1,
                    preferred_lanes: 1,
                    maximum_lanes: 1,
                    service: ComputeServiceGuarantee::Shared,
                },
            },
            cache_policy: ModelCachePolicy::bounded(65536, 1).unwrap(),
        };
        let runtime_source = [
            include_bytes!("../../../ai/src/integer_categorical.rs").as_slice(),
            include_bytes!("../../../../targets/std/src/hosted_integer_categorical.rs").as_slice(),
        ]
        .concat();
        let runtime_digest =
            semantic_digest("ai/integer-categorical-runtime-source@1", &runtime_source)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
        let runtime = ModelComputeRuntimeIdentity {
            provider_name: "conduit-std".into(),
            runtime_name: "categorical-i16-sum".into(),
            runtime_version: "1".into(),
            runtime_build_identity: runtime_digest.clone(),
            adapter_artifact_identity: "std/hosted-integer-categorical@1".into(),
            device_evidence: "observed/host-cpu".into(),
            precision_profile: CATEGORICAL_I16_PRECISION.into(),
        };
        let realization = ModelRuntimeRealization {
            implementation_identity: "std/hosted-integer-categorical@1".into(),
            runtime_name: "categorical-i16-sum".into(),
            runtime_version: "1".into(),
            runtime_build_identity: runtime_digest.clone(),
            device_profile: "host-cpu".into(),
            supported_formats: vec![CATEGORICAL_I16_FORMAT.into()],
            supported_precisions: vec![CATEGORICAL_I16_PRECISION.into()],
            loaded_artifact_identity: digest,
            loaded_checkpoint_identity: None,
        };
        let adapter = HostedIntegerCategorical::prepare(
            &artifact,
            &signature,
            bytes,
            offer,
            runtime,
            requirement,
            realization,
        )
        .unwrap();
        Self {
            artifact,
            signature,
            adapter,
            bytes,
            lookups,
        }
    }
    pub fn score(
        &mut self,
        indices: &[u64],
        basis_identity: [u8; 32],
    ) -> (Vec<i64>, ModelInvocationEvidence) {
        assert_eq!(indices.len() as u64, self.lookups);
        let bytes = indices
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<_>>();
        let input = TensorValue {
            element: TensorElement::U64,
            dimensions: BoundedSequence::try_from_iter([self.lookups]).unwrap(),
            axes: BoundedSequence::try_from_iter([TensorAxis {
                role: TensorAxisRole::Feature,
                identity: Some("language/parser-indices-v1".into()),
                unit: None,
            }])
            .unwrap(),
            content_digest: tensor_content_digest(&bytes),
            backing: TensorBacking::Inline(BoundedBytes::new(&bytes).unwrap()),
        };
        let units = self.adapter.work_units();
        let result = invoke_hosted_model(
            &Store(self.bytes),
            &mut self.adapter,
            &self.artifact,
            None,
            &self.signature,
            ModelOperation::Infer,
            basis_identity,
            &input.encode().unwrap(),
            units,
            4096,
        )
        .unwrap();
        let tensor = TensorValue::decode(&result.output).unwrap();
        let TensorBacking::Inline(bytes) = tensor.backing else {
            panic!("inline scores")
        };
        (
            bytes
                .as_slice()
                .as_chunks::<8>()
                .0
                .iter()
                .map(|b| i64::from_le_bytes(*b))
                .collect(),
            result.evidence,
        )
    }
}
