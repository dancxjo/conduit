//! Scoped hosted session around the actual immutable model, Source/Plan and
//! scheduler. Known component quotas do not constitute whole-working admission.
use super::*;
use conduit_ai::*;
use conduit_data::TensorElement;
use conduit_plot::CheckedSyntaxDocument;
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[derive(Debug)]
enum Refusal {
    Lifecycle,
    ForeignBasis,
    Queue,
    IncompleteWorkingInventory,
    Execution,
}
struct Session<'a> {
    model: &'a super::super::custody::RetainedSignalModel,
    retained_model: Option<Arc<AdmittedModelResource>>,
    descriptor: Arc<AdmittedModelResource>,
    warm_seeds: Vec<StructuredInfoValue>,
    warm_proposal: Option<Vec<u8>>,
    lifecycle: ModelComputeSession,
    requirement: ModelComputeRequirement,
    source: String,
    plan: Plan,
    service: super::super::service_profile::PreparedServiceProfile,
    warm: Option<native_startup::WarmRun>,
    queue: Option<Arc<BTreeMap<String, Vec<Vec<u8>>>>>,
    last_batch: Option<Arc<BTreeMap<String, Vec<Vec<u8>>>>>,
    queued_bytes: u64,
    stopped: bool,
    outcomes: Vec<&'static str>,
}
impl<'a> Session<'a> {
    fn discover(
        model: &'a super::super::custody::RetainedSignalModel,
        plan: &Plan,
        context: &super::super::EpochProfiles,
        runtime: ModelComputeRuntimeIdentity,
    ) -> Result<Self, Refusal> {
        let checked = context
            .checked_source
            .as_ref()
            .ok_or(Refusal::ForeignBasis)?;
        super::super::plan_artifact::plan_image(plan, checked)
            .map_err(|_| Refusal::ForeignBasis)?;
        let descriptor = model.compound_descriptor();
        let bytes = descriptor.bytes().len() as u64;
        // This explicitly named offer bounds only the already selected ValueStorage
        // component. ASTs, scheduler arrays, Host owners, preparation and allocator
        // overhead remain uninventoried, so require_full_working_admission refuses.
        let requirement = ModelComputeRequirement {
            operation: ModelComputeOperation::Inference,
            model_format: model.model.artifact().format_profile.clone(),
            element: TensorElement::F32,
            rank: 1,
            model_bytes: bytes,
            working_memory_bytes: 16 * 1024 * 1024,
            device_memory_bytes: 0,
            input_bytes: 256 * 1024,
            output_bytes: 256 * 1024,
            batch_items: 16,
            compute_class: PortableComputeClass::GeneralCpu,
            minimum_lanes: 1,
            preferred_lanes: 1,
            maximum_lanes: 1,
            minimum_service: ComputeServiceGuarantee::Shared,
            solver_profile: None,
            determinism_profile: "source-f32-libm-host-observed@1".into(),
            requires_checkpoint_load: false,
            requires_checkpoint_write: false,
        };
        let offer = ModelComputeOffer {
            identity: "fargan/hosted-value-storage-component-only@1".into(),
            supported_operations: vec![ModelComputeOperation::Inference],
            accepted_formats: vec![requirement.model_format.clone()],
            supported_elements: vec![TensorElement::F32],
            solver_profiles: vec![],
            determinism_profiles: vec![requirement.determinism_profile.clone()],
            checkpoint_loading: false,
            checkpoint_writing: false,
            limits: ModelComputeLimits {
                maximum_model_bytes: bytes,
                maximum_working_memory_bytes: requirement.working_memory_bytes,
                maximum_device_memory_bytes: 0,
                maximum_input_bytes: requirement.input_bytes,
                maximum_output_bytes: requirement.output_bytes,
                maximum_batch_items: 16,
                maximum_rank: 1,
                maximum_in_flight: 1,
                maximum_queue_items: 1,
                maximum_queue_bytes: requirement.input_bytes,
                cancellation_supported: true,
                compute: ComputeCapacity {
                    class: PortableComputeClass::GeneralCpu,
                    minimum_lanes: 1,
                    preferred_lanes: 1,
                    maximum_lanes: 1,
                    service: ComputeServiceGuarantee::Shared,
                },
            },
            cache_policy: ModelCachePolicy::NoCache,
        };
        offer.admits(&requirement).map_err(|_| Refusal::Lifecycle)?;
        let mut lifecycle =
            ModelComputeSession::discovered(offer, runtime).map_err(|_| Refusal::Lifecycle)?;
        // Load means adopt this already Source/resource-admitted immutable owner;
        // it does not claim fresh filesystem/device upload or cache timing.
        lifecycle
            .begin_load(model.model.artifact().content_identity(), bytes)
            .map_err(|_| Refusal::Lifecycle)?;
        Ok(Self {
            model,
            retained_model: Some(Arc::clone(&descriptor)),
            descriptor,
            warm_seeds: Vec::new(),
            warm_proposal: None,
            lifecycle,
            requirement,
            source: checked.text.clone(),
            plan: plan.clone(),
            service: super::super::service_profile::PreparedServiceProfile::production(),
            warm: None,
            queue: None,
            last_batch: None,
            queued_bytes: 0,
            stopped: false,
            outcomes: vec!["adopt-loaded-original-model"],
        })
    }
    fn require_full_working_admission(&self) -> Result<(), Refusal> {
        Err(Refusal::IncompleteWorkingInventory)
    }
    fn check_basis(
        &self,
        model: &super::super::custody::RetainedSignalModel,
        plan: &Plan,
    ) -> Result<(), Refusal> {
        if !core::ptr::eq(self.model, model)
            || self.retained_model.as_ref().is_none_or(|owned| {
                !Arc::ptr_eq(&owned.shared_storage(), &model.model.shared_storage())
            })
            || self.plan != *plan
            || self.lifecycle.loaded_model_identity()
                != Some(self.descriptor.artifact().content_identity())
        {
            return Err(Refusal::ForeignBasis);
        }
        Ok(())
    }
    fn warm(&mut self, proposal: &StructuredInfoValue) -> Result<(), Refusal> {
        if self.stopped {
            return Err(Refusal::Lifecycle);
        }
        self.lifecycle
            .begin_warming()
            .map_err(|_| Refusal::Lifecycle)?;
        let warm =
            native_startup::run_warm_profile(self.model, proposal, ExecutionMode::LifecycleWarm);
        if !(warm.result.drained || warm.result.warm_retired) || warm.result.values.len() != 3 {
            return Err(Refusal::Execution);
        }
        self.warm_proposal = Some(
            proposal
                .canonical_bytes()
                .map_err(|_| Refusal::ForeignBasis)?,
        );
        self.warm_seeds = run_native_startup_feedback(&warm.result.values, proposal);
        self.warm = Some(warm);
        self.lifecycle.ready().map_err(|_| Refusal::Lifecycle)?;
        self.outcomes
            .push("actual-source-warm-products-idle-explicit-retirement");
        Ok(())
    }
    fn enqueue(
        &mut self,
        checked: &CheckedSyntaxDocument,
        native_names: &BTreeMap<String, String>,
        inputs: BTreeMap<String, Vec<Vec<u8>>>,
    ) -> Result<(), Refusal> {
        if self.stopped || self.queue.is_some() || self.warm.is_none() {
            return Err(Refusal::Queue);
        }
        for name in ["signal_seed", "conditioning_seed"] {
            let frames = inputs.get(name).ok_or(Refusal::ForeignBasis)?;
            if frames.len() != 1
                || !self
                    .warm_seeds
                    .iter()
                    .any(|seed| seed.canonical_bytes().ok().as_ref() == frames.first())
            {
                return Err(Refusal::ForeignBasis);
            }
        }
        if inputs.len() != 3
            || native_names
                != &BTreeMap::from([
                    ("signal_seed".into(), "FarganSignalEpochFeedback".into()),
                    (
                        "conditioning_seed".into(),
                        "FarganConditioningEpochFeedback".into(),
                    ),
                    ("events".into(), "FarganFeatureConditionEpoch".into()),
                ])
        {
            return Err(Refusal::ForeignBasis);
        }
        let mut bytes = 0u64;
        let mut count = 0u32;
        for (name, frames) in &inputs {
            let native = native_names.get(name).ok_or(Refusal::ForeignBasis)?;
            for frame in frames {
                super::super::interface::admit_retained_session_native(checked, native, frame)
                    .map_err(|_| Refusal::ForeignBasis)?;
                bytes = bytes
                    .checked_add(frame.len() as u64)
                    .ok_or(Refusal::Queue)?;
                count = count.checked_add(1).ok_or(Refusal::Queue)?;
            }
        }
        if bytes == 0
            || bytes > self.requirement.input_bytes
            || count > self.requirement.batch_items
        {
            return Err(Refusal::Queue);
        }
        self.lifecycle.enqueue(bytes).map_err(|_| Refusal::Queue)?;
        self.queued_bytes = bytes;
        let original = Arc::new(inputs);
        self.queue = Some(Arc::clone(&original));
        self.last_batch = Some(original);
        self.outcomes.push("exact-native-owned-batch-enqueued");
        Ok(())
    }
    fn execute(
        &mut self,
        context: &super::super::EpochProfiles,
        seeded: &conduitos::seeded_state::SeededStateOperationFactory,
        expected: usize,
        mode: ExecutionMode,
    ) -> Result<Option<StreamResultAndTiming>, Refusal> {
        if self.stopped {
            return Err(Refusal::Lifecycle);
        }
        self.check_basis(self.model, &self.plan)?;
        if context
            .checked_source
            .as_ref()
            .is_none_or(|checked| checked.text != self.source)
            || self
                .queue
                .as_ref()
                .and_then(|ports| ports.get("events"))
                .map(Vec::len)
                != Some(expected)
        {
            return Err(Refusal::ForeignBasis);
        }
        let inputs = self.queue.take().ok_or(Refusal::Queue)?;
        self.lifecycle
            .begin(&self.requirement, self.queued_bytes)
            .map_err(|_| Refusal::Lifecycle)?;
        self.queued_bytes = 0;
        let result = run_epoch_stream_plan_with_trace(
            self.plan.clone(),
            context,
            &self.model.resources,
            (*inputs).clone(),
            Some(seeded),
            StreamRun {
                expected,
                mode,
                trace: None,
                service: ServiceBudget::direct(&self.service),
            },
        );
        match (&result, mode) {
            (Some(result), ExecutionMode::LifecycleNormal)
                if result.drained && result.values.len() == expected =>
            {
                self.lifecycle.finish().map_err(|_| Refusal::Lifecycle)?;
                self.outcomes.push("actual-scheduler-drained-finish");
            }
            (None, ExecutionMode::LifecycleCancel) => {
                self.lifecycle.cancel().map_err(|_| Refusal::Lifecycle)?;
                self.stopped = true;
                self.outcomes.push("actual-host-call-cancel-stop");
            }
            (None, ExecutionMode::LifecyclePressure) => {
                self.stopped = true;
                self.outcomes.push("actual-storage-pressure-stop-no-finish");
            }
            (None, ExecutionMode::LifecycleProviderLost) => {
                self.provider_lost();
            }
            _ => {
                self.stopped = true;
                self.outcomes
                    .push("actual-execution-refusal-without-provider-loss-claim");
                return Err(Refusal::Execution);
            }
        }
        Ok(result)
    }
    fn provider_lost(&mut self) {
        self.lifecycle.provider_lost();
        self.retained_model = None;
        self.queue = None;
        self.queued_bytes = 0;
        self.stopped = true;
        self.outcomes.push("provider-lost-stop");
    }
    fn unload(&mut self) -> Result<(), Refusal> {
        if self.queue.is_some() {
            return Err(Refusal::Queue);
        }
        self.lifecycle
            .begin_unload()
            .map_err(|_| Refusal::Lifecycle)?;
        self.lifecycle.shutdown().map_err(|_| Refusal::Lifecycle)?;
        self.retained_model = None;
        self.stopped = true;
        self.outcomes
            .push("compute-owner-released-original-audit-owner-retained-shutdown");
        Ok(())
    }
    fn material(&self) -> serde_json::Value {
        serde_json::json!({"loaded_compute_owner":self.retained_model.is_some(),"outcomes":self.outcomes,"state":format!("{:?}",self.lifecycle.state()),"stopped":self.stopped,"queued_bytes":self.queued_bytes,"original_batch":self.last_batch.as_deref()})
    }
    fn basis_material(&self) -> serde_json::Value {
        use conduit_plot::rust_binding::NativeRustBinding;
        serde_json::json!({"scope":"hosted ModelCompute lifecycle around actual Source/Plan/model; known ValueStorage component quota only","full_working_memory_admitted":false,"unknown_working_components":["Source AST/evaluator scratch","scheduler arrays and sign storage","Host owners/pending buffers","preparation/Native receipts","allocator overhead/stack"],"model_descriptor":format!("{:?}",self.descriptor.artifact()),"model_descriptor_identity":self.descriptor.descriptor_identity(),"model_content_identity":self.descriptor.artifact().content_identity(),"model_content_bytes":self.descriptor.bytes().len(),"full_compound_signature":self.descriptor.signature().clone().encode().unwrap(),"read_access":format!("{:?}",self.descriptor.access()),"resource_binding":format!("{:?}",self.model.model_binding),"runtime":format!("{:?}",self.lifecycle.runtime()),"source":self.source,"sealed_plan":self.plan,"finite_source_service":self.service.material(),"warm_plan":self.warm.as_ref().map(|w|&w.plan),"warm_source":self.warm.as_ref().map(|w|&w.source),"warm_proposal":self.warm_proposal,"warm_outputs":self.warm.as_ref().map(|w|w.result.values.iter().map(|v|v.canonical_bytes().unwrap()).collect::<Vec<_>>()),"warm_seeds":self.warm_seeds.iter().map(|v|v.canonical_bytes().unwrap()).collect::<Vec<_>>(),"model_raw_sha256":format!("{:x}",Sha256::digest(self.descriptor.bytes()))})
    }
}

