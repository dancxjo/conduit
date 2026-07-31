use conduit_core::{
    BoundednessProfile, CancellationGuarantee, ExecutionLimits, ExecutionProfile, Id,
    ImplementationError, ImplementationMachine, InstancePath, InstantiationContext, LifecycleUsage,
    PinnedDescriptor, PrepareOutcome, SemanticHash, StepUsage, prepare_all, start_all,
};
use conduit_runtime::{
    ForeignStepReply, ForeignStepRequest, MessageStepBinding, MessageStepEndpoint,
    NativeStepBinding, NativeStepImplementation, OwnedStepOutcome, OwnedStepReply,
};

const FIXTURE: &str = include_str!("../../../conformance/c4/implementation-step.json");
const ZERO: SemanticHash = SemanticHash::from_bytes([0; 32]);
const LIMITS: ExecutionLimits = ExecutionLimits {
    max_step_work: 4,
    max_retained_values: 0,
    max_retained_bytes: 0,
    max_scratch_bytes: 0,
    max_input_leases: 0,
    max_input_bytes: 0,
    max_output_reservations: 0,
    max_output_bytes: 0,
    max_transactions: 1,
    max_fragments_per_step: 0,
    max_pending_operations: 0,
    max_timers: 0,
    max_child_tasks: 0,
    max_host_buffer_bytes: 0,
    max_foreign_queue_items: 0,
    max_foreign_queue_bytes: 0,
    max_checkpoint_bytes: 0,
    implementation_memory_bytes: 0,
    cancellation_ticks: 1,
};

fn profile() -> ExecutionProfile<'static> {
    let mut profile = ExecutionProfile {
        id: Id("fixture/adapter-profile"),
        schema_version: 0,
        semantic_hash: ZERO,
        boundedness: BoundednessProfile::Hard,
        cancellation: CancellationGuarantee::Bounded,
        step_bound_enforced: true,
        limits: LIMITS,
        representations: &[],
        memory_claims: &[],
        checkpoint: None,
    };
    profile.semantic_hash = profile.computed_semantic_hash(&mut []).unwrap();
    profile
}

fn started<'a>(profile: &'a ExecutionProfile<'a>) -> ImplementationMachine {
    let mut machines = [ImplementationMachine::instantiate(
        profile,
        InstantiationContext {
            instance: InstancePath::new("root/node").unwrap(),
            implementation: PinnedDescriptor {
                id: Id("fixture/implementation"),
                schema_version: 0,
                semantic_hash: SemanticHash::from_bytes([3; 32]),
            },
            artifact: Id("artifact/a"),
            execution_profile_hash: profile.semantic_hash,
            configuration_validated: true,
            caller_memory_bytes: 0,
            required_resource_bindings: &[],
            provided_resource_bindings: &[],
            required_grants: &[],
            provided_grants: &[],
            cancellation_scope: Id("scope/a"),
        },
    )
    .unwrap()];
    prepare_all(
        &mut machines,
        &[PrepareOutcome::Ready],
        &[LifecycleUsage::default()],
    )
    .unwrap();
    start_all(&mut machines, &[LifecycleUsage::default()]).unwrap();
    machines[0]
}

#[derive(Default)]
struct NativeUppercase {
    output: String,
}

impl NativeStepImplementation for NativeUppercase {
    fn step(&mut self, maximum_work: u32) -> OwnedStepReply {
        assert_eq!(maximum_work, 4);
        self.output = "hello".to_uppercase();
        OwnedStepReply {
            outcome: OwnedStepOutcome::Progress,
            interests: Vec::new(),
        }
    }
}

struct MessageUppercase {
    output: String,
    protocol_version: u16,
}

impl MessageStepEndpoint for MessageUppercase {
    fn exchange(&mut self, request: ForeignStepRequest) -> ForeignStepReply {
        assert_eq!(
            request,
            ForeignStepRequest {
                protocol_version: 0,
                sequence: 0,
                maximum_work: 4
            }
        );
        self.output = "hello".to_uppercase();
        ForeignStepReply {
            protocol_version: self.protocol_version,
            outcome: "progress".to_owned(),
            failure_code: None,
            interests: Vec::new(),
        }
    }
}

#[test]
fn direct_native_and_message_bindings_have_equivalent_semantics_and_evidence() {
    let profile = profile();
    let mut native_machine = started(&profile);
    let mut message_machine = started(&profile);
    let mut native = NativeStepBinding::new(NativeUppercase::default());
    let mut message = MessageStepBinding::new(MessageUppercase {
        output: String::new(),
        protocol_version: 0,
    });

    let executor_usage = StepUsage {
        work_units: 1,
        observable_operations: 1,
        ..StepUsage::default()
    };
    let native_observation = native.step(&mut native_machine, 4, executor_usage).unwrap();
    let message_observation = message
        .step(&mut message_machine, 4, executor_usage)
        .unwrap();
    assert_eq!(native_observation, message_observation);
    assert_eq!(native.into_inner().output, "HELLO");
    assert_eq!(message.into_inner().output, "HELLO");
}

#[test]
fn foreign_protocol_version_is_rejected_before_step_evidence() {
    let profile = profile();
    let mut machine = started(&profile);
    let mut message = MessageStepBinding::new(MessageUppercase {
        output: String::new(),
        protocol_version: 1,
    });
    assert_eq!(
        message.step(&mut machine, 4, StepUsage::default()),
        Err(ImplementationError::InvalidProfile)
    );
}

#[test]
fn every_hosted_binding_fixture_is_owned_here() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    let ids = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["runner"] == "hosted-bindings")
        .map(|case| case["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        [
            "native-message-binding-equivalence",
            "foreign-protocol-version-rejected"
        ]
    );
}
