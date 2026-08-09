mod host_contract_fixtures;

use conduit_core::{
    kind_id, port_id, process_owned_link_binding, ArtifactId, BootId, CancellationPolicy,
    CapabilityId, CapabilityLimits, CapabilityOffer, CheckedFormId, ConnectionBase,
    ConnectionOutcome, ExecutionProfileId, ExpandedFormId, ExpectedClue, ExpectedTerminal,
    FailureReason, FormIdentity, FragmentId, GearId, HostCommand, HostEvent, HostId,
    HostOperationContractId, HostProfileId, ImplementationId, KindContractRevision,
    ObservationKind, OfferGeneration, PlacementId, PlanFragment, PlanId, PlannedGear,
    PlatformEffect, PortDirection, SourceDocumentId, TerminalDisposition, TerminalPolicy,
    ValuePayload, TIMER_RESOURCE_CLASS,
};
use conduit_form::parse;
use conduit_planner::{plan_with_link_bindings, PlacementChoice, PlacementChoices};
use conduit_runtime::{
    HostRuntime, ImplementationFailure, ImplementationRegistry, OperationImplementation,
};
use host_contract_fixtures::*;
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn prepare_is_effect_free_and_installed_profile_triggers_generically() {
    let advertisement = advertisement();
    let fragment = fragment(&advertisement);
    let mut runtime = HostRuntime::new(advertisement, registry(), 128);
    let prepared = runtime.handle(HostCommand::Prepare(fragment.clone()));
    assert!(prepared.effects.is_empty());
    assert!(matches!(
        prepared.events.first(),
        Some(HostEvent::Prepared { .. })
    ));

    let triggerd = runtime.handle(HostCommand::StartPlay(fragment.plan_id));
    assert!(triggerd.effects.iter().any(|effect| matches!(
        effect,
        PlatformEffect::PresentValue {
            presentation_kind,
            ..
        } if presentation_kind.as_str() == PRESENTATION_KIND
    )));
}

#[test]
fn planned_clue_storage_survives_observation_overflow() {
    let advertised = advertisement();
    let fragment = fragment(&advertised);
    let plan_id = fragment.plan_id.clone();
    let mut runtime = HostRuntime::new(advertised, registry(), 1);

    runtime.handle(HostCommand::Prepare(fragment.clone()));
    let prepared_report = mandatory_clue_reports(&mut runtime)
        .into_iter()
        .next()
        .expect("prepared plan has a mandatory clue report");
    assert_eq!(prepared_report.recorded, fragment.expected_clue[..3]);
    assert!(!prepared_report.overflowed);
    let allocated_item_slots = prepared_report.allocated_item_slots;
    assert!(allocated_item_slots >= u32::from(fragment.clue_storage_budget.item_capacity));

    let triggerd = runtime.handle(HostCommand::StartPlay(plan_id));
    let (plan_id, active_play_id, presentation_id, placement_id, value) = triggerd
        .effects
        .into_iter()
        .find_map(|effect| match effect {
            PlatformEffect::PresentValue {
                plan_id,
                active_play_id,
                presentation_id,
                placement_id,
                value,
                ..
            } => Some((
                plan_id,
                active_play_id,
                presentation_id,
                placement_id,
                value,
            )),
            _ => None,
        })
        .expect("presentation effect exists");
    assert!(triggerd.events.iter().any(|event| matches!(
        event,
        HostEvent::PlayStarted {
            active_play_id: triggerd_id,
            ..
        } if triggerd_id == &active_play_id
    )));
    runtime.handle(HostCommand::CompletePresentation {
        plan_id,
        active_play_id,
        presentation_id,
        placement_id,
        value,
        success: true,
        message: None,
    });

    let completed_report = mandatory_clue_reports(&mut runtime)
        .into_iter()
        .next()
        .expect("completed plan retains mandatory clue");
    assert_eq!(
        completed_report.recorded.len(),
        completed_report.expected.len()
    );
    assert!(completed_report
        .expected
        .iter()
        .all(|item| completed_report.recorded.contains(item)));
    assert_eq!(
        completed_report.storage_budget,
        fragment.clue_storage_budget
    );
    assert_eq!(
        completed_report.used_bytes,
        fragment.clue_storage_budget.byte_capacity
    );
    assert_eq!(completed_report.allocated_item_slots, allocated_item_slots);
    assert!(!completed_report.overflowed);
    assert!(observations(&mut runtime)
        .iter()
        .any(|observation| matches!(observation.kind, ObservationKind::ClueGap { .. })));
}

#[test]
fn preparation_rejects_uninstalled_and_mismatched_implementations_structurally() {
    let mut missing_advertisement = advertisement();
    missing_advertisement.capabilities[1]
        .implementation
        .implementation_id = ImplementationId::from("missing/sink-v1");
    let missing_fragment = fragment(&missing_advertisement);
    let mut runtime = HostRuntime::new(missing_advertisement, registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(missing_fragment))),
        Some(FailureReason::UnknownImplementation)
    );

    let advertised = advertisement();
    let mut mismatched_fragment = fragment(&advertised);
    mismatched_fragment
        .placements
        .iter_mut()
        .find(|placement| placement.kind_id.as_str() == SINK_KIND)
        .expect("sink placement exists")
        .implementation_id = ImplementationId::from("other/sink-v1");
    let mismatched_fragment = reseal_fragment(mismatched_fragment);
    let mut runtime = HostRuntime::new(advertised.clone(), registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(mismatched_fragment))),
        Some(FailureReason::AdvertisedImplementationMismatch)
    );
}

#[test]
fn preparation_rejects_unsupported_kind_and_invalid_configuration_structurally() {
    let advertised = advertisement();
    let mut unsupported = fragment(&advertised);
    unsupported.placements[0].kind_id = kind_id("contract/not-installed");
    let unsupported = reseal_fragment(unsupported);
    let mut runtime = HostRuntime::new(advertised.clone(), registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(unsupported))),
        Some(FailureReason::UnsupportedKind)
    );

    let mut invalid = fragment(&advertised);
    invalid
        .placements
        .iter_mut()
        .find(|placement| placement.kind_id.as_str() == SOURCE_KIND)
        .expect("source placement exists")
        .configuration
        .push(conduit_core::ConfigurationEntry {
            key: "unexpected".into(),
            value: conduit_core::ConfigurationValue::Bool(true),
        });
    let invalid = reseal_fragment(invalid);
    let mut runtime = HostRuntime::new(advertised.clone(), registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(invalid))),
        Some(FailureReason::InvalidGearConfiguration)
    );
}

struct UnsupportedValueImplementation(SourceImplementation);

impl OperationImplementation for UnsupportedValueImplementation {
    fn kind_id(&self) -> &conduit_core::KindId {
        self.0.kind_id()
    }

    fn kind_contract_revision(&self) -> KindContractRevision {
        self.0.kind_contract_revision()
    }

    fn execution_profile_id(&self) -> ExecutionProfileId {
        self.0.execution_profile_id()
    }

    fn implementation_id(&self) -> &ImplementationId {
        self.0.implementation_id()
    }

    fn artifact_id(&self) -> &ArtifactId {
        self.0.artifact_id()
    }

