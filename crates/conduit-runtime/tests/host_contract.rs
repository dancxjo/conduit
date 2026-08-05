use conduit_core::{
    kind_id, mandatory_evidence_storage_requirement, port_id, seal_plan, ArtifactId, BootId,
    CancellationPolicy, CapabilityId, CapabilityLimits, CapabilityOffer, CheckedFormId,
    ConnectionOutcome, ConnectionProvider, ExecutionProfileId, ExpandedFormId, ExpectedEvidence,
    ExpectedTerminal, FailureReason, FormIdentity, FragmentId, HostAdvertisement, HostCommand,
    HostEvent, HostId, HostProfileId, ImplementationId, KindContractRevision, ObservationKind,
    OfferGeneration, OperationId, PlacementId, PlanFragment, PlanId, PlannedOperation,
    PlatformEffect, PortDescriptor, PortDirection, SourceDocumentId, TerminalDisposition,
    TerminalPolicy, ValuePayload, PROTOCOL_VERSION,
};
use conduit_form::{parse, KindDefinition, ProfileCatalog};
use conduit_planner::{default_placements, plan, PlacementChoice, PlacementChoices};
use conduit_runtime::{
    HostRuntime, ImplementationFailure, ImplementationRegistry, OperationAction,
    OperationCompletion, OperationImplementation, OperationState,
};
use std::collections::BTreeMap;

const SOURCE_KIND: &str = "contract/source";
const SINK_KIND: &str = "contract/sink";
const VALUE_KIND: &str = "contract/value";
const PRESENTATION_KIND: &str = "contract/presentation";
const SOURCE_CONTRACT: &str = "contract/source@1";
const SINK_CONTRACT: &str = "contract/sink@1";
const SOURCE_PROFILE: &str = "contract/source-hosted@1";
const SINK_PROFILE: &str = "contract/sink-hosted@1";

fn source_outputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id("value"),
        value_kind: kind_id(VALUE_KIND),
        direction: PortDirection::Output,
    }]
}

fn sink_inputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id("value"),
        value_kind: kind_id(VALUE_KIND),
        direction: PortDirection::Input,
    }]
}

fn profile_catalog() -> ProfileCatalog {
    let mut catalog = ProfileCatalog::new();
    catalog
        .insert(KindDefinition {
            kind_id: kind_id(SOURCE_KIND),
            kind_contract_revision: KindContractRevision::from(SOURCE_CONTRACT),
            inputs: Vec::new(),
            outputs: source_outputs(),
            configuration: Vec::new(),
        })
        .expect("source kind installs");
    catalog
        .insert(KindDefinition {
            kind_id: kind_id(SINK_KIND),
            kind_contract_revision: KindContractRevision::from(SINK_CONTRACT),
            inputs: sink_inputs(),
            outputs: Vec::new(),
            configuration: Vec::new(),
        })
        .expect("sink kind installs");
    catalog
}

fn advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("contract-host"),
        boot_id: BootId::from("contract-boot"),
        offer_generation: OfferGeneration(4),
        profile: HostProfileId::from("contract-test"),
        capabilities: vec![
            CapabilityOffer {
                capability_id: CapabilityId::from("pulse"),
                kind_id: kind_id(SOURCE_KIND),
                kind_contract_revision: KindContractRevision::from(SOURCE_CONTRACT),
                execution_profile_id: ExecutionProfileId::from(SOURCE_PROFILE),
                implementation_id: ImplementationId::from("contract/source-v1"),
                artifact_id: ArtifactId::from("contract/source-artifact-v1"),
                inputs: vec![],
                outputs: source_outputs(),
                limits: CapabilityLimits {
                    max_active_instances: 2,
                    max_queue_items: 4,
                    max_queue_bytes: 64,
                },
            },
            CapabilityOffer {
                capability_id: CapabilityId::from("show"),
                kind_id: kind_id(SINK_KIND),
                kind_contract_revision: KindContractRevision::from(SINK_CONTRACT),
                execution_profile_id: ExecutionProfileId::from(SINK_PROFILE),
                implementation_id: ImplementationId::from("contract/sink-v1"),
                artifact_id: ArtifactId::from("contract/sink-artifact-v1"),
                inputs: sink_inputs(),
                outputs: vec![],
                limits: CapabilityLimits {
                    max_active_instances: 2,
                    max_queue_items: 4,
                    max_queue_bytes: 64,
                },
            },
        ],
    }
}

