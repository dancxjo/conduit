mod model_effect_common;

use conduit_ai::{EffectAuthority, ProposalGate};
use conduit_core::*;
use conduit_runtime::{
    HostRuntime, ImplementationFailure, ImplementationRegistry, OperationAction,
    OperationCompletion, OperationImplementation, OperationOutput, OperationState,
};
use model_effect_common::{proposal, wired_plan, ARGUMENT_KIND, EFFECT_KIND, EFFECT_OPERATION};

const PROPOSER_ID: &str = "placement/proposer";
const EFFECT_ID: &str = "placement/effect";

struct ProposerImplementation {
    kind_id: KindId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
    output: ValuePayload,
    requirements: Vec<HostOperationRequirement>,
}

impl OperationImplementation for ProposerImplementation {
    fn kind_id(&self) -> &KindId {
        &self.kind_id
    }

    fn kind_contract_revision(&self) -> KindContractRevision {
        KindContractRevision::from("conduit.llm/propose@1")
    }

    fn execution_profile_id(&self) -> ExecutionProfileId {
        ExecutionProfileId::from("test/hosted@1")
    }

    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation_id
    }

    fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    fn host_operation_requirements(&self) -> Vec<HostOperationRequirement> {
        self.requirements.clone()
    }

    fn prepare(
        &self,
        _placement: &PlannedGear,
    ) -> Result<Box<dyn OperationState>, ImplementationFailure> {
        Ok(Box::new(ProposerState {
            output: Some(self.output.clone()),
            emitted: false,
        }))
    }

    fn minimum_value_size(&self, _value_kind: &KindId) -> Option<u32> {
        Some(1)
    }
}

struct ProposerState {
    output: Option<ValuePayload>,
    emitted: bool,
}

impl OperationState for ProposerState {
    fn start(&mut self) -> OperationAction {
        OperationAction::Emit(vec![OperationOutput {
            port: port_id("result"),
            value: self.output.take().expect("one admitted request"),
        }])
    }

    fn resume(&mut self, completion: OperationCompletion) -> OperationAction {
        if completion == OperationCompletion::Emitted && !self.emitted {
            self.emitted = true;
            OperationAction::Complete
        } else {
            OperationAction::Fail(ImplementationFailure::new(
                FailureReason::InvalidLifecycleCommand,
                "unexpected proposal completion",
            ))
        }
    }
}

struct EffectImplementation {
    kind_id: KindId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
    expected_arguments: Vec<u8>,
    host_operation: HostOperationRequirement,
    authority: AuthorityRequirement,
}

impl OperationImplementation for EffectImplementation {
    fn kind_id(&self) -> &KindId {
        &self.kind_id
    }

    fn kind_contract_revision(&self) -> KindContractRevision {
        KindContractRevision::from("conduit.effect/send-message@1@1")
    }

    fn execution_profile_id(&self) -> ExecutionProfileId {
        ExecutionProfileId::from("test/hosted@1")
    }

    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation_id
    }

    fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    fn host_operation_requirements(&self) -> Vec<HostOperationRequirement> {
        vec![self.host_operation.clone()]
    }

    fn authority_requirements(&self) -> Vec<AuthorityRequirement> {
        vec![self.authority.clone()]
    }

    fn prepare(
        &self,
        _placement: &PlannedGear,
    ) -> Result<Box<dyn OperationState>, ImplementationFailure> {
        Ok(Box::new(EffectState {
            expected_arguments: self.expected_arguments.clone(),
            manifested: false,
        }))
    }

    fn minimum_value_size(&self, _value_kind: &KindId) -> Option<u32> {
        Some(1)
    }
}

struct EffectState {
    expected_arguments: Vec<u8>,
    manifested: bool,
}

impl OperationState for EffectState {
    fn start(&mut self) -> OperationAction {
        OperationAction::Idle
    }