    fn host_operation_requirements(&self) -> Vec<conduit_core::HostOperationRequirement> {
        self.0.host_operation_requirements()
    }

    fn resource_requirements(&self) -> Vec<conduit_core::ResourceRequirement> {
        self.0.resource_requirements()
    }

    fn authority_requirements(&self) -> Vec<conduit_core::AuthorityRequirement> {
        self.0.authority_requirements()
    }

    fn prepare(
        &self,
        placement: &PlannedGear,
    ) -> Result<Box<dyn OperationState>, ImplementationFailure> {
        self.0.prepare(placement)
    }
}

#[test]
fn preparation_requires_exact_capability_ports_and_implementation_value_support() {
    let advertised = advertisement();
    let planned = fragment(&advertised);

    let mut lying_capability = advertised.clone();
    lying_capability.capabilities[0].outputs[0].value_kind = kind_id("contract/other-value");
    let mut runtime = HostRuntime::new(lying_capability, registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(planned.clone()))),
        Some(FailureReason::PortContractMismatch)
    );

    let mut unsupported_registry = ImplementationRegistry::new();
    unsupported_registry
        .install(UnsupportedValueImplementation(SourceImplementation::new(
            ImplementationId::from("contract/source-v1"),
        )))
        .expect("unsupported-value fixture installs");
    unsupported_registry
        .install(SinkImplementation::new(ImplementationId::from(
            "contract/sink-v1",
        )))
        .expect("sink installs");
    let mut runtime = HostRuntime::new(advertised, unsupported_registry, 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(planned))),
        Some(FailureReason::UnsupportedValueKind)
    );
}

fn assert_post_identity_mutation_is_rejected(
    advertised: &HostAdvertisement,
    fragment: PlanFragment,
) {
    let mut runtime = HostRuntime::new(advertised.clone(), registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(fragment))),
        Some(FailureReason::PlanIdentityMismatch)
    );
}

#[test]
fn preparation_rejects_mutation_of_every_executable_identity_field_group() {
    let advertised = advertisement();
    let original = fragment(&advertised);

    let mut mutated = original.clone();
    mutated.source_document_id = SourceDocumentId::from("mutated-source");
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    mutated.checked_form_id = CheckedFormId::from("mutated-checked");
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    mutated.expanded_form_id = ExpandedFormId::from("mutated-expanded");
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    mutated.placements[0].implementation_id = ImplementationId::from("mutated/implementation");
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    mutated.placements[0].artifact_id = ArtifactId::from("mutated/artifact");
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    mutated.placements[0].kind_contract_revision = KindContractRevision::from("mutated/contract@1");
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    mutated.placements[0].execution_profile_id = ExecutionProfileId::from("mutated/profile@1");
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    mutated
        .placements
        .iter_mut()
        .find(|placement| !placement.outputs.is_empty())
        .expect("source placement exists")
        .outputs[0]
        .port_id = port_id("mutated-output");
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    let requirement = mutated
        .placements
        .iter_mut()
        .find_map(|placement| placement.host_operations.first_mut())
        .expect("host-operation requirement exists");
    requirement.contract_id = HostOperationContractId::from("mutated/host-operation@1");
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    let requirement = mutated
        .placements
        .iter_mut()
        .find_map(|placement| placement.host_operations.first_mut())
        .expect("host-operation requirement exists");
    requirement.target_kind = Some(kind_id("mutated/host-operation-target"));
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    let requirement = mutated
        .placements
        .iter_mut()
        .find_map(|placement| placement.host_operations.first_mut())
        .expect("host-operation requirement exists");
    requirement.maximum_in_flight += 1;
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    let requirement = mutated
        .placements
        .iter_mut()
        .find_map(|placement| placement.host_operations.first_mut())
        .expect("host-operation requirement exists");
    requirement.maximum_input_bytes += 1;
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    let requirement = mutated
        .placements
        .iter_mut()
        .find_map(|placement| placement.host_operations.first_mut())
        .expect("host-operation requirement exists");
    requirement.maximum_output_bytes += 1;
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    let binding = mutated
        .placements
        .iter_mut()
        .find_map(|placement| placement.resources.first_mut())
        .expect("resource binding exists");
    binding.pool_id = conduit_core::ResourcePoolId::from("mutated/resource-pool");
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    let binding = mutated
        .placements
        .iter_mut()
        .find_map(|placement| placement.resources.first_mut())
        .expect("resource binding exists");
    binding.class_id = conduit_core::ResourceClassId::from("mutated/resource-class@1");
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    let binding = mutated
        .placements
        .iter_mut()
        .find_map(|placement| placement.resources.first_mut())
        .expect("resource binding exists");
    binding.units += 1;
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    mutated
        .placements
        .iter_mut()
        .find(|placement| !placement.outputs.is_empty())
        .expect("source placement exists")
        .outputs[0]
        .value_kind = kind_id("mutated/port-value");
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    mutated
        .placements
        .iter_mut()
        .find(|placement| !placement.outputs.is_empty())
        .expect("source placement exists")
        .outputs[0]
        .direction = PortDirection::Input;
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    mutated
        .placements
        .iter_mut()
        .find(|placement| !placement.outputs.is_empty())
        .expect("source placement exists")
        .outputs[0]
        .temporal = conduit_core::PortTemporal::Current;
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    mutated.connections[0].value_kind = kind_id("mutated/value");
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    mutated.connections[0].temporal = conduit_core::PortTemporal::Current;
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    mutated.connections[0].item_capacity += 1;
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    mutated.connections[0].byte_capacity += 1;
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    mutated.startup_dependencies.clear();
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    mutated.startup_order.reverse();
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    mutated.cancellation_policy = CancellationPolicy::DrainBeforeCancel;
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    mutated.terminal_policy = TerminalPolicy::RequirePlacementsOnly;
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    mutated.expected_terminals.pop();
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    mutated.expected_clue.pop();
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    mutated.clue_storage_budget.item_capacity -= 1;
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    mutated.clue_storage_budget.byte_capacity -= 1;
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    mutated
        .placements
        .iter_mut()
        .find(|placement| placement.kind_id.as_str() == SOURCE_KIND)
        .expect("source placement exists")
        .configuration
        .push(conduit_core::ConfigurationEntry {
            key: "mutated".into(),
            value: conduit_core::ConfigurationValue::U64(99),
        });
    assert_post_identity_mutation_is_rejected(&advertised, mutated);
}

