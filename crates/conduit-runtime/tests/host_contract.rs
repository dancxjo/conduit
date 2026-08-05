use conduit_core::{
    kind_id, port_id, BootId, CapabilityId, CapabilityLimits, CapabilityOffer, FailureReason,
    FormId, HostAdvertisement, HostCommand, HostEvent, HostId, HostProfileId, ImplementationId,
    OfferGeneration, OperationId, PlacementId, PlanFragment, PlanId, PlannedOperation,
    PlatformEffect, PortDescriptor, PortDirection, TerminalDisposition, ValuePayload,
    PROTOCOL_VERSION,
};
use conduit_form::{parse, KindDefinition, ProfileCatalog};
use conduit_planner::{default_placements, plan};
use conduit_runtime::{
    HostRuntime, ImplementationFailure, ImplementationRegistry, OperationAction,
    OperationCompletion, OperationImplementation, OperationState,
};

const SOURCE_KIND: &str = "contract/source";
const SINK_KIND: &str = "contract/sink";
const VALUE_KIND: &str = "contract/value";
const PRESENTATION_KIND: &str = "contract/presentation";

fn profile_catalog() -> ProfileCatalog {
    let mut catalog = ProfileCatalog::new();
    catalog
        .insert(KindDefinition {
            kind_id: kind_id(SOURCE_KIND),
            inputs: Vec::new(),
            outputs: vec![PortDescriptor {
                port_id: port_id("value"),
                value_kind: kind_id(VALUE_KIND),
                direction: PortDirection::Output,
            }],
            configuration: Vec::new(),
        })
        .expect("source kind installs");
    catalog
        .insert(KindDefinition {
            kind_id: kind_id(SINK_KIND),
            inputs: vec![PortDescriptor {
                port_id: port_id("value"),
                value_kind: kind_id(VALUE_KIND),
                direction: PortDirection::Input,
            }],
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
                implementation_id: ImplementationId::from("contract/source-v1"),
                limits: CapabilityLimits {
                    value_kind: kind_id(VALUE_KIND),
                    max_active_instances: 2,
                    max_queue_items: 4,
                    max_queue_bytes: 64,
                },
            },
            CapabilityOffer {
                capability_id: CapabilityId::from("show"),
                kind_id: kind_id(SINK_KIND),
                implementation_id: ImplementationId::from("contract/sink-v1"),
                limits: CapabilityLimits {
                    value_kind: kind_id(VALUE_KIND),
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
}

impl SourceImplementation {
    fn new(implementation_id: ImplementationId) -> Self {
        Self {
            kind_id: kind_id(SOURCE_KIND),
            implementation_id,
        }
    }
}

impl OperationImplementation for SourceImplementation {
    fn kind_id(&self) -> &conduit_core::KindId {
        &self.kind_id
    }

    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation_id
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
}

impl SinkImplementation {
    fn new(implementation_id: ImplementationId) -> Self {
        Self {
            kind_id: kind_id(SINK_KIND),
            implementation_id,
        }
    }
}

impl OperationImplementation for SinkImplementation {
    fn kind_id(&self) -> &conduit_core::KindId {
        &self.kind_id
    }

    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation_id
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

    fn implementation_id(&self) -> &ImplementationId {
        self.0.implementation_id()
    }

    fn prepare(
        &self,
        placement: &PlannedOperation,
    ) -> Result<Box<dyn OperationState>, ImplementationFailure> {
        self.0.prepare(placement)
    }
}

#[test]
fn preparation_requires_capability_and_implementation_value_kind_agreement() {
    let advertised = advertisement();
    let planned = fragment(&advertised);

    let mut lying_capability = advertised.clone();
    lying_capability.capabilities[0].limits.value_kind = kind_id("contract/other-value");
    let mut runtime = HostRuntime::new(lying_capability, registry(), 64);
    assert_eq!(
        rejection_reason(&runtime.handle(HostCommand::Prepare(planned.clone()))),
        Some(FailureReason::UnsupportedValueKind)
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

struct FutureImplementation {
    kind_id: conduit_core::KindId,
    implementation_id: ImplementationId,
}

impl OperationImplementation for FutureImplementation {
    fn kind_id(&self) -> &conduit_core::KindId {
        &self.kind_id
    }

    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation_id
    }

    fn prepare(
        &self,
        _placement: &PlannedOperation,
    ) -> Result<Box<dyn OperationState>, ImplementationFailure> {
        Ok(Box::new(FutureState))
    }
}

struct FutureState;

impl OperationState for FutureState {
    fn start(&mut self) -> OperationAction {
        OperationAction::Complete
    }

    fn resume(&mut self, _completion: OperationCompletion) -> OperationAction {
        OperationAction::Complete
    }
}

#[test]
fn future_semantic_kind_runs_without_runtime_source_changes() {
    let future_kind_id = kind_id("future/operation");
    let implementation_id = ImplementationId::from("future/implementation-v1");
    let advertisement = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("future-host"),
        boot_id: BootId::from("future-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("future-test"),
        capabilities: vec![CapabilityOffer {
            capability_id: CapabilityId::from("future-capability"),
            kind_id: future_kind_id.clone(),
            implementation_id: implementation_id.clone(),
            limits: CapabilityLimits {
                value_kind: kind_id("value/none"),
                max_active_instances: 1,
                max_queue_items: 0,
                max_queue_bytes: 0,
            },
        }],
    };
    let placement_id = PlacementId::from("future-placement");
    let plan_id = PlanId::from("future-plan");
    let fragment = PlanFragment {
        plan_id: plan_id.clone(),
        form_id: FormId::from("future-form"),
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        offer_generation: advertisement.offer_generation,
        placements: vec![PlannedOperation {
            placement_id: placement_id.clone(),
            operation_id: OperationId::from("future"),
            kind_id: future_kind_id.clone(),
            configuration: Vec::new(),
            host_id: advertisement.host_id.clone(),
            boot_id: advertisement.boot_id.clone(),
            offer_generation: advertisement.offer_generation,
            capability_id: CapabilityId::from("future-capability"),
            implementation_id: implementation_id.clone(),
            inputs: Vec::new(),
            outputs: Vec::new(),
        }],
        connections: Vec::new(),
        startup_order: vec![placement_id],
    };
    let mut registry = ImplementationRegistry::new();
    registry
        .install(FutureImplementation {
            kind_id: future_kind_id,
            implementation_id,
        })
        .expect("future implementation installs");
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