    fn resume(&mut self, completion: OperationCompletion) -> OperationAction {
        match completion {
            OperationCompletion::Value { port, value }
                if port.as_str() == "request"
                    && value.value_kind.as_str() == ARGUMENT_KIND
                    && value.encoded == self.expected_arguments =>
            {
                OperationAction::Present {
                    presentation_kind: kind_id(EFFECT_KIND),
                    value,
                }
            }
            OperationCompletion::PresentationCompleted { success: true, .. } => {
                self.manifested = true;
                OperationAction::Idle
            }
            OperationCompletion::InputsClosed if self.manifested => OperationAction::Complete,
            OperationCompletion::PresentationCompleted {
                success: false,
                message,
            } => OperationAction::Fail(ImplementationFailure {
                reason: FailureReason::ManifestationFailed,
                message,
            }),
            _ => OperationAction::Fail(ImplementationFailure::new(
                FailureReason::InvalidLifecycleCommand,
                "unexpected effect completion",
            )),
        }
    }
}

fn capability(placement: &PlannedGear) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: placement.capability_id.clone(),
        kind_id: placement.kind_id.clone(),
        kind_contract_revision: placement.kind_contract_revision.clone(),
        implementation: ImplementationOffer {
            execution_profile_id: placement.execution_profile_id.clone(),
            implementation_id: placement.implementation_id.clone(),
            artifact_id: placement.artifact_id.clone(),
        },
        inputs: placement.inputs.clone(),
        outputs: placement.outputs.clone(),
        host_operations: placement.host_operations.clone(),
        resource_requirements: vec![],
        authority_requirements: placement
            .authority
            .iter()
            .map(|binding| AuthorityRequirement {
                contract_id: binding.contract_id.clone(),
                host_operation_contract_id: binding.host_operation_contract_id.clone(),
                subject_kind: binding.subject_kind.clone(),
            })
            .collect(),
        limits: placement.limits.clone(),
    }
}

fn runtime_for(request: &conduit_ai::AuthorizedEffectRequest, plan: &Plan) -> HostRuntime {
    let fragment = &plan.fragments[0];
    let proposer = fragment
        .placements
        .iter()
        .find(|placement| placement.placement_id.as_str() == PROPOSER_ID)
        .unwrap();
    let effect = fragment
        .placements
        .iter()
        .find(|placement| placement.placement_id.as_str() == EFFECT_ID)
        .unwrap();
    let authority = effect.authority[0].clone();
    let mut registry = ImplementationRegistry::new();
    registry
        .install(ProposerImplementation {
            kind_id: proposer.kind_id.clone(),
            implementation_id: proposer.implementation_id.clone(),
            artifact_id: proposer.artifact_id.clone(),
            output: ValuePayload {
                value_kind: kind_id(ARGUMENT_KIND),
                encoded: request.canonical_arguments.clone(),
            },
            requirements: proposer.host_operations.clone(),
        })
        .unwrap();
    registry
        .install(EffectImplementation {
            kind_id: effect.kind_id.clone(),
            implementation_id: effect.implementation_id.clone(),
            artifact_id: effect.artifact_id.clone(),
            expected_arguments: request.canonical_arguments.clone(),
            host_operation: effect.host_operations[0].clone(),
            authority: AuthorityRequirement {
                contract_id: authority.contract_id.clone(),
                host_operation_contract_id: authority.host_operation_contract_id.clone(),
                subject_kind: authority.subject_kind.clone(),
            },
        })
        .unwrap();
    let advertisement = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        offer_generation: fragment.offer_generation,
        profile: HostProfileId::from("test/model-effect-runtime@1"),
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vec![capability(proposer), capability(effect)],
    };
    HostRuntime::new_with_authority_grants(
        advertisement,
        registry,
        64,
        vec![AuthorityGrant {
            grant_id: authority.grant_id,
            contract_id: authority.contract_id,
            host_operation_contract_id: authority.host_operation_contract_id,
            subject_kind: authority.subject_kind,
            host_id: authority.host_id,
            boot_id: authority.boot_id,
            capability_id: authority.capability_id,
        }],
    )
}

