extern crate alloc;
use conduit_ai::*;
use conduit_core::*;
use conduit_data::{TensorAxisRole, TensorElement};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn model_access(artifact: &ModelArtifact) -> conduit_core::ResourceReferenceBinding {
    use conduit_core::*;
    let reference = &artifact.content;
    ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        handle: ResourceHandleId::from("test/model-content"),
        authority_contract: AuthorityContractId::from(MODEL_READ_AUTHORITY),
        authority_grant: AuthorityGrantId::from("test/model-read-grant"),
        maximum_bytes: reference.extent.bytes,
        maximum_items: reference.extent.items,
        availability: ResourceReferenceAvailability::Available,
    }
}

fn reference(digest: [u8; 32], profile: &str, bytes: u64) -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest(digest),
        content_profile: KindId::from(profile),
        access_class: ResourceClassId::from("model-store/read@1"),
        extent: ResourceExtent { bytes, items: None },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([7; 32]),
            expires_at: None,
        },
    }
}

fn tensor() -> ModelTensorConstraint {
    tensor_with(
        TensorAxisRole::Time,
        ModelDimensionConstraint::bounded(256, 1).unwrap(),
    )
}

fn tensor_with(
    first_role: TensorAxisRole,
    first_dimension: ModelDimensionConstraint,
) -> ModelTensorConstraint {
    ModelTensorConstraint::from_parts(
        vec![TensorElement::F32],
        vec![
            ModelAxisConstraint {
                role: first_role,
                dimension: first_dimension,
            },
            ModelAxisConstraint {
                role: TensorAxisRole::Feature,
                dimension: ModelDimensionConstraint::fixed(12).unwrap(),
            },
        ],
        12_288,
    )
    .unwrap()
}

fn signature_fixture() -> ModelSignature {
    signature_with(tensor())
}

