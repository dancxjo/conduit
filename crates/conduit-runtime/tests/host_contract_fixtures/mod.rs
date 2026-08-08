pub use conduit_core::{
    authority_grant, kind_id, mandatory_evidence_storage_requirement, port_id,
    present_authority_requirement, present_host_operation_requirement, resource_offer,
    resource_requirement, seal_plan, wait_host_operation_requirement, ArtifactId, AuthorityGrant,
    BootId, CapabilityId, CapabilityLimits, CapabilityOffer, ConnectionProvider,
    ExecutionProfileId, FailureReason, FormIdentity, FragmentId, HostAdvertisement, HostCommand,
    HostEvent, HostId, HostProfileId, ImplementationId, KindContractRevision, OfferGeneration,
    PlanFragment, PlannedOperation, PortDescriptor, PortDirection, ValuePayload,
    PRESENTATION_RESOURCE_CLASS, PROTOCOL_VERSION, TIMER_RESOURCE_CLASS,
};
pub use conduit_form::{parse, KindDefinition, ProfileCatalog};
pub use conduit_planner::{default_placements, plan, plan_with_authority_grants};
pub use conduit_runtime::{
    HostRuntime, ImplementationFailure, ImplementationRegistry, OperationAction,
    OperationCompletion, OperationImplementation, OperationOutput, OperationState,
};

pub const SOURCE_KIND: &str = "contract/source";
pub const SINK_KIND: &str = "contract/sink";
pub const VALUE_KIND: &str = "contract/value";
pub const PRESENTATION_KIND: &str = "contract/presentation";
pub const UNGRANTED_PRESENTATION_KIND: &str = "contract/ungranted-presentation";
pub const SOURCE_CONTRACT: &str = "contract/source@1";
pub const SINK_CONTRACT: &str = "contract/sink@1";
pub const SOURCE_PROFILE: &str = "contract/source-hosted@1";
pub const SINK_PROFILE: &str = "contract/sink-hosted@1";

pub fn source_outputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id("value"),
        value_kind: kind_id(VALUE_KIND),
        direction: PortDirection::Output,
        temporal: conduit_core::PortTemporal::Value,
    }]
}

pub fn sink_inputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id("value"),
        value_kind: kind_id(VALUE_KIND),
        direction: PortDirection::Input,
        temporal: conduit_core::PortTemporal::Value,
    }]
}

pub fn profile_catalog() -> ProfileCatalog {
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

pub fn advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("contract-host"),
        boot_id: BootId::from("contract-boot"),
        offer_generation: OfferGeneration(4),
        profile: HostProfileId::from("contract-test"),
        resources: vec![
            resource_offer("contract/presentation", PRESENTATION_RESOURCE_CLASS, 2),
            resource_offer("contract/timer", TIMER_RESOURCE_CLASS, 2),
        ],
        planner_capabilities: vec![],
        capabilities: vec![
            CapabilityOffer {
                startup_parameters: vec![],
                shorthand: None,
                capability_id: CapabilityId::from("pulse"),
                kind_id: kind_id(SOURCE_KIND),
                kind_contract_revision: KindContractRevision::from(SOURCE_CONTRACT),
                execution_profile_id: ExecutionProfileId::from(SOURCE_PROFILE),
                implementation_id: ImplementationId::from("contract/source-v1"),
                artifact_id: ArtifactId::from("contract/source-artifact-v1"),
                inputs: vec![],
                outputs: source_outputs(),
                host_operations: vec![],
                resource_requirements: vec![],
                authority_requirements: vec![],
                limits: CapabilityLimits {
                    max_active_instances: 2,
                    max_queue_items: 4,
                    max_queue_bytes: 64,
                },
            },
            CapabilityOffer {
                startup_parameters: vec![],
                shorthand: None,
                capability_id: CapabilityId::from("show"),
                kind_id: kind_id(SINK_KIND),
                kind_contract_revision: KindContractRevision::from(SINK_CONTRACT),
                execution_profile_id: ExecutionProfileId::from(SINK_PROFILE),
                implementation_id: ImplementationId::from("contract/sink-v1"),
                artifact_id: ArtifactId::from("contract/sink-artifact-v1"),
                inputs: sink_inputs(),
                outputs: vec![],
                host_operations: vec![present_host_operation_requirement(
                    kind_id(PRESENTATION_KIND),
                    1,
                )],
                resource_requirements: vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)],
                authority_requirements: vec![],
                limits: CapabilityLimits {
                    max_active_instances: 2,
                    max_queue_items: 4,
                    max_queue_bytes: 64,
                },
            },
        ],
    }
}