#[test]
fn preparation_rejects_resealed_contract_profile_and_port_lies() {
    let advertised = advertisement();

    let mut mutated = fragment(&advertised);
    mutated.placements[0].kind_contract_revision = KindContractRevision::from("mutated/contract@1");
    let mut runtime = HostRuntime::new(advertised.clone(), registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(reseal_fragment(mutated)))),
        Some(FailureReason::KindContractRevisionMismatch)
    );

    let mut mutated = fragment(&advertised);
    mutated.placements[0].execution_profile_id = ExecutionProfileId::from("mutated/profile@1");
    let mut runtime = HostRuntime::new(advertised.clone(), registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(reseal_fragment(mutated)))),
        Some(FailureReason::ExecutionProfileMismatch)
    );

    let mut mutated = fragment(&advertised);
    let placement = mutated
        .placements
        .iter_mut()
        .find(|placement| !placement.outputs.is_empty())
        .expect("source placement exists");
    placement.outputs[0].port_id = port_id("mutated-output");
    let mut runtime = HostRuntime::new(advertised.clone(), registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(reseal_fragment(mutated)))),
        Some(FailureReason::PortContractMismatch)
    );

    let mut mutated = fragment(&advertised);
    mutated
        .placements
        .iter_mut()
        .find_map(|placement| placement.host_operations.first_mut())
        .expect("host-operation requirement exists")
        .maximum_input_bytes += 1;
    let mut runtime = HostRuntime::new(advertised.clone(), registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(reseal_fragment(mutated)))),
        Some(FailureReason::HostOperationContractMismatch)
    );

    let mut mutated = fragment(&advertised);
    mutated
        .placements
        .iter_mut()
        .find_map(|placement| placement.host_operations.first_mut())
        .expect("host-operation requirement exists")
        .target_kind = Some(kind_id("mutated/presentation-target"));
    let mut runtime = HostRuntime::new(advertised.clone(), registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(reseal_fragment(mutated)))),
        Some(FailureReason::HostOperationContractMismatch)
    );

    let host_operation_fragment = fragment(&advertised);
    let mut runtime = HostRuntime::new(advertised, undersized_presentation_registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(host_operation_fragment))),
        Some(FailureReason::HostOperationContractMismatch)
    );

    let advertised = advertisement();
    let mut mutated = fragment(&advertised);
    mutated
        .placements
        .iter_mut()
        .find_map(|placement| placement.resources.first_mut())
        .expect("resource binding exists")
        .units += 1;
    let mut runtime = HostRuntime::new(advertised.clone(), registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(reseal_fragment(mutated)))),
        Some(FailureReason::ResourceContractMismatch)
    );

    let unavailable_pool_fragment = fragment(&advertised);
    let mut current = advertised.clone();
    current.resources[0].class_id = conduit_core::ResourceClassId::from("mutated/resource-class@1");
    let mut runtime = HostRuntime::new(current, registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(unavailable_pool_fragment))),
        Some(FailureReason::ResourceContractMismatch)
    );

    let implementation_mismatch_fragment = fragment(&advertised);
    let mut runtime = HostRuntime::new(advertised, missing_resource_registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(implementation_mismatch_fragment))),
        Some(FailureReason::ResourceContractMismatch)
    );
}

#[test]
fn preparation_rejects_resealed_policy_dependency_and_budget_lies() {
    let advertised = advertisement();

    let mut mutated = fragment(&advertised);
    mutated.startup_dependencies.clear();
    let mut runtime = HostRuntime::new(advertised.clone(), registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(reseal_fragment(mutated)))),
        Some(FailureReason::InvalidStartupDependencies)
    );

    let mut mutated = fragment(&advertised);
    mutated.startup_order.reverse();
    let mut runtime = HostRuntime::new(advertised.clone(), registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(reseal_fragment(mutated)))),
        Some(FailureReason::InvalidStartupDependencies)
    );

    let mut mutated = fragment(&advertised);
    mutated.cancellation_policy = CancellationPolicy::DrainBeforeCancel;
    let mut runtime = HostRuntime::new(advertised.clone(), registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(reseal_fragment(mutated)))),
        Some(FailureReason::UnsupportedCancellationPolicy)
    );

    let mut mutated = fragment(&advertised);
    mutated.terminal_policy = TerminalPolicy::RequirePlacementsOnly;
    let mut runtime = HostRuntime::new(advertised.clone(), registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(reseal_fragment(mutated)))),
        Some(FailureReason::UnsupportedTerminalPolicy)
    );

    let mut mutated = fragment(&advertised);
    mutated.clue_storage_budget.item_capacity -= 1;
    let mut runtime = HostRuntime::new(advertised.clone(), registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(reseal_fragment(mutated)))),
        Some(FailureReason::ClueBudgetExceeded)
    );

    let mut mutated = fragment(&advertised);
    mutated.clue_storage_budget.byte_capacity -= 1;
    let mut runtime = HostRuntime::new(advertised, registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(reseal_fragment(mutated)))),
        Some(FailureReason::ClueBudgetExceeded)
    );
}

#[test]
fn host_with_only_pulse_rejects_show_and_wrong_kind_registry_entries() {
    let advertised = advertisement();
    let planned = fragment(&advertised);
    let mut pulse_only = ImplementationRegistry::new();
    pulse_only
        .install(SourceImplementation::new(ImplementationId::from(
            "contract/source-v1",
        )))
        .expect("pulse installs");
    let mut runtime = HostRuntime::new(advertised.clone(), pulse_only, 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(planned.clone()))),
        Some(FailureReason::UnknownImplementation)
    );

    let mut wrong_kind = ImplementationRegistry::new();
    wrong_kind
        .install(SourceImplementation::new(ImplementationId::from(
            "contract/source-v1",
        )))
        .expect("pulse installs");
    wrong_kind
        .install(SourceImplementation::new(ImplementationId::from(
            "contract/sink-v1",
        )))
        .expect("wrong-kind fixture installs");
    let mut runtime = HostRuntime::new(advertised, wrong_kind, 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(planned))),
        Some(FailureReason::ImplementationKindMismatch)
    );
}

struct EchoImplementation {
    kind_id: conduit_core::KindId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
}

impl OperationImplementation for EchoImplementation {
    fn kind_id(&self) -> &conduit_core::KindId {
        &self.kind_id
    }

    fn kind_contract_revision(&self) -> KindContractRevision {
        KindContractRevision::from("test/echo@1")
    }

    fn execution_profile_id(&self) -> ExecutionProfileId {
        ExecutionProfileId::from("test/echo-hosted@1")
    }

    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation_id
    }

    fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    fn prepare(
        &self,
        _placement: &PlannedGear,
    ) -> Result<Box<dyn OperationState>, ImplementationFailure> {
        Ok(Box::new(EchoState))
    }
}

struct EchoState;

impl OperationState for EchoState {
    fn start(&mut self) -> OperationAction {
        OperationAction::Complete
    }

    fn resume(&mut self, _completion: OperationCompletion) -> OperationAction {
        OperationAction::Complete
    }
}