fn fragment(advertisement: &HostAdvertisement) -> PlanFragment {
    let form = parse(
        "form 0\n\ncontract {\n source: contract/source\n sink: contract/sink\n source > sink\n}\n",
        &profile_catalog(),
    )
    .expect("contract form parses");
    let placements = default_placements(&form, std::slice::from_ref(advertisement))
        .expect("default placements resolve");
    plan(
        &form,
        std::slice::from_ref(advertisement),
        &placements,
        &[conduit_core::ConnectionProvider::Local],
    )
    .expect("contract plan resolves")
    .fragments
    .into_iter()
    .next()
    .expect("host fragment exists")
}

fn registry() -> ImplementationRegistry {
    let mut registry = ImplementationRegistry::new();
    registry
        .install(SourceImplementation::new(ImplementationId::from(
            "contract/source-v1",
        )))
        .expect("source installs");
    registry
        .install(SinkImplementation::new(ImplementationId::from(
            "contract/sink-v1",
        )))
        .expect("sink installs");
    registry
}

struct SourceImplementation {
    kind_id: conduit_core::KindId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
}

impl SourceImplementation {
    fn new(implementation_id: ImplementationId) -> Self {
        Self {
            kind_id: kind_id(SOURCE_KIND),
            implementation_id,
            artifact_id: ArtifactId::from("contract/source-artifact-v1"),
        }
    }
}

impl OperationImplementation for SourceImplementation {
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

    fn prepare(
        &self,
        placement: &PlannedOperation,
    ) -> Result<Box<dyn OperationState>, ImplementationFailure> {
        if !placement.configuration.is_empty() {
            return Err(ImplementationFailure::new(
                FailureReason::InvalidOperationConfiguration,
                "contract source accepts no configuration",
            ));
        }
        Ok(Box::new(SourceState { emitted: false }))
    }

    fn minimum_value_size(&self, value_kind: &conduit_core::KindId) -> Option<u32> {
        (value_kind.as_str() == VALUE_KIND).then_some(1)
    }
}

struct SourceState {
    emitted: bool,
}

impl OperationState for SourceState {
    fn start(&mut self) -> OperationAction {
        OperationAction::Emit(ValuePayload {
            value_kind: kind_id(VALUE_KIND),
            encoded: vec![42],
        })
    }

    fn resume(&mut self, completion: OperationCompletion) -> OperationAction {
        if matches!(completion, OperationCompletion::Emitted) && !self.emitted {
            self.emitted = true;
            OperationAction::Complete
        } else {
            OperationAction::Fail(ImplementationFailure::new(
                FailureReason::InvalidLifecycleCommand,
                "unexpected source completion",
            ))
        }
    }
}

struct SinkImplementation {
    kind_id: conduit_core::KindId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
}

impl SinkImplementation {
    fn new(implementation_id: ImplementationId) -> Self {
        Self {
            kind_id: kind_id(SINK_KIND),
            implementation_id,
            artifact_id: ArtifactId::from("contract/sink-artifact-v1"),
        }
    }
}

impl OperationImplementation for SinkImplementation {
    fn kind_id(&self) -> &conduit_core::KindId {
        &self.kind_id
    }

    fn kind_contract_revision(&self) -> KindContractRevision {
        KindContractRevision::from(SINK_CONTRACT)
    }

    fn execution_profile_id(&self) -> ExecutionProfileId {
        ExecutionProfileId::from(SINK_PROFILE)
    }

    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation_id
    }

    fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    fn prepare(
        &self,
        _placement: &PlannedOperation,
    ) -> Result<Box<dyn OperationState>, ImplementationFailure> {
        Ok(Box::new(SinkState))
    }

    fn minimum_value_size(&self, value_kind: &conduit_core::KindId) -> Option<u32> {
        (value_kind.as_str() == VALUE_KIND).then_some(1)
    }
}