#[test]
fn admitted_model_request_crosses_runtime_effect_and_returns_exact_sign() {
    let plan = wired_plan();
    let authority = EffectAuthority::from_plan(
        &plan,
        &PlacementId::from(PROPOSER_ID),
        &kind_id(EFFECT_KIND),
    )
    .unwrap();
    assert_eq!(
        plan.fragments[0].placements[1].host_operations[0]
            .contract_id
            .as_str(),
        EFFECT_OPERATION
    );
    let mut gate = ProposalGate::new(Some(authority), 2).unwrap();
    let request = gate.submit(proposal(&plan)).unwrap().request.unwrap();
    let mut runtime = runtime_for(&request, &plan);
    let prepared = runtime.handle(HostCommand::Prepare(plan.fragments[0].clone()));
    assert!(
        prepared
            .events
            .iter()
            .any(|event| matches!(event, HostEvent::Prepared { .. })),
        "{:?}",
        prepared.events
    );
    let started = runtime.handle(HostCommand::StartPlay(plan.plan_id.clone()));
    let effect = started
        .effects
        .into_iter()
        .find(|effect| matches!(effect, PlatformEffect::PresentValue { .. }))
        .expect("admitted request reaches the ordinary platform-effect boundary");
    let PlatformEffect::PresentValue {
        plan_id,
        active_play_id,
        presentation_id,
        placement_id,
        presentation_kind,
        value,
    } = &effect
    else {
        unreachable!()
    };
    assert_eq!(plan_id, &request.plan_id);
    assert_eq!(placement_id.as_str(), EFFECT_ID);
    assert_eq!(presentation_kind.as_str(), EFFECT_KIND);
    assert_eq!(value.encoded, request.canonical_arguments);

    let completed = runtime.handle(HostCommand::CompletePresentation {
        plan_id: plan_id.clone(),
        active_play_id: active_play_id.clone(),
        presentation_id: presentation_id.clone(),
        placement_id: placement_id.clone(),
        value: value.clone(),
        success: true,
        message: None,
    });
    assert!(completed.events.iter().any(|event| matches!(
        event,
        HostEvent::ManifestationCompleted { presentation_id: completed_id, .. }
            if completed_id == presentation_id
    )));
    let signs = runtime
        .handle(HostCommand::Inspect)
        .events
        .into_iter()
        .find_map(|event| match event {
            HostEvent::Observations { items } => Some(items),
            _ => None,
        })
        .unwrap();
    let observation = signs
        .iter()
        .find(|observation| {
            observation.presentation_id.as_ref() == Some(presentation_id)
                && matches!(observation.kind, ObservationKind::ValuePresented { .. })
        })
        .expect("runtime Sign correlates the actual manifestation")
        .clone();
    let mut wrong_plan = observation.clone();
    wrong_plan.plan_id = Some(PlanId::from("plan/fabricated"));
    assert_eq!(
        gate.complete_runtime_manifestation(&request, &effect, &wrong_plan),
        Err(conduit_ai::ProposalGateError::InvalidEffectReceipt)
    );
    assert_eq!(
        gate.complete_runtime_manifestation(
            &request,
            &PlatformEffect::Wait {
                plan_id: request.plan_id.clone(),
                placement_id: PlacementId::from(EFFECT_ID),
                duration_ms: 1,
            },
            &observation,
        ),
        Err(conduit_ai::ProposalGateError::InvalidEffectReceipt)
    );
    assert!(gate.effects().is_empty());
    let receipt = gate
        .complete_runtime_manifestation(&request, &effect, &observation)
        .unwrap();
    assert_ne!(receipt.effect_id, request.request_id);
    assert_eq!(receipt.resulting_signs, vec![observation.sign_id]);
}