#[test]
fn echo_kind_uses_only_the_installed_implementation_boundary() {
    let echo_kind_id = kind_id("test/echo");
    let implementation_id = ImplementationId::from("test/echo-v1");
    let advertisement = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("echo-host"),
        boot_id: BootId::from("echo-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("echo-test"),
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vec![CapabilityOffer {
            startup_parameters: vec![],
            shorthand: None,
            capability_id: CapabilityId::from("echo-capability"),
            kind_id: echo_kind_id.clone(),
            kind_contract_revision: KindContractRevision::from("test/echo@1"),
            implementation: conduit_core::ImplementationOffer {
                execution_profile_id: ExecutionProfileId::from("test/echo-hosted@1"),
                implementation_id: implementation_id.clone(),
                artifact_id: ArtifactId::from("test/echo-artifact-v1"),
            },
            inputs: vec![],
            outputs: vec![],
            host_operations: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 0,
                max_queue_bytes: 0,
            },
        }],
    };
    let placement_id = PlacementId::from("echo-placement");
    let expected_clue = vec![
        ExpectedClue::PlanFragmentReceived,
        ExpectedClue::PlacementPrepared(placement_id.clone()),
        ExpectedClue::PlacementTerminal(placement_id.clone()),
        ExpectedClue::PlanTerminal,
    ];
    let clue_storage_budget =
        mandatory_clue_storage_requirement(&expected_clue).expect("echo clue fits budget types");
    let mut echo_plan = seal_plan(
        FormIdentity {
            source_document_id: SourceDocumentId::from("echo-source"),
            checked_form_id: CheckedFormId::from("echo-form"),
            expanded_form_id: ExpandedFormId::from("echo-expanded"),
        },
        vec![PlanFragment {
            plan_id: PlanId::from(""),
            fragment_id: FragmentId::from(""),
            source_document_id: SourceDocumentId::from("echo-source"),
            checked_form_id: CheckedFormId::from("echo-form"),
            expanded_form_id: ExpandedFormId::from("echo-expanded"),
            host_id: advertisement.host_id.clone(),
            boot_id: advertisement.boot_id.clone(),
            offer_generation: advertisement.offer_generation,
            placements: vec![PlannedGear {
                placement_id: placement_id.clone(),
                gear_id: GearId::from("echo"),
                kind_id: echo_kind_id.clone(),
                kind_contract_revision: KindContractRevision::from("test/echo@1"),
                execution_profile_id: ExecutionProfileId::from("test/echo-hosted@1"),
                configuration: Vec::new(),
                host_id: advertisement.host_id.clone(),
                boot_id: advertisement.boot_id.clone(),
                offer_generation: advertisement.offer_generation,
                capability_id: CapabilityId::from("echo-capability"),
                implementation_id: implementation_id.clone(),
                artifact_id: ArtifactId::from("test/echo-artifact-v1"),
                realization_characteristics: Vec::new(),
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: 1,
                    max_queue_bytes: 1,
                },
                inputs: Vec::new(),
                outputs: Vec::new(),
                host_operations: Vec::new(),
                resources: Vec::new(),
                authority: Vec::new(),
                pool_references: Vec::new(),
            }],
            connections: Vec::new(),
            shared_pools: Vec::new(),
            startup_dependencies: Vec::new(),
            startup_order: vec![placement_id.clone()],
            cancellation_policy: CancellationPolicy::CancelAllAndRejectLateCompletion,
            terminal_policy: TerminalPolicy::RequireAllPlacementsAndConnections,
            expected_terminals: vec![
                ExpectedTerminal::PlacementCompleted(placement_id.clone()),
                ExpectedTerminal::PlanCompleted,
            ],
            expected_clue,
            clue_storage_budget,
            plan_fragments: Vec::new(),
        }],
    );
    let fragment = echo_plan.fragments.remove(0);
    let plan_id = echo_plan.plan_id;
    let mut missing_runtime =
        HostRuntime::new(advertisement.clone(), ImplementationRegistry::new(), 32);
    assert_eq!(
        rejection_reason(&missing_runtime.handle(HostCommand::Prepare(fragment.clone()))),
        Some(FailureReason::UnknownImplementation)
    );

    let mut registry = ImplementationRegistry::new();
    registry
        .install(EchoImplementation {
            kind_id: echo_kind_id,
            implementation_id,
            artifact_id: ArtifactId::from("test/echo-artifact-v1"),
        })
        .expect("echo implementation installs");
    let mut runtime = HostRuntime::new(advertisement, registry, 32);
    assert!(matches!(
        runtime
            .handle(HostCommand::Prepare(fragment))
            .events
            .first(),
        Some(HostEvent::Prepared { .. })
    ));
    let triggerd = runtime.handle(HostCommand::StartPlay(plan_id));
    assert!(triggerd.events.iter().any(|event| matches!(
        event,
        HostEvent::PlanTerminated {
            disposition: TerminalDisposition::Completed,
            ..
        }
    )));
}

#[test]
fn fake_adapter_failure_is_structured_and_terminal() {
    let advertisement = advertisement();
    let fragment = fragment(&advertisement);
    let plan_id = fragment.plan_id.clone();
    let mut runtime = HostRuntime::new(advertisement, registry(), 128);
    runtime.handle(HostCommand::Prepare(fragment));
    let triggerd = runtime.handle(HostCommand::StartPlay(plan_id));
    let (plan_id, active_play_id, presentation_id, placement_id, value) = triggerd
        .effects
        .into_iter()
        .find_map(|effect| match effect {
            PlatformEffect::PresentValue {
                plan_id,
                active_play_id,
                presentation_id,
                placement_id,
                value,
                ..
            } => Some((
                plan_id,
                active_play_id,
                presentation_id,
                placement_id,
                value,
            )),
            _ => None,
        })
        .expect("presentation effect exists");
    assert_ne!(active_play_id.as_str(), plan_id.as_str());
    assert_ne!(presentation_id.as_str(), plan_id.as_str());
    assert_ne!(presentation_id.as_str(), active_play_id.as_str());
    let wrong_identity = runtime.handle(HostCommand::CompletePresentation {
        plan_id: plan_id.clone(),
        active_play_id: active_play_id.clone(),
        presentation_id: conduit_core::PresentationId::from("wrong-presentation"),
        placement_id: placement_id.clone(),
        value: value.clone(),
        success: false,
        message: Some("must not consume the pending presentation".to_string()),
    });
    assert!(wrong_identity.events.iter().any(|event| matches!(
        event,
        HostEvent::CommandRejected {
            reason: FailureReason::LatePlatformCompletion,
            ..
        }
    )));
    let failed = runtime.handle(HostCommand::CompletePresentation {
        plan_id,
        active_play_id: active_play_id.clone(),
        presentation_id: presentation_id.clone(),
        placement_id,
        value,
        success: false,
        message: Some("fake adapter failed".to_string()),
    });
    assert!(failed.events.iter().any(|event| matches!(
        event,
        HostEvent::ManifestationFailed {
            reason: FailureReason::ManifestationFailed,
            ..
        }
    )));
    let clue = observations(&mut runtime);
    assert!(clue.iter().any(|observation| {
        observation.active_play_id.as_ref() == Some(&active_play_id)
            && observation.presentation_id.as_ref() == Some(&presentation_id)
    }));
    assert_eq!(
        clue.iter()
            .map(|observation| observation.clue_id.clone())
            .collect::<BTreeSet<_>>()
            .len(),
        clue.len()
    );
    assert!(failed.events.iter().any(|event| matches!(
        event,
        HostEvent::PlanTerminated {
            disposition: TerminalDisposition::Failed { .. },
            ..
        }
    )));
}

struct AdapterSourceImplementation {
    kind_id: conduit_core::KindId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
    declares_wait: bool,
}

impl OperationImplementation for AdapterSourceImplementation {
    fn kind_id(&self) -> &conduit_core::KindId {
        &self.kind_id
    }