struct SinkState;

impl OperationState for SinkState {
    fn start(&mut self) -> OperationAction {
        OperationAction::Idle
    }

    fn resume(&mut self, completion: OperationCompletion) -> OperationAction {
        match completion {
            OperationCompletion::Value(value) => OperationAction::Present {
                presentation_kind: kind_id(PRESENTATION_KIND),
                value,
            },
            OperationCompletion::PresentationCompleted { success: true, .. } => {
                OperationAction::Idle
            }
            OperationCompletion::PresentationCompleted {
                success: false,
                message,
            } => OperationAction::Fail(ImplementationFailure {
                reason: FailureReason::ManifestationFailed,
                message,
            }),
            OperationCompletion::InputsClosed => OperationAction::Complete,
            _ => OperationAction::Fail(ImplementationFailure::new(
                FailureReason::InvalidLifecycleCommand,
                "unexpected sink completion",
            )),
        }
    }
}

fn rejection_reason(output: &conduit_runtime::RuntimeOutput) -> Option<FailureReason> {
    // Conformance deliberately discards human prose and compares only the stable reason enum.
    output.events.iter().find_map(|event| match event {
        HostEvent::PreparationRejected { reason, .. } => Some(*reason),
        _ => None,
    })
}

fn mandatory_evidence_reports(
    runtime: &mut HostRuntime,
) -> Vec<conduit_core::MandatoryEvidenceReport> {
    runtime
        .handle(HostCommand::Inspect)
        .events
        .into_iter()
        .find_map(|event| match event {
            HostEvent::MandatoryEvidenceReports { items } => Some(items),
            _ => None,
        })
        .expect("inspection returns mandatory evidence reports")
}

fn reseal_fragment(mut fragment: PlanFragment) -> PlanFragment {
    fragment.plan_id = PlanId::from("");
    fragment.fragment_id = FragmentId::from("");
    fragment.plan_fragments.clear();
    seal_plan(
        FormIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_form_id: fragment.checked_form_id.clone(),
            expanded_form_id: fragment.expanded_form_id.clone(),
        },
        vec![fragment],
    )
    .fragments
    .into_iter()
    .next()
    .expect("single-fragment plan reseals")
}

#[test]
fn prepare_is_effect_free_and_installed_profile_activates_generically() {
    let advertisement = advertisement();
    let fragment = fragment(&advertisement);
    let mut runtime = HostRuntime::new(advertisement, registry(), 128);
    let prepared = runtime.handle(HostCommand::Prepare(fragment.clone()));
    assert!(prepared.effects.is_empty());
    assert!(matches!(
        prepared.events.first(),
        Some(HostEvent::Prepared { .. })
    ));

    let activated = runtime.handle(HostCommand::Activate(fragment.plan_id));
    assert!(activated.effects.iter().any(|effect| matches!(
        effect,
        PlatformEffect::PresentValue {
            presentation_kind,
            ..
        } if presentation_kind.as_str() == PRESENTATION_KIND
    )));
}