pub fn fragment(advertisement: &HostAdvertisement) -> PlanFragment {
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

pub fn authority_advertisement() -> HostAdvertisement {
    let mut advertised = advertisement();
    advertised.capabilities[1].authority_requirements =
        vec![present_authority_requirement(kind_id(PRESENTATION_KIND))];
    advertised
}

pub fn presentation_grant(advertisement: &HostAdvertisement) -> AuthorityGrant {
    authority_grant(
        "grant/contract-presentation",
        &advertisement.capabilities[1].authority_requirements[0],
        advertisement.host_id.clone(),
        advertisement.boot_id.clone(),
        advertisement.capabilities[1].capability_id.clone(),
    )
}

pub fn authority_fragment(
    advertisement: &HostAdvertisement,
    grants: &[AuthorityGrant],
) -> PlanFragment {
    let form = parse(
        "form 0\n\ncontract {\n source: contract/source\n sink: contract/sink\n source > sink\n}\n",
        &profile_catalog(),
    )
    .expect("contract form parses");
    let placements = default_placements(&form, std::slice::from_ref(advertisement))
        .expect("default placements resolve without granting authority");
    plan_with_authority_grants(
        &form,
        std::slice::from_ref(advertisement),
        &placements,
        &[ConnectionProvider::Local],
        grants,
    )
    .expect("authority-bound contract plan resolves")
    .fragments
    .into_iter()
    .next()
    .expect("host fragment exists")
}

pub fn registry() -> ImplementationRegistry {
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

pub struct SourceImplementation {
    kind_id: conduit_core::KindId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
}

impl SourceImplementation {
    pub fn new(implementation_id: ImplementationId) -> Self {
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
        OperationAction::Emit(vec![OperationOutput {
            port: port_id("value"),
            value: ValuePayload {
                value_kind: kind_id(VALUE_KIND),
                encoded: vec![42],
            },
        }])
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

pub struct SinkImplementation {
    kind_id: conduit_core::KindId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
    maximum_presentation_bytes: u32,
    presentation_resource_units: u32,
    declares_presentation_authority: bool,
    additional_presentation_kind: Option<conduit_core::KindId>,
    requested_presentation_kind: conduit_core::KindId,
}

impl SinkImplementation {
    pub fn new(implementation_id: ImplementationId) -> Self {
        Self {
            kind_id: kind_id(SINK_KIND),
            implementation_id,
            artifact_id: ArtifactId::from("contract/sink-artifact-v1"),
            maximum_presentation_bytes: 1,
            presentation_resource_units: 1,
            declares_presentation_authority: false,
            additional_presentation_kind: None,
            requested_presentation_kind: kind_id(PRESENTATION_KIND),
        }
    }

    pub fn with_maximum_presentation_bytes(mut self, maximum: u32) -> Self {
        self.maximum_presentation_bytes = maximum;
        self
    }

    pub fn with_presentation_resource_units(mut self, units: u32) -> Self {
        self.presentation_resource_units = units;
        self
    }

    pub fn with_presentation_authority(mut self) -> Self {
        self.declares_presentation_authority = true;
        self
    }

    pub fn with_ungranted_presentation(mut self, presentation_kind: conduit_core::KindId) -> Self {
        self.additional_presentation_kind = Some(presentation_kind.clone());
        self.requested_presentation_kind = presentation_kind;
        self
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

    fn host_operation_requirements(&self) -> Vec<conduit_core::HostOperationRequirement> {
        let mut requirements = vec![present_host_operation_requirement(
            kind_id(PRESENTATION_KIND),
            self.maximum_presentation_bytes,
        )];
        if let Some(presentation_kind) = &self.additional_presentation_kind {
            requirements.push(present_host_operation_requirement(
                presentation_kind.clone(),
                self.maximum_presentation_bytes,
            ));
        }
        requirements.sort();
        requirements
    }

    fn resource_requirements(&self) -> Vec<conduit_core::ResourceRequirement> {
        (self.presentation_resource_units != 0)
            .then_some(resource_requirement(
                PRESENTATION_RESOURCE_CLASS,
                self.presentation_resource_units,
            ))
            .into_iter()
            .collect()
    }

    fn authority_requirements(&self) -> Vec<conduit_core::AuthorityRequirement> {
        self.declares_presentation_authority
            .then(|| present_authority_requirement(kind_id(PRESENTATION_KIND)))
            .into_iter()
            .collect()
    }

    fn prepare(
        &self,
        _placement: &PlannedOperation,
    ) -> Result<Box<dyn OperationState>, ImplementationFailure> {
        Ok(Box::new(SinkState {
            presentation_kind: self.requested_presentation_kind.clone(),
        }))
    }

    fn minimum_value_size(&self, value_kind: &conduit_core::KindId) -> Option<u32> {
        (value_kind.as_str() == VALUE_KIND).then_some(1)
    }
}

struct SinkState {
    presentation_kind: conduit_core::KindId,
}

impl OperationState for SinkState {
    fn start(&mut self) -> OperationAction {
        OperationAction::Idle
    }

    fn resume(&mut self, completion: OperationCompletion) -> OperationAction {
        match completion {
            OperationCompletion::Value { port, value } if port.as_str() == "value" => {
                OperationAction::Present {
                    presentation_kind: self.presentation_kind.clone(),
                    value,
                }
            }
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

pub fn rejection_reason(output: &conduit_runtime::RuntimeOutput) -> Option<FailureReason> {
    output.events.iter().find_map(|event| match event {
        HostEvent::PreparationRejected { reason, .. } => Some(*reason),
        _ => None,
    })
}

pub fn mandatory_evidence_reports(
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

pub fn reseal_fragment(mut fragment: PlanFragment) -> PlanFragment {
    fragment.plan_id = conduit_core::PlanId::from("");
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