    fn kind_contract_revision(&self) -> KindContractRevision {
        KindContractRevision::from(SOURCE_CONTRACT)
    }

    fn execution_profile_id(&self) -> ExecutionProfileId {
        ExecutionProfileId::from(SOURCE_PROFILE)
    }

    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation_id
    }

    fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    fn host_operation_requirements(&self) -> Vec<conduit_core::HostOperationRequirement> {
        if self.declares_wait {
            vec![wait_host_operation_requirement()]
        } else {
            Vec::new()
        }
    }

    fn resource_requirements(&self) -> Vec<conduit_core::ResourceRequirement> {
        if self.declares_wait {
            vec![resource_requirement(TIMER_RESOURCE_CLASS, 1)]
        } else {
            Vec::new()
        }
    }

    fn prepare(
        &self,
        _placement: &PlannedGear,
    ) -> Result<Box<dyn OperationState>, ImplementationFailure> {
        Ok(Box::new(AdapterSourceState { emitted: false }))
    }

    fn minimum_value_size(&self, value_kind: &conduit_core::KindId) -> Option<u32> {
        (value_kind.as_str() == VALUE_KIND).then_some(1)
    }
}

struct AdapterSourceState {
    emitted: bool,
}

impl OperationState for AdapterSourceState {
    fn start(&mut self) -> OperationAction {
        OperationAction::Wait { duration_ms: 25 }
    }

    fn resume(&mut self, completion: OperationCompletion) -> OperationAction {
        match completion {
            OperationCompletion::TimerElapsed if !self.emitted => {
                self.emitted = true;
                OperationAction::Emit(vec![OperationOutput {
                    port: port_id("value"),
                    value: ValuePayload {
                        value_kind: kind_id(VALUE_KIND),
                        encoded: vec![7],
                    },
                }])
            }
            OperationCompletion::Emitted if self.emitted => OperationAction::Complete,
            _ => OperationAction::Fail(ImplementationFailure::new(
                FailureReason::InvalidLifecycleCommand,
                "fake adapter source received an unexpected completion",
            )),
        }
    }
}

#[derive(Default)]
struct FakeBrowserStyleAdapter {
    saw_wait: bool,
    saw_presentation: bool,
    delayed_connection: Option<conduit_core::ConnectionEnvelope>,
}

impl FakeBrowserStyleAdapter {
    fn receive(&mut self, effects: Vec<PlatformEffect>) {
        for effect in effects {
            match effect {
                PlatformEffect::Wait { .. } => self.saw_wait = true,
                PlatformEffect::PresentValue { .. } => self.saw_presentation = true,
                PlatformEffect::TransmitConnection { envelope } => {
                    self.delayed_connection = Some(envelope);
                }
            }
        }
    }
}

fn adapter_registry() -> ImplementationRegistry {
    let mut registry = ImplementationRegistry::new();
    registry
        .install(AdapterSourceImplementation {
            kind_id: kind_id(SOURCE_KIND),
            implementation_id: ImplementationId::from("contract/source-v1"),
            artifact_id: ArtifactId::from("contract/source-artifact-v1"),
            declares_wait: true,
        })
        .expect("adapter source installs");
    registry
        .install(SinkImplementation::new(ImplementationId::from(
            "contract/sink-v1",
        )))
        .expect("adapter sink installs");
    registry
}

fn undeclared_adapter_registry() -> ImplementationRegistry {
    let mut registry = ImplementationRegistry::new();
    registry
        .install(AdapterSourceImplementation {
            kind_id: kind_id(SOURCE_KIND),
            implementation_id: ImplementationId::from("contract/source-v1"),
            artifact_id: ArtifactId::from("contract/source-artifact-v1"),
            declares_wait: false,
        })
        .expect("undeclared adapter source installs");
    registry
        .install(SinkImplementation::new(ImplementationId::from(
            "contract/sink-v1",
        )))
        .expect("sink installs");
    registry
}

fn undersized_presentation_registry() -> ImplementationRegistry {
    let mut registry = ImplementationRegistry::new();
    registry
        .install(SourceImplementation::new(ImplementationId::from(
            "contract/source-v1",
        )))
        .expect("source installs");
    registry
        .install(
            SinkImplementation::new(ImplementationId::from("contract/sink-v1"))
                .with_maximum_presentation_bytes(0),
        )
        .expect("bounded sink installs");
    registry
}

fn missing_resource_registry() -> ImplementationRegistry {
    let mut registry = ImplementationRegistry::new();
    registry
        .install(SourceImplementation::new(ImplementationId::from(
            "contract/source-v1",
        )))
        .expect("source installs");
    registry
        .install(
            SinkImplementation::new(ImplementationId::from("contract/sink-v1"))
                .with_presentation_resource_units(0),
        )
        .expect("resource-mismatched sink installs");
    registry
}

fn authority_registry() -> ImplementationRegistry {
    let mut registry = ImplementationRegistry::new();
    registry
        .install(SourceImplementation::new(ImplementationId::from(
            "contract/source-v1",
        )))
        .expect("source installs");
    registry
        .install(
            SinkImplementation::new(ImplementationId::from("contract/sink-v1"))
                .with_presentation_authority(),
        )
        .expect("authority-declaring sink installs");
    registry
}

fn ungranted_authority_registry() -> ImplementationRegistry {
    let mut registry = ImplementationRegistry::new();
    registry
        .install(SourceImplementation::new(ImplementationId::from(
            "contract/source-v1",
        )))
        .expect("source installs");
    registry
        .install(
            SinkImplementation::new(ImplementationId::from("contract/sink-v1"))
                .with_presentation_authority()
                .with_ungranted_presentation(kind_id(UNGRANTED_PRESENTATION_KIND)),
        )
        .expect("mis-scoped authority sink installs");
    registry
}

fn ungranted_authority_advertisement() -> HostAdvertisement {
    let mut advertised = authority_advertisement();
    advertised.capabilities[1]
        .host_operations
        .push(present_host_operation_requirement(
            kind_id(UNGRANTED_PRESENTATION_KIND),
            1,
        ));
    advertised.capabilities[1].host_operations.sort();
    advertised
}

fn adapter_advertisement() -> HostAdvertisement {
    let mut advertised = advertisement();
    advertised.capabilities[0].host_operations = vec![wait_host_operation_requirement()];
    advertised.capabilities[0].resource_requirements =
        vec![resource_requirement(TIMER_RESOURCE_CLASS, 1)];
    advertised
}