#[test]
fn planned_evidence_storage_survives_observation_overflow() {
    let advertised = advertisement();
    let fragment = fragment(&advertised);
    let plan_id = fragment.plan_id.clone();
    let mut runtime = HostRuntime::new(advertised, registry(), 1);

    runtime.handle(HostCommand::Prepare(fragment.clone()));
    let prepared_report = mandatory_evidence_reports(&mut runtime)
        .into_iter()
        .next()
        .expect("prepared plan has a mandatory evidence report");
    assert_eq!(prepared_report.recorded, fragment.expected_evidence[..3]);
    assert!(!prepared_report.overflowed);
    let allocated_item_slots = prepared_report.allocated_item_slots;
    assert!(allocated_item_slots >= u32::from(fragment.evidence_storage_budget.item_capacity));

    let activated = runtime.handle(HostCommand::Activate(plan_id));
    let (plan_id, placement_id, value) = activated
        .effects
        .into_iter()
        .find_map(|effect| match effect {
            PlatformEffect::PresentValue {
                plan_id,
                placement_id,
                value,
                ..
            } => Some((plan_id, placement_id, value)),
            _ => None,
        })
        .expect("presentation effect exists");
    runtime.handle(HostCommand::CompletePresentation {
        plan_id,
        placement_id,
        value,
        success: true,
        message: None,
    });

    let completed_report = mandatory_evidence_reports(&mut runtime)
        .into_iter()
        .next()
        .expect("completed plan retains mandatory evidence");
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
        fragment.evidence_storage_budget
    );
    assert_eq!(
        completed_report.used_bytes,
        fragment.evidence_storage_budget.byte_capacity
    );
    assert_eq!(completed_report.allocated_item_slots, allocated_item_slots);
    assert!(!completed_report.overflowed);
    assert!(observations(&mut runtime)
        .iter()
        .any(|observation| matches!(observation.kind, ObservationKind::EvidenceGap { .. })));
}