fn signature_with(input: ModelTensorConstraint) -> ModelSignature {
    ModelSignature::from_parts(
        "tongues/articulatory-encoder@1".into(),
        1,
        vec![ModelOperation::Encode, ModelOperation::Evaluate],
        vec![
            ModelPortConstraint::from_parts(
                "trajectory".into(),
                "data/sampled-signal@1".into(),
                ModelPortPresence::Required,
                ModelValueConstraint::sampled_signal(input).unwrap(),
            )
            .unwrap(),
        ],
        vec![
            ModelPortConstraint::from_parts(
                "latent".into(),
                "data/tensor@1".into(),
                ModelPortPresence::Required,
                ModelValueConstraint::tensor(tensor()).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn artifact_fixture(signature: &ModelSignature) -> ModelArtifact {
    let bytes = b"finite non-llm articulatory encoder";
    ModelArtifact {
        architecture_profile: "tongues/linear-articulatory-encoder@1".into(),
        format_profile: "model/artifact/reference-matrix@1".into(),
        precision_profile: "number/ieee754-f32-le".into(),
        state_schema_version: 1,
        signature_identity: signature.semantic_digest().unwrap(),
        content: reference(
            model_content_digest(bytes),
            "model/artifact/reference-matrix@1",
            bytes.len() as u64,
        ),
    }
}

fn limits() -> ModelComputeLimits {
    ModelComputeLimits {
        maximum_model_bytes: 4096,
        maximum_working_memory_bytes: 8192,
        maximum_device_memory_bytes: 0,
        maximum_input_bytes: 256,
        maximum_output_bytes: 256,
        maximum_batch_items: 8,
        maximum_rank: 4,
        maximum_in_flight: 1,
        maximum_queue_items: 2,
        maximum_queue_bytes: 512,
        cancellation_supported: true,
        compute: ComputeCapacity {
            class: PortableComputeClass::GeneralCpu,
            minimum_lanes: 1,
            preferred_lanes: 2,
            maximum_lanes: 4,
            service: ComputeServiceGuarantee::Shared,
        },
    }
}

fn offer() -> ModelComputeOffer {
    ModelComputeOffer {
        identity: "std/reference-model-compute".into(),
        supported_operations: vec![
            ModelComputeOperation::Inference,
            ModelComputeOperation::TrainStep,
            ModelComputeOperation::Evaluate,
            ModelComputeOperation::Checkpoint,
            ModelComputeOperation::IntegrateDynamics,
            ModelComputeOperation::RelationQuery,
        ],
        accepted_formats: vec!["model/reference-linear".into()],
        supported_elements: vec![TensorElement::F32],
        solver_profiles: vec!["fixed-step/reference".into()],
        determinism_profiles: vec!["deterministic/f32".into()],
        checkpoint_loading: true,
        checkpoint_writing: true,
        limits: limits(),
        cache_policy: ModelCachePolicy::bounded(4096, 1).unwrap(),
    }
}

fn requirement(operation: ModelComputeOperation) -> ModelComputeRequirement {
    ModelComputeRequirement {
        operation,
        model_format: "model/reference-linear".into(),
        element: TensorElement::F32,
        rank: 1,
        model_bytes: 128,
        working_memory_bytes: 512,
        device_memory_bytes: 0,
        input_bytes: 8,
        output_bytes: 8,
        batch_items: 1,
        compute_class: PortableComputeClass::GeneralCpu,
        minimum_lanes: 1,
        preferred_lanes: 2,
        maximum_lanes: 4,
        minimum_service: ComputeServiceGuarantee::Shared,
        solver_profile: None,
        determinism_profile: "deterministic/f32".into(),
        requires_checkpoint_load: false,
        requires_checkpoint_write: false,
    }
}

fn runtime() -> ModelComputeRuntimeIdentity {
    ModelComputeRuntimeIdentity {
        provider_name: "conduit-reference".into(),
        runtime_name: "native-rust".into(),
        runtime_version: "1".into(),
        runtime_build_identity: "build/reference/1".into(),
        adapter_artifact_identity: "adapter/reference/1".into(),
        device_evidence: "host/cpu-observed".into(),
        precision_profile: "f32".into(),
    }
}

#[derive(Default)]
struct Calls {
    prep: AtomicUsize,
    load: AtomicUsize,
    cancel: AtomicUsize,
    lost: AtomicUsize,
    unload: AtomicUsize,
    abandon: AtomicUsize,
    batches: AtomicUsize,
}
struct Driver {
    model: Arc<AdmittedModelResource>,
    foreign_model: Arc<AdmittedModelResource>,
    source: Arc<str>,
    foreign_source: Arc<str>,
    plan: Arc<Plan>,
    foreign_plan: Arc<Plan>,
    runtime: ModelComputeRuntimeIdentity,
    offer: ModelComputeOffer,
    swap: Arc<AtomicUsize>,
    calls: Arc<Calls>,
    preparation_limit: usize,
    preparation_required: usize,
}
impl ModelComputeRuntimeDriver for Driver {
    type Error = &'static str;
    type Batch = Vec<u8>;
    type PreparationReceipt = usize;
    type ResourceReceipt = ();
    type WarmReceipt = ();
    type Completion = ();
    type StopReceipt = ();
    fn model(&self) -> &Arc<AdmittedModelResource> {
        if self.swap.load(Ordering::SeqCst) == 1 {
            &self.foreign_model
        } else {
            &self.model
        }
    }
    fn source(&self) -> &Arc<str> {
        if self.swap.load(Ordering::SeqCst) == 2 {
            &self.foreign_source
        } else {
            &self.source
        }
    }
    fn plan(&self) -> &Arc<Plan> {
        if self.swap.load(Ordering::SeqCst) == 3 {
            &self.foreign_plan
        } else {
            &self.plan
        }
    }
    fn runtime(&self) -> &ModelComputeRuntimeIdentity {
        &self.runtime
    }
    fn offer(&self) -> &ModelComputeOffer {
        &self.offer
    }
    fn admit_preparation(&self, _: &ModelComputeRequirement) -> Result<usize, Self::Error> {
        self.calls.prep.fetch_add(1, Ordering::SeqCst);
        if self.preparation_limit < self.preparation_required {
            Err("preparation capacity")
        } else {
            Ok(self.preparation_required)
        }
    }
    fn validate_preparation(
        &self,
        r: &usize,
        _: &ModelComputeRequirement,
    ) -> Result<(), Self::Error> {
        if *r == self.preparation_required {
            Ok(())
        } else {
            Err("foreign preparation")
        }
    }
    fn admit_resources(&self, _: &ModelComputeRequirement) -> Result<(), Self::Error> {
        self.calls.load.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn validate_resources(&self, _: &(), _: &ModelComputeRequirement) -> Result<(), Self::Error> {
        Ok(())
    }
    fn abandon_original(
        &mut self,
        m: &Arc<AdmittedModelResource>,
        s: &Arc<str>,
        p: &Arc<Plan>,
    ) -> Result<(), Self::Error> {
        assert!(Arc::ptr_eq(m, &self.model));
        assert!(Arc::ptr_eq(s, &self.source));
        assert!(Arc::ptr_eq(p, &self.plan));
        self.calls.abandon.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn begin_warm(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn poll_warm(&mut self) -> Result<ModelComputeDriverProgress<()>, Self::Error> {
        Ok(ModelComputeDriverProgress::Complete(()))
    }
    fn validate_warm(&self, _: &()) -> Result<(), Self::Error> {
        Ok(())
    }
    fn admit_batch(&self, b: &Vec<u8>) -> Result<(u64, u32), Self::Error> {
        Ok((b.len() as u64, 1))
    }
    fn begin_batch(&mut self, _: Vec<u8>) -> Result<(), Self::Error> {
        self.calls.batches.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn poll(&mut self) -> Result<ModelComputeDriverProgress<()>, Self::Error> {
        Ok(ModelComputeDriverProgress::Pending)
    }
    fn validate_completion(&self, _: &()) -> Result<u64, Self::Error> {
        Ok(1)
    }
    fn cancel(&mut self) -> Result<(), Self::Error> {
        self.calls.cancel.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn provider_lost(&mut self) -> Result<(), Self::Error> {
        self.calls.lost.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn unload(&mut self) -> Result<(), Self::Error> {
        self.calls.unload.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
fn fixture() -> (
    Driver,
    ModelComputeRequirement,
    Arc<AtomicUsize>,
    Arc<Calls>,
) {
    static BASIS: std::sync::OnceLock<(Arc<str>, Arc<Plan>, Arc<Plan>)> =
        std::sync::OnceLock::new();
    let (source, plan, foreign_plan) = BASIS.get_or_init(|| {
        let source = Arc::from(
            std::fs::read_to_string(fixture_path("checked-epoch-source.conduit")).unwrap(),
        );
        let plan: Plan =
            serde_json::from_slice(&std::fs::read(fixture_path("sealed-epoch-plan.json")).unwrap())
                .unwrap();
        let foreign_plan = Arc::new(plan.clone());
        (source, Arc::new(plan), foreign_plan)
    });
    let signature = signature_fixture();
    let artifact = artifact_fixture(&signature);
    let binding = model_access(&artifact);
    let bytes: Arc<[u8]> = Arc::from(&b"finite non-llm articulatory encoder"[..]);
    let model = Arc::new(
        AdmittedModelResource::adopt(
            artifact.clone(),
            signature.clone(),
            Arc::clone(&bytes),
            &binding,
        )
        .unwrap(),
    );
    let foreign_model = Arc::new(
        AdmittedModelResource::adopt(artifact.clone(), signature, bytes, &binding).unwrap(),
    );
    let mut runtime = runtime();
    runtime.precision_profile = artifact.precision_profile.clone();
    let mut offer = offer();
    offer.accepted_formats = vec![artifact.format_profile.clone()];
    let mut requirement = requirement(ModelComputeOperation::Inference);
    requirement.model_format = artifact.format_profile;
    requirement.model_bytes = model.bytes().len() as u64;
    let swap = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(Calls::default());
    let driver = Driver {
        model,
        foreign_model,
        source: Arc::clone(source),
        foreign_source: Arc::from(source.as_ref()),
        plan: Arc::clone(plan),
        foreign_plan: Arc::clone(foreign_plan),
        runtime,
        offer,
        swap: Arc::clone(&swap),
        calls: Arc::clone(&calls),
        preparation_limit: 4096,
        preparation_required: 4096,
    };
    (driver, requirement, swap, calls)
}
fn prepare(
    d: Driver,
    r: ModelComputeRequirement,
) -> Result<OwnedModelComputeSession<Driver>, OwnedModelComputeRefusal<&'static str>> {
    OwnedModelComputeSession::prepare(
        Arc::clone(&d.model),
        d.offer.clone(),
        r,
        d.runtime.clone(),
        d,
    )
}
fn ready(s: &mut OwnedModelComputeSession<Driver>) {
    s.load().unwrap();
    s.begin_warm().unwrap();
    assert!(matches!(
        s.poll_warm().unwrap(),
        ModelComputeDriverProgress::Complete(())
    ));
}
#[test]
#[ignore = "requires archived original FARGAN Source and sealed Plan fixture"]
fn preparation_one_under_refuses_before_source_plan_verification_or_load() {
    let (mut d, r, _, calls) = fixture();
    d.preparation_limit = 4095;
    // A deliberately wrong Source seal would fail later: preparation must win.
    d.source = Arc::from("foreign unsealed Source");
    assert!(matches!(
        prepare(d, r),
        Err(OwnedModelComputeRefusal::Driver("preparation capacity"))
    ));
    assert_eq!(calls.prep.load(Ordering::SeqCst), 1);
    assert_eq!(calls.load.load(Ordering::SeqCst), 0);
    assert_eq!(calls.abandon.load(Ordering::SeqCst), 0);
}
#[test]
#[ignore = "requires archived original FARGAN Source and sealed Plan fixture"]
fn foreign_equal_material_owners_stop_without_foreign_actions() {
    for owner in 1..=3 {
        for action in 0..3 {
            let (d, r, swap, calls) = fixture();
            let mut s = prepare(d, r).unwrap();
            assert_eq!(*s.preparation(), 4096);
            ready(&mut s);
            if action != 2 {
                s.enqueue(vec![7]).unwrap();
                s.begin().unwrap();
            }
            swap.store(owner, Ordering::SeqCst);
            let result = match action {
                0 => s.cancel(),
                1 => s.provider_lost(),
                _ => s.unload(),
            };
            assert!(matches!(
                result,
                Err(OwnedModelComputeRefusal::ForeignModel
                    | OwnedModelComputeRefusal::ForeignSourcePlan)
            ));
            assert_eq!(s.state(), ModelComputeLifecycle::Lost);
            assert!(s.loaded_model().is_none());
            assert_eq!(calls.abandon.load(Ordering::SeqCst), 1);
            assert_eq!(calls.cancel.load(Ordering::SeqCst), 0);
            assert_eq!(calls.lost.load(Ordering::SeqCst), 0);
            assert_eq!(calls.unload.load(Ordering::SeqCst), 0);
            assert_eq!(
                calls.batches.load(Ordering::SeqCst),
                usize::from(action != 2)
            );
        }
    }
}
#[test]
#[ignore = "requires archived original FARGAN Source and sealed Plan fixture"]
fn unchanged_basis_stop_unload_use_normal_driver_actions() {
    for action in 0..3 {
        let (d, r, _, calls) = fixture();
        let mut s = prepare(d, r).unwrap();
        ready(&mut s);
        if action != 2 {
            s.enqueue(vec![1]).unwrap();
            s.begin().unwrap();
        }
        match action {
            0 => s.cancel().unwrap(),
            1 => s.provider_lost().unwrap(),
            _ => s.unload().unwrap(),
        };
        assert_eq!(calls.abandon.load(Ordering::SeqCst), 0);
        assert_eq!(
            calls.cancel.load(Ordering::SeqCst),
            usize::from(action == 0)
        );
        assert_eq!(calls.lost.load(Ordering::SeqCst), usize::from(action == 1));
        assert_eq!(
            calls.unload.load(Ordering::SeqCst),
            usize::from(action == 2)
        );
    }
}

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(
        std::env::var_os("CONDUIT_FARGAN_SESSION_PLAN_FIXTURE")
            .expect("original FARGAN Source/Plan fixture required"),
    )
    .join(name)
}