fn remote_link_plan_fixture() -> (
    HostAdvertisement,
    HostAdvertisement,
    conduit_core::Plan,
    conduit_core::LinkBinding,
) {
    let form = parse(
        "form 0\n\nlink-runtime {\n source: contract/source\n sink: contract/sink\n source > sink\n}\n",
        &profile_catalog(),
    )
    .expect("remote link form parses");
    let mut source = adapter_advertisement();
    source.host_id = HostId::from("link-source-host");
    source.boot_id = BootId::from("link-source-boot");
    source.capabilities.truncate(1);
    let mut sink = advertisement();
    sink.host_id = HostId::from("link-sink-host");
    sink.boot_id = BootId::from("link-sink-boot");
    sink.capabilities.remove(0);
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("source"),
                PlacementChoice {
                    host_id: source.host_id.clone(),
                    capability_id: CapabilityId::from("pulse"),
                },
            ),
            (
                GearId::from("sink"),
                PlacementChoice {
                    host_id: sink.host_id.clone(),
                    capability_id: CapabilityId::from("show"),
                },
            ),
        ]),
    };
    let link = process_owned_link_binding(
        "link/runtime",
        ConnectionBase::InMemory,
        "fixture/in-memory/runtime",
        &source,
        &sink,
        conduit_core::DEFAULT_CONNECTION_ITEM_CAPACITY,
        conduit_core::DEFAULT_CONNECTION_BYTE_CAPACITY,
    );
    let plan = plan_with_link_bindings(
        &form,
        &[source.clone(), sink.clone()],
        &placements,
        &[],
        conduit_core::DEFAULT_CONNECTION_ITEM_CAPACITY,
        conduit_core::DEFAULT_CONNECTION_BYTE_CAPACITY,
        std::slice::from_ref(&link),
    )
    .expect("observed remote link resolves");
    (source, sink, plan, link)
}

fn remote_link_source_fixture() -> (HostAdvertisement, PlanFragment, conduit_core::LinkBinding) {
    let (source, _sink, plan, link) = remote_link_plan_fixture();
    let fragment = plan
        .fragments
        .into_iter()
        .find(|fragment| fragment.host_id == source.host_id)
        .expect("source fragment exists");
    (source, fragment, link)
}

#[test]
fn exact_remote_fragments_lower_to_directional_kernel_cords() {
    let (source, sink, plan, link) = remote_link_plan_fixture();
    let source_fragment = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id == source.host_id)
        .expect("source fragment");
    let sink_fragment = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id == sink.host_id)
        .expect("sink fragment");
    let source_lowered = conduit_runtime::lowering::lower_plan_fragment(source_fragment)
        .expect("remote source lowers");
    let sink_lowered =
        conduit_runtime::lowering::lower_plan_fragment(sink_fragment).expect("remote sink lowers");

    assert_eq!(source_lowered.remote_endpoints.len(), 1);
    assert_eq!(sink_lowered.remote_endpoints.len(), 1);
    assert_eq!(source_lowered.routes.len(), 1);
    assert!(sink_lowered.routes.is_empty());
    let egress = &source_lowered.remote_endpoints[0];
    let ingress = &sink_lowered.remote_endpoints[0];
    assert_eq!(
        egress.direction,
        conduit_runtime::lowering::RemoteCordDirection::Egress
    );
    assert_eq!(
        ingress.direction,
        conduit_runtime::lowering::RemoteCordDirection::Ingress
    );
    assert_eq!(egress.binding, link);
    assert_eq!(ingress.binding, link);
    assert_eq!(egress.local, link.source);
    assert_eq!(egress.peer, link.sink);
    assert_eq!(ingress.local, link.sink);
    assert_eq!(ingress.peer, link.source);
    assert_eq!(egress.connection_id, ingress.connection_id);
    assert_eq!(egress.value_kind, ingress.value_kind);
    assert_eq!(
        source_lowered.cords[0].spec.sink,
        conduit_kernel::CordEndpoint::Remote(egress.endpoint)
    );
    assert_eq!(
        sink_lowered.cords[0].spec.source,
        conduit_kernel::CordEndpoint::Remote(ingress.endpoint)
    );
    assert_eq!(
        sink_lowered.node_specs[0].input_cords[0],
        Some(sink_lowered.cords[0].spec.cord)
    );
    assert_eq!(
        source_lowered.identity.remote_endpoints,
        vec![(egress.endpoint, egress.connection_id.clone())]
    );
    assert_eq!(
        sink_lowered.identity.remote_endpoints,
        vec![(ingress.endpoint, ingress.connection_id.clone())]
    );
    assert_eq!(
        source_lowered
            .identity
            .connection_for_remote_endpoint(egress.endpoint),
        Some(&egress.connection_id)
    );
    assert_eq!(
        sink_lowered
            .identity
            .remote_endpoint_for_connection(&ingress.connection_id),
        Some(ingress.endpoint)
    );
}

fn observations(runtime: &mut HostRuntime) -> Vec<conduit_core::Observation> {
    runtime
        .handle(HostCommand::Inspect)
        .events
        .into_iter()
        .find_map(|event| match event {
            HostEvent::Observations { items } => Some(items),
            _ => None,
        })
        .expect("inspection returns observations")
}

#[test]
fn runtime_rejects_an_implementation_that_requests_an_unplanned_host_operation() {
    let advertised = advertisement();
    let fragment = fragment(&advertised);
    let plan_id = fragment.plan_id.clone();
    let mut runtime = HostRuntime::new(advertised, undeclared_adapter_registry(), 64);
    assert!(runtime
        .handle(HostCommand::Prepare(fragment))
        .events
        .iter()
        .any(|event| matches!(event, HostEvent::Prepared { .. })));
    let triggerd = runtime.handle(HostCommand::StartPlay(plan_id));
    assert!(triggerd.effects.is_empty());
    assert!(triggerd.events.iter().any(|event| matches!(
        event,
        HostEvent::PlacementTerminated {
            disposition: TerminalDisposition::Failed {
                reason: FailureReason::HostOperationNotPlanned
            },
            ..
        }
    )));
}

#[test]
fn runtime_rejects_a_host_operation_input_above_its_planned_bound() {
    let mut advertised = advertisement();
    advertised.capabilities[1].host_operations[0].maximum_input_bytes = 0;
    let fragment = fragment(&advertised);
    let plan_id = fragment.plan_id.clone();
    let mut runtime = HostRuntime::new(advertised, undersized_presentation_registry(), 64);
    assert!(runtime
        .handle(HostCommand::Prepare(fragment))
        .events
        .iter()
        .any(|event| matches!(event, HostEvent::Prepared { .. })));
    let triggerd = runtime.handle(HostCommand::StartPlay(plan_id));
    assert!(triggerd.effects.is_empty());
    assert!(triggerd.events.iter().any(|event| matches!(
        event,
        HostEvent::PlacementTerminated {
            disposition: TerminalDisposition::Failed {
                reason: FailureReason::HostOperationInputExceeded
            },
            ..
        }
    )));
}

#[test]
fn preparation_reserves_resource_pool_capacity_until_release() {
    let mut advertised = advertisement();
    for resource in &mut advertised.resources {
        resource.capacity_units = 1;
    }
    let first = fragment(&advertised);
    let first_plan_id = first.plan_id.clone();
    let mut second = fragment(&advertised);
    second.source_document_id = SourceDocumentId::from("second/source-document");
    let second = reseal_fragment(second);

    let mut runtime = HostRuntime::new(advertised, registry(), 64);
    assert!(runtime
        .handle(HostCommand::Prepare(first))
        .events
        .iter()
        .any(|event| matches!(event, HostEvent::Prepared { .. })));
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(second.clone()))),
        Some(FailureReason::ResourceCapacityExceeded)
    );

    let _ = runtime.handle(HostCommand::StartPlay(first_plan_id.clone()));
    assert!(runtime
        .handle(HostCommand::Cancel(first_plan_id.clone()))
        .events
        .iter()
        .any(|event| matches!(event, HostEvent::Cancelled { .. })));
    assert!(runtime
        .handle(HostCommand::Release(first_plan_id))
        .events
        .iter()
        .any(|event| matches!(event, HostEvent::Released { .. })));
    assert!(runtime
        .handle(HostCommand::Prepare(second))
        .events
        .iter()
        .any(|event| matches!(event, HostEvent::Prepared { .. })));
}