#[test]
fn preparation_rejects_uninstalled_and_mismatched_implementations_structurally() {
    let mut missing_advertisement = advertisement();
    missing_advertisement.capabilities[1].implementation_id =
        ImplementationId::from("missing/sink-v1");
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
    let mut runtime = HostRuntime::new(advertised, registry(), 64);
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
    let mut runtime = HostRuntime::new(advertised, registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(invalid))),
        Some(FailureReason::InvalidOperationConfiguration)
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

    fn prepare(
        &self,
        placement: &PlannedOperation,
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
    mutated.connections[0].value_kind = kind_id("mutated/value");
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
    mutated.expected_evidence.pop();
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    mutated.evidence_storage_budget.item_capacity -= 1;
    assert_post_identity_mutation_is_rejected(&advertised, mutated);

    let mut mutated = original.clone();
    mutated.evidence_storage_budget.byte_capacity -= 1;
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
    let mut runtime = HostRuntime::new(advertised, registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(reseal_fragment(mutated)))),
        Some(FailureReason::PortContractMismatch)
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
    mutated.evidence_storage_budget.item_capacity -= 1;
    let mut runtime = HostRuntime::new(advertised.clone(), registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(reseal_fragment(mutated)))),
        Some(FailureReason::EvidenceBudgetExceeded)
    );

    let mut mutated = fragment(&advertised);
    mutated.evidence_storage_budget.byte_capacity -= 1;
    let mut runtime = HostRuntime::new(advertised, registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(reseal_fragment(mutated)))),
        Some(FailureReason::EvidenceBudgetExceeded)
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
        _placement: &PlannedOperation,
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
        capabilities: vec![CapabilityOffer {
            capability_id: CapabilityId::from("echo-capability"),
            kind_id: echo_kind_id.clone(),
            kind_contract_revision: KindContractRevision::from("test/echo@1"),
            execution_profile_id: ExecutionProfileId::from("test/echo-hosted@1"),
            implementation_id: implementation_id.clone(),
            artifact_id: ArtifactId::from("test/echo-artifact-v1"),
            inputs: vec![],
            outputs: vec![],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 0,
                max_queue_bytes: 0,
            },
        }],
    };
    let placement_id = PlacementId::from("echo-placement");
    let expected_evidence = vec![
        ExpectedEvidence::PlanFragmentReceived,
        ExpectedEvidence::PlacementPrepared(placement_id.clone()),
        ExpectedEvidence::PlacementTerminal(placement_id.clone()),
        ExpectedEvidence::PlanTerminal,
    ];
    let evidence_storage_budget = mandatory_evidence_storage_requirement(&expected_evidence)
        .expect("echo evidence fits budget types");
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
            placements: vec![PlannedOperation {
                placement_id: placement_id.clone(),
                operation_id: OperationId::from("echo"),
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
                inputs: Vec::new(),
                outputs: Vec::new(),
            }],
            connections: Vec::new(),
            startup_dependencies: Vec::new(),
            startup_order: vec![placement_id.clone()],
            cancellation_policy: CancellationPolicy::CancelAllAndRejectLateCompletion,
            terminal_policy: TerminalPolicy::RequireAllPlacementsAndConnections,
            expected_terminals: vec![
                ExpectedTerminal::PlacementCompleted(placement_id.clone()),
                ExpectedTerminal::PlanCompleted,
            ],
            expected_evidence,
            evidence_storage_budget,
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
    let activated = runtime.handle(HostCommand::Activate(plan_id));
    assert!(activated.events.iter().any(|event| matches!(
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
    let activated = runtime.handle(HostCommand::Activate(plan_id));
    let (plan_id, placement_id, value) = activated
        .effects
        .into_iter()
        .find_map(|effect| match effect {
            PlatformEffect::PresentValue {
                plan_id,
                placement_id,
                value,
                ..
            } => Some((plan_id, placement_id, value)),
            _ => None,
        })
        .expect("presentation effect exists");
    let failed = runtime.handle(HostCommand::CompletePresentation {
        plan_id,
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

    fn prepare(
        &self,
        _placement: &PlannedOperation,
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
                OperationAction::Emit(ValuePayload {
                    value_kind: kind_id(VALUE_KIND),
                    encoded: vec![7],
                })
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
        })
        .expect("adapter source installs");
    registry
        .install(SinkImplementation::new(ImplementationId::from(
            "contract/sink-v1",
        )))
        .expect("adapter sink installs");
    registry
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
fn fake_browser_style_adapter_drives_effects_delay_disconnect_and_inspection() {
    let local_advertisement = advertisement();
    let local_fragment = fragment(&local_advertisement);
    let mut local_runtime = HostRuntime::new(local_advertisement, adapter_registry(), 128);
    local_runtime.handle(HostCommand::Prepare(local_fragment.clone()));
    let activated = local_runtime.handle(HostCommand::Activate(local_fragment.plan_id.clone()));
    let mut adapter = FakeBrowserStyleAdapter::default();
    adapter.receive(activated.effects);
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
                placement_id,
                value,
                ..
            } => Some((plan_id.clone(), placement_id.clone(), value.clone())),
            _ => None,
        })
        .expect("wait completion reaches presentation adapter");
    adapter.receive(waited.effects);
    assert!(adapter.saw_presentation);
    let failed = local_runtime.handle(HostCommand::CompletePresentation {
        plan_id: presentation.0,
        placement_id: presentation.1,
        value: presentation.2,
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
    let mut source_advertisement = advertisement();
    source_advertisement.host_id = HostId::from("adapter-source-host");
    source_advertisement.boot_id = BootId::from("adapter-source-boot");
    source_advertisement.capabilities.truncate(1);
    let mut sink_advertisement = advertisement();
    sink_advertisement.host_id = HostId::from("adapter-sink-host");
    sink_advertisement.boot_id = BootId::from("adapter-sink-boot");
    sink_advertisement.capabilities.remove(0);
    let placements = PlacementChoices {
        by_operation: BTreeMap::from([
            (
                OperationId::from("source"),
                PlacementChoice {
                    host_id: source_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from("pulse"),
                },
            ),
            (
                OperationId::from("sink"),
                PlacementChoice {
                    host_id: sink_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from("show"),
                },
            ),
        ]),
    };
    let cross_host_plan = plan(
        &form,
        &[source_advertisement.clone(), sink_advertisement],
        &placements,
        &[ConnectionProvider::Local, ConnectionProvider::InMemory],
    )
    .expect("cross-host adapter plan resolves");
    let source_fragment = cross_host_plan
        .fragments
        .into_iter()
        .find(|fragment| fragment.host_id == source_advertisement.host_id)
        .expect("source fragment exists");
    let mut source_runtime = HostRuntime::new(source_advertisement, adapter_registry(), 128);
    source_runtime.handle(HostCommand::Prepare(source_fragment.clone()));
    let activated = source_runtime.handle(HostCommand::Activate(source_fragment.plan_id.clone()));
    let wait = activated
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