#[test]
#[ignore = "requires actual pinned retained model; Source warm and two compound feedback epochs"]
fn actual_model_compute_session_owns_source_warm_queue_and_finish() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(run_actual_model_compute_session)
        .unwrap()
        .join()
        .unwrap();
}
fn run_actual_model_compute_session() {
    let root = std::path::PathBuf::from(std::env::var("CONDUIT_FARGAN_MODEL_FIXTURE").unwrap());
    let mut model = super::super::custody::RetainedSignalModel::load(&root);
    let (_, conditioning) = model.conditioning_resources();
    for (name, resource) in conditioning {
        assert!(
            model
                .resources
                .insert(format!("conditioning_{name}"), resource)
                .is_none()
        );
    }
    let (context, seeded, _) = super::super::prepared_signal_cycle_profiles_with_capacity(true);
    let (context, seeded, _) = super::super::conditioning_cycle::prepare_with(context, seeded);
    let source = super::super::conditioning_cycle::compound_source();
    let basis = model.basis_material(
        &source,
        b"actual ModelCompute component lifecycle; not whole working-memory admission",
        b"two exact Source feedback epochs",
    );
    let anchor = super::super::custody::anchor_literal(&model, Sha256::digest(&basis).into());
    let source = source.replace(
        "selected: FarganModelFrameAnchor\n",
        &format!("selected: FarganModelFrameAnchor = {anchor}\n"),
    );
    let (plan, context) = super::super::prepare_authored_epoch_entry(
        context,
        source,
        "speech/flow-fargan-compound-cycle",
        true,
        seeded.offers().cloned().collect(),
    )
    .unwrap();
    let executable = std::fs::read(std::env::current_exe().unwrap()).unwrap();
    let runtime = ModelComputeRuntimeIdentity {
        provider_name: "hosted-conduit-source-reference".into(),
        runtime_name: "actual-ordinary-kernel-source-hostcalls".into(),
        runtime_version: env!("CARGO_PKG_VERSION").into(),
        runtime_build_identity: format!("sha256:{:x}", Sha256::digest(&executable)),
        adapter_artifact_identity: format!(
            "sha256:{:x}",
            Sha256::digest(include_bytes!("model_compute_session.rs"))
        ),
        device_evidence: format!("hosted-{}-{}", std::env::consts::ARCH, std::env::consts::OS),
        precision_profile: model.model.artifact().precision_profile.clone(),
    };
    let mut session = Session::discover(&model, &plan, &context, runtime.clone()).unwrap();
    assert!(session.require_full_working_admission().is_err());
    assert!(session.unload().is_err());
    let definition = super::super::declarations::exact_epoch_declarations()
        + "\n"
        + include_str!("../../../../speech/fargan_epoch_feedback.conduit")
        + "\n"
        + include_str!("../../../../speech/fargan_conditioning_epoch_contracts.conduit");
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&definition),
        &conduit_plot::StartupCatalog::new(),
    )
    .unwrap();
    let ty = |name: &str| {
        &checked
            .native_types
            .iter()
            .find(|t| t.name == name)
            .unwrap()
            .value_type
    };
    let periods = run_native_period_controls(&[(false, 20480)]);
    let proposal = run_native_first_feature16k(&[1000; 160], &periods[0]);
    session.warm(&proposal).unwrap();
    assert_eq!(session.lifecycle.state(), ModelComputeLifecycle::Ready);
    assert!(session.warm(&proposal).is_err());
    let seeds =
        run_native_startup_feedback(&session.warm.as_ref().unwrap().result.values, &proposal);
    let seed = |name: &str| {
        seeds
            .iter()
            .find(|s| s.value_type() == ty(name))
            .unwrap()
            .canonical_bytes()
            .unwrap()
    };
    let inputs = BTreeMap::from([
        (
            "signal_seed".into(),
            vec![seed("FarganSignalEpochFeedback")],
        ),
        (
            "conditioning_seed".into(),
            vec![seed("FarganConditioningEpochFeedback")],
        ),
        (
            "events".into(),
            [1, 2]
                .iter()
                .map(|e| {
                    super::super::epoch_pair_fixture(ty("FarganFeatureConditionEpoch"), "", *e, *e)
                        .canonical_bytes()
                        .unwrap()
                })
                .collect(),
        ),
    ]);
    let names = BTreeMap::from([
        ("signal_seed".into(), "FarganSignalEpochFeedback".into()),
        (
            "conditioning_seed".into(),
            "FarganConditioningEpochFeedback".into(),
        ),
        ("events".into(), "FarganFeatureConditionEpoch".into()),
    ]);
    let mut foreign_plan = plan.clone();
    foreign_plan.plan_id = PlanId::from("foreign/plan");
    assert!(session.check_basis(&model, &foreign_plan).is_err());
    let mut malformed = inputs.clone();
    malformed.get_mut("events").unwrap()[0].push(0);
    assert!(session.enqueue(&checked, &names, malformed).is_err());
    let mut oversized = inputs.clone();
    oversized.insert("events".into(), vec![inputs["events"][0].clone(); 17]);
    assert!(session.enqueue(&checked, &names, oversized).is_err());
    session.enqueue(&checked, &names, inputs.clone()).unwrap();
    assert!(session.unload().is_err());
    assert!(
        session
            .execute(&context, &seeded, 1, ExecutionMode::LifecycleNormal)
            .is_err()
    );
    assert_eq!(session.lifecycle.state(), ModelComputeLifecycle::Ready);
    assert_eq!(session.last_batch.as_deref(), Some(&inputs));
    let exact_bytes = inputs
        .values()
        .flatten()
        .map(|v| v.len() as u64)
        .sum::<u64>();
    assert_eq!(session.queued_bytes, exact_bytes);
    assert!(session.enqueue(&checked, &names, inputs.clone()).is_err());
    let result = session
        .execute(&context, &seeded, 2, ExecutionMode::LifecycleNormal)
        .unwrap()
        .unwrap();
    assert!(result.drained);
    assert_eq!(result.values.len(), 2);
    assert_eq!(session.lifecycle.state(), ModelComputeLifecycle::Ready);
    let complete = session.material();
    session.enqueue(&checked, &names, inputs.clone()).unwrap();
    assert!(
        session
            .execute(&context, &seeded, 2, ExecutionMode::LifecycleCancel)
            .unwrap()
            .is_none()
    );
    assert!(session.stopped);
    assert!(session.enqueue(&checked, &names, inputs.clone()).is_err());
    assert!(
        session
            .execute(&context, &seeded, 2, ExecutionMode::LifecycleNormal)
            .is_err()
    );
    let cancelled = session.material();
    session.unload().unwrap();
    assert_eq!(session.lifecycle.state(), ModelComputeLifecycle::Shutdown);
    assert!(session.lifecycle.loaded_model_identity().is_none());
    let unloaded = session.material();
    assert!(session.retained_model.is_none());
    assert!(
        session
            .execute(&context, &seeded, 2, ExecutionMode::LifecycleNormal)
            .is_err()
    );
    let mut exceptional = Vec::new();
    for mode in [
        ExecutionMode::LifecyclePressure,
        ExecutionMode::LifecycleProviderLost,
    ] {
        let mut branch = Session::discover(&model, &plan, &context, runtime.clone()).unwrap();
        branch.warm(&proposal).unwrap();
        branch.enqueue(&checked, &names, inputs.clone()).unwrap();
        assert!(
            branch
                .execute(&context, &seeded, 2, mode)
                .unwrap()
                .is_none()
        );
        assert!(branch.stopped);
        assert!(branch.enqueue(&checked, &names, inputs.clone()).is_err());
        assert!(
            branch
                .execute(&context, &seeded, 2, ExecutionMode::LifecycleNormal)
                .is_err()
        );
        if matches!(mode, ExecutionMode::LifecycleProviderLost) {
            assert_eq!(branch.lifecycle.state(), ModelComputeLifecycle::Lost);
            assert!(branch.lifecycle.loaded_model_identity().is_none());
            assert!(branch.retained_model.is_none());
        } else {
            assert!(matches!(
                branch.lifecycle.state(),
                ModelComputeLifecycle::Active(_)
            ));
            assert!(branch.unload().is_err());
        }
        exceptional.push(branch.material());
    }
    if let Ok(path) = std::env::var("CONDUIT_FARGAN_COMPUTE_SESSION_RECEIPT") {
        std::fs::write(path,serde_json::to_vec(&serde_json::json!({"basis":session.basis_material(),"exceptional":exceptional,"complete":complete,"cancelled":cancelled,"unloaded":unloaded,"actual_outputs":result.values.iter().map(|v|v.canonical_bytes().unwrap()).collect::<Vec<_>>(),"full_working_admission":false,"target_execution":false})).unwrap()).unwrap();
    }
}