#[test]
fn authority_binding_mutations_change_fragment_identity() {
    let advertised = authority_advertisement();
    let grant = presentation_grant(&advertised);
    let original = authority_fragment(&advertised, std::slice::from_ref(&grant));

    for field in 0..7 {
        let mut mutated = original.clone();
        let binding = mutated
            .placements
            .iter_mut()
            .find_map(|placement| placement.authority.first_mut())
            .expect("authority binding exists");
        match field {
            0 => {
                binding.grant_id = conduit_core::AuthorityGrantId::from("mutated/grant");
            }
            1 => {
                binding.contract_id =
                    conduit_core::AuthorityContractId::from("mutated/authority@1");
            }
            2 => {
                binding.host_operation_contract_id =
                    HostOperationContractId::from("mutated/host-operation@1");
            }
            3 => binding.subject_kind = kind_id("mutated/subject"),
            4 => binding.host_id = HostId::from("mutated-host"),
            5 => binding.boot_id = BootId::from("mutated-boot"),
            6 => binding.capability_id = CapabilityId::from("mutated-capability"),
            _ => unreachable!(),
        }
        assert_post_identity_mutation_is_rejected(&advertised, mutated);
    }
}

#[test]
fn preparation_and_effect_admission_require_the_exact_current_authority_grant() {
    let advertised = authority_advertisement();
    let grant = presentation_grant(&advertised);
    let fragment = authority_fragment(&advertised, std::slice::from_ref(&grant));

    let mut runtime = HostRuntime::new(advertised.clone(), authority_registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(fragment.clone()))),
        Some(FailureReason::AuthorityDenied)
    );

    let mut runtime = HostRuntime::new_with_authority_grants(
        advertised.clone(),
        registry(),
        64,
        vec![grant.clone()],
    );
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(fragment.clone()))),
        Some(FailureReason::AuthorityContractMismatch)
    );

    let mut conflicting = grant.clone();
    conflicting.subject_kind = kind_id("conflicting/subject");
    let mut runtime = HostRuntime::new_with_authority_grants(
        advertised.clone(),
        authority_registry(),
        64,
        vec![grant.clone(), conflicting],
    );
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(fragment.clone()))),
        Some(FailureReason::AuthorityDenied)
    );

    let plan_id = fragment.plan_id.clone();
    let mut runtime = HostRuntime::new_with_authority_grants(
        advertised.clone(),
        authority_registry(),
        64,
        vec![grant.clone()],
    );
    assert!(runtime
        .handle(HostCommand::Prepare(fragment.clone()))
        .events
        .iter()
        .any(|event| matches!(event, HostEvent::Prepared { .. })));
    assert!(runtime
        .handle(HostCommand::StartPlay(plan_id))
        .effects
        .iter()
        .any(|effect| matches!(effect, PlatformEffect::PresentValue { .. })));

    let mut mutated = fragment.clone();
    mutated
        .placements
        .iter_mut()
        .find_map(|placement| placement.authority.first_mut())
        .expect("authority binding exists")
        .grant_id = conduit_core::AuthorityGrantId::from("mutated/grant");
    let mut runtime = HostRuntime::new_with_authority_grants(
        advertised.clone(),
        authority_registry(),
        64,
        vec![grant.clone()],
    );
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(reseal_fragment(mutated)))),
        Some(FailureReason::AuthorityDenied)
    );

    let mut mutated = fragment;
    mutated
        .placements
        .iter_mut()
        .find_map(|placement| placement.authority.first_mut())
        .expect("authority binding exists")
        .subject_kind = kind_id("mutated/subject");
    let mut runtime =
        HostRuntime::new_with_authority_grants(advertised, authority_registry(), 64, vec![grant]);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(reseal_fragment(mutated)))),
        Some(FailureReason::AuthorityContractMismatch)
    );
}

#[test]
fn effect_admission_rejects_a_planned_host_operation_outside_the_bound_grant_subject() {
    let advertised = ungranted_authority_advertisement();
    let grant = presentation_grant(&advertised);
    let fragment = authority_fragment(&advertised, std::slice::from_ref(&grant));
    let plan_id = fragment.plan_id.clone();
    let mut runtime = HostRuntime::new_with_authority_grants(
        advertised,
        ungranted_authority_registry(),
        64,
        vec![grant],
    );
    assert!(runtime
        .handle(HostCommand::Prepare(fragment))
        .events
        .iter()
        .any(|event| matches!(event, HostEvent::Prepared { .. })));
    let triggerd = runtime.handle(HostCommand::StartPlay(plan_id));
    assert!(triggerd.effects.is_empty());
    assert!(triggerd.events.iter().any(|event| matches!(
        event,
        HostEvent::PlacementTerminated {
            disposition: TerminalDisposition::Failed {
                reason: FailureReason::AuthorityDenied
            },
            ..
        }
    )));
}

#[test]
fn preparation_requires_the_exact_current_boot_scoped_link_observation() {
    let (source, fragment, link) = remote_link_source_fixture();

    let mut runtime = HostRuntime::new(source.clone(), adapter_registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(fragment.clone()))),
        Some(FailureReason::LinkUnavailable)
    );

    let mut stale = link.clone();
    stale.source.boot_id = BootId::from("stale-link-source-boot");
    let mut runtime = HostRuntime::new_with_external_state(
        source.clone(),
        adapter_registry(),
        64,
        vec![],
        vec![stale],
    );
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(fragment.clone()))),
        Some(FailureReason::LinkUnavailable)
    );

    let mut conflicting = link.clone();
    conflicting.base_instance_id = conduit_core::ConnectionBaseInstanceId::from("conflicting/base");
    let mut runtime = HostRuntime::new_with_external_state(
        source.clone(),
        adapter_registry(),
        64,
        vec![],
        vec![link.clone(), conflicting],
    );
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(fragment.clone()))),
        Some(FailureReason::LinkUnavailable)
    );

    let mut mutated = fragment.clone();
    mutated.connections[0]
        .link_binding
        .as_mut()
        .expect("link binding exists")
        .source
        .boot_id = BootId::from("resealed-wrong-boot");
    let mut runtime = HostRuntime::new_with_external_state(
        source.clone(),
        adapter_registry(),
        64,
        vec![],
        vec![link.clone()],
    );
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(reseal_fragment(mutated)))),
        Some(FailureReason::LinkBindingMismatch)
    );

    let mut runtime =
        HostRuntime::new_with_external_state(source, adapter_registry(), 64, vec![], vec![link]);
    assert!(runtime
        .handle(HostCommand::Prepare(fragment))
        .events
        .iter()
        .any(|event| matches!(event, HostEvent::Prepared { .. })));
}

#[test]
fn fake_browser_style_adapter_drives_effects_delay_disconnect_and_inspection() {
    let local_advertisement = adapter_advertisement();
    let local_fragment = fragment(&local_advertisement);
    let mut local_runtime = HostRuntime::new(local_advertisement, adapter_registry(), 128);
    local_runtime.handle(HostCommand::Prepare(local_fragment.clone()));
    let triggerd = local_runtime.handle(HostCommand::StartPlay(local_fragment.plan_id.clone()));
    let mut adapter = FakeBrowserStyleAdapter::default();
    adapter.receive(triggerd.effects);
    assert!(adapter.saw_wait);
    assert!(!adapter.saw_presentation);

    let waited = local_runtime.handle(HostCommand::CompleteWait {
        plan_id: local_fragment.plan_id.clone(),
        placement_id: local_fragment
            .placements
            .iter()
            .find(|placement| placement.kind_id.as_str() == SOURCE_KIND)
            .expect("source placement exists")
            .placement_id
            .clone(),
    });
    let presentation = waited
        .effects
        .iter()
        .find_map(|effect| match effect {
            PlatformEffect::PresentValue {
                plan_id,
                active_play_id,
                presentation_id,
                placement_id,
                value,
                ..
            } => Some((
                plan_id.clone(),
                active_play_id.clone(),
                presentation_id.clone(),
                placement_id.clone(),
                value.clone(),
            )),
            _ => None,
        })
        .expect("wait completion reaches presentation adapter");
    adapter.receive(waited.effects);
    assert!(adapter.saw_presentation);
    let oversized = local_runtime.handle(HostCommand::CompletePresentation {
        plan_id: presentation.0.clone(),
        active_play_id: presentation.1.clone(),
        presentation_id: presentation.2.clone(),
        placement_id: presentation.3.clone(),
        value: presentation.4.clone(),
        success: false,
        message: Some(
            "x".repeat(
                usize::try_from(conduit_core::MAX_PRESENTATION_COMPLETION_BYTES + 1)
                    .expect("test bound fits usize"),
            ),
        ),
    });
    assert!(oversized.events.iter().any(|event| matches!(
        event,
        HostEvent::CommandRejected {
            reason: FailureReason::HostOperationOutputExceeded,
            ..
        }
    )));
    let failed = local_runtime.handle(HostCommand::CompletePresentation {
        plan_id: presentation.0,
        active_play_id: presentation.1,
        presentation_id: presentation.2,
        placement_id: presentation.3,
        value: presentation.4,
        success: false,
        message: Some("injected adapter failure".into()),
    });
    assert!(failed.events.iter().any(|event| matches!(
        event,
        HostEvent::ManifestationFailed {
            reason: FailureReason::ManifestationFailed,
            ..
        }
    )));
    assert!(observations(&mut local_runtime).iter().any(|observation| {
        matches!(
            observation.kind,
            ObservationKind::Failure {
                reason: FailureReason::ManifestationFailed,
                ..
            }
        )
    }));

    let form = parse(
        "form 0\n\nadapter {\n source: contract/source\n sink: contract/sink\n source > sink\n}\n",
        &profile_catalog(),
    )
    .expect("adapter form parses");
    let mut source_advertisement = adapter_advertisement();
    source_advertisement.host_id = HostId::from("adapter-source-host");
    source_advertisement.boot_id = BootId::from("adapter-source-boot");
    source_advertisement.capabilities.truncate(1);
    let mut sink_advertisement = advertisement();
    sink_advertisement.host_id = HostId::from("adapter-sink-host");
    sink_advertisement.boot_id = BootId::from("adapter-sink-boot");
    sink_advertisement.capabilities.remove(0);
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("source"),
                PlacementChoice {
                    host_id: source_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from("pulse"),
                },
            ),
            (
                GearId::from("sink"),
                PlacementChoice {
                    host_id: sink_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from("show"),
                },
            ),
        ]),
    };
    let link_binding = process_owned_link_binding(
        "link/adapter",
        ConnectionBase::InMemory,
        "fixture/in-memory/adapter",
        &source_advertisement,
        &sink_advertisement,
        conduit_core::DEFAULT_CONNECTION_ITEM_CAPACITY,
        conduit_core::DEFAULT_CONNECTION_BYTE_CAPACITY,
    );
    let cross_host_plan = plan_with_link_bindings(
        &form,
        &[source_advertisement.clone(), sink_advertisement],
        &placements,
        &[ConnectionBase::Local, ConnectionBase::InMemory],
        conduit_core::DEFAULT_CONNECTION_ITEM_CAPACITY,
        conduit_core::DEFAULT_CONNECTION_BYTE_CAPACITY,
        std::slice::from_ref(&link_binding),
    )
    .expect("cross-host adapter plan resolves");
    let source_fragment = cross_host_plan
        .fragments
        .into_iter()
        .find(|fragment| fragment.host_id == source_advertisement.host_id)
        .expect("source fragment exists");
    let mut source_runtime = HostRuntime::new_with_external_state(
        source_advertisement,
        adapter_registry(),
        128,
        vec![],
        vec![link_binding],
    );
    source_runtime.handle(HostCommand::Prepare(source_fragment.clone()));
    let triggerd = source_runtime.handle(HostCommand::StartPlay(source_fragment.plan_id.clone()));
    let wait = triggerd
        .effects
        .into_iter()
        .find_map(|effect| match effect {
            PlatformEffect::Wait {
                plan_id,
                placement_id,
                ..
            } => Some((plan_id, placement_id)),
            _ => None,
        })
        .expect("outbound source waits through the adapter");
    let transmitted = source_runtime.handle(HostCommand::CompleteWait {
        plan_id: wait.0,
        placement_id: wait.1,
    });
    adapter.receive(transmitted.effects);
    let delayed = adapter
        .delayed_connection
        .take()
        .expect("adapter retains connection delivery until explicitly completed");
    assert!(!observations(&mut source_runtime)
        .iter()
        .any(|observation| matches!(observation.kind, ObservationKind::ConnectionTerminal { .. })));

    let disconnected = source_runtime.handle(HostCommand::CompleteConnectionDelivery {
        plan_id: delayed.plan_id.clone(),
        connection_id: delayed.connection_id.clone(),
        sequence: delayed.sequence,
        outcome: ConnectionOutcome::Disconnected,
    });
    assert!(disconnected.events.iter().any(|event| matches!(
        event,
        HostEvent::ConnectionTerminated { disposition, .. }
            if matches!(
                disposition.disposition,
                TerminalDisposition::Failed {
                    reason: FailureReason::ConnectionDisconnected
                }
            )
    )));
    assert!(observations(&mut source_runtime).iter().any(|observation| {
        matches!(
            &observation.kind,
            ObservationKind::ConnectionTerminal { disposition }
                if matches!(
                    disposition.disposition,
                    TerminalDisposition::Failed {
                        reason: FailureReason::ConnectionDisconnected
                    }
                )
        )
    }));
}
