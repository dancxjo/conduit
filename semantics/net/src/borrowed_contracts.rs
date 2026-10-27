//! The semantic owner's complete canonical definitions, shared by owned and ROM paths.
use super::*;
use conduit_core::{
    KindDefinitionRef, KindSemanticContract, PortDescriptorRef, PortDescriptorRefs,
};
static JOIN_INPUTS: [PortDescriptorRef<'static>; 1] = [PortDescriptorRef {
    port_id: "request",
    value_kind: NETWORK_JOIN_REQUEST_KIND,
    direction: PortDirection::Input,
    temporal: PortTemporal::Value,
    abnormal_kind: None,
}];
static JOIN_OUTPUTS: [PortDescriptorRef<'static>; 1] = [PortDescriptorRef {
    port_id: "attachment",
    value_kind: NETWORK_ATTACHMENT_KIND,
    direction: PortDirection::Output,
    temporal: PortTemporal::Value,
    abnormal_kind: None,
}];
static CREDENTIAL_OUTPUTS: [PortDescriptorRef<'static>; 1] = [PortDescriptorRef {
    port_id: "request",
    value_kind: NETWORK_JOIN_REQUEST_KIND,
    direction: PortDirection::Output,
    temporal: PortTemporal::Value,
    abnormal_kind: None,
}];
static ATTACHMENT_INPUTS: [PortDescriptorRef<'static>; 1] = [PortDescriptorRef {
    port_id: "attachment",
    value_kind: NETWORK_ATTACHMENT_KIND,
    direction: PortDirection::Input,
    temporal: PortTemporal::Value,
    abnormal_kind: None,
}];
static JOIN_LIMITS: CapabilityLimits = CapabilityLimits {
    max_active_instances: 1,
    max_queue_items: 1,
    max_queue_bytes: MAXIMUM_JOIN_INPUT_BYTES,
};
static SIGN_LIMITS: CapabilityLimits = CapabilityLimits {
    max_active_instances: 1,
    max_queue_items: 1,
    max_queue_bytes: MAXIMUM_JOIN_OUTPUT_BYTES,
};
pub fn network_join_contract_ref() -> KindDefinitionRef<'static> {
    KindDefinitionRef {
        startup_parameters: &[],
        shorthand: Some(("request", "attachment")),
        kind_id: NETWORK_JOIN_OPERATION,
        kind_contract_revision: NETWORK_JOIN_CONTRACT_REVISION,
        inputs: PortDescriptorRefs::Borrowed(&JOIN_INPUTS),
        outputs: PortDescriptorRefs::Borrowed(&JOIN_OUTPUTS),
        configuration: &[],
        semantic_laws: &[],
        limits: &JOIN_LIMITS,
    }
}
pub fn network_credentials_contract_ref() -> KindDefinitionRef<'static> {
    KindDefinitionRef {
        startup_parameters: &[],
        shorthand: None,
        kind_id: NETWORK_CREDENTIALS_OPERATION,
        kind_contract_revision: NETWORK_CREDENTIALS_CONTRACT_REVISION,
        inputs: PortDescriptorRefs::Borrowed(&[]),
        outputs: PortDescriptorRefs::Borrowed(&CREDENTIAL_OUTPUTS),
        configuration: &[],
        semantic_laws: &[],
        limits: &JOIN_LIMITS,
    }
}
pub fn network_attachment_sign_contract_ref() -> KindDefinitionRef<'static> {
    KindDefinitionRef {
        startup_parameters: &[],
        shorthand: None,
        kind_id: NETWORK_ATTACHMENT_SIGN_OPERATION,
        kind_contract_revision: NETWORK_ATTACHMENT_SIGN_CONTRACT_REVISION,
        inputs: PortDescriptorRefs::Borrowed(&ATTACHMENT_INPUTS),
        outputs: PortDescriptorRefs::Borrowed(&[]),
        configuration: &[],
        semantic_laws: &[],
        limits: &SIGN_LIMITS,
    }
}

static EMPTY_SEMANTIC_CONTRACT: KindSemanticContract = KindSemanticContract {
    configuration: alloc::vec::Vec::new(),
    laws: alloc::vec::Vec::new(),
};
static JOIN_CALLS: [conduit_core::HostCallRequirementRef<'static>; 1] =
    [conduit_core::HostCallRequirementRef {
        contract_id: NETWORK_JOIN_HOST_CALL,
        target_kind: Some(NETWORK_CONFIG_SUBJECT),
        maximum_in_flight: 1,
        maximum_input_bytes: MAXIMUM_JOIN_INPUT_BYTES,
        maximum_output_bytes: MAXIMUM_JOIN_OUTPUT_BYTES,
    }];
static CREDENTIAL_CALLS: [conduit_core::HostCallRequirementRef<'static>; 1] =
    [conduit_core::HostCallRequirementRef {
        contract_id: NETWORK_CREDENTIALS_HOST_CALL,
        target_kind: Some(NETWORK_CREDENTIALS_SUBJECT),
        maximum_in_flight: 1,
        maximum_input_bytes: 1,
        maximum_output_bytes: MAXIMUM_JOIN_INPUT_BYTES,
    }];
static SIGN_CALLS: [conduit_core::HostCallRequirementRef<'static>; 1] =
    [conduit_core::HostCallRequirementRef {
        contract_id: NETWORK_ATTACHMENT_SIGN_HOST_CALL,
        target_kind: None,
        maximum_in_flight: 1,
        maximum_input_bytes: MAXIMUM_JOIN_OUTPUT_BYTES,
        maximum_output_bytes: 1,
    }];
static JOIN_RESOURCES: [conduit_core::ResourceRequirementRef<'static>; 1] =
    [conduit_core::ResourceRequirementRef {
        class_id: WIFI_STATION_RESOURCE_CLASS,
        units: 1,
        protected_role: None,
        compute: None,
        content: None,
    }];
static JOIN_AUTHORITY: [conduit_core::AuthorityRequirementRef<'static>; 1] =
    [conduit_core::AuthorityRequirementRef {
        contract_id: NETWORK_CONFIG_AUTHORITY,
        host_call_contract_id: NETWORK_JOIN_HOST_CALL,
        subject_kind: NETWORK_CONFIG_SUBJECT,
    }];
static CREDENTIAL_AUTHORITY: [conduit_core::AuthorityRequirementRef<'static>; 1] =
    [conduit_core::AuthorityRequirementRef {
        contract_id: NETWORK_CREDENTIALS_AUTHORITY,
        host_call_contract_id: NETWORK_CREDENTIALS_HOST_CALL,
        subject_kind: NETWORK_CREDENTIALS_SUBJECT,
    }];
#[derive(Clone, Copy)]
pub(super) enum Shape {
    Join,
    Credentials,
    AttachmentLocal,
    AttachmentUsb,
}
impl Shape {
    fn kind(self) -> KindDefinitionRef<'static> {
        match self {
            Self::Join => network_join_contract_ref(),
            Self::Credentials => network_credentials_contract_ref(),
            Self::AttachmentLocal | Self::AttachmentUsb => network_attachment_sign_contract_ref(),
        }
    }
    fn profile(self) -> &'static str {
        match self {
            Self::Join => NETWORK_JOIN_EXECUTION_PROFILE,
            Self::Credentials => "conduit.network/credentials-hosted@1",
            Self::AttachmentLocal => NETWORK_ATTACHMENT_LOCAL_PROFILE,
            Self::AttachmentUsb => "conduit.network/attachment-sign-usb@1",
        }
    }
    fn calls(self) -> &'static [conduit_core::HostCallRequirementRef<'static>] {
        match self {
            Self::Join => &JOIN_CALLS,
            Self::Credentials => &CREDENTIAL_CALLS,
            _ => &SIGN_CALLS,
        }
    }
    fn resources(self) -> &'static [conduit_core::ResourceRequirementRef<'static>] {
        match self {
            Self::Join => &JOIN_RESOURCES,
            _ => &[],
        }
    }
    fn authority(self) -> &'static [conduit_core::AuthorityRequirementRef<'static>] {
        match self {
            Self::Join => &JOIN_AUTHORITY,
            Self::Credentials => &CREDENTIAL_AUTHORITY,
            _ => &[],
        }
    }
}
/// Complete canonical offer checked by the ordinary Kind/Back factory. This is
/// descriptive metadata, never readiness, Source proof, a grant or possession.
/// Its actual identities stay borrowed and cannot be rebound after checking.
pub struct PreparedCanonicalNetworkOffer<'a> {
    shape: Shape,
    capability: &'a str,
    implementation: &'a str,
    artifact: &'a str,
}
impl<'a> PreparedCanonicalNetworkOffer<'a> {
    pub fn join(capability: &'a str, implementation: &'a str, artifact: &'a str) -> Self {
        Self::prepare(Shape::Join, capability, implementation, artifact)
    }
    pub fn credentials(capability: &'a str, implementation: &'a str, artifact: &'a str) -> Self {
        Self::prepare(Shape::Credentials, capability, implementation, artifact)
    }
    pub fn attachment_local(
        capability: &'a str,
        implementation: &'a str,
        artifact: &'a str,
    ) -> Self {
        Self::prepare(Shape::AttachmentLocal, capability, implementation, artifact)
    }
    fn prepare(
        shape: Shape,
        capability: &'a str,
        implementation: &'a str,
        artifact: &'a str,
    ) -> Self {
        // Preserve the complete existing validator, in this measured construction
        // phase. Retire all owning metadata before publishing the immutable view.
        drop(materialize(
            shape,
            capability.into(),
            implementation.into(),
            artifact.into(),
        ));
        Self {
            shape,
            capability,
            implementation,
            artifact,
        }
    }
    pub fn view(&self) -> conduit_core::CapabilityOfferRef<'a> {
        use conduit_core::*;
        let k = self.shape.kind();
        CapabilityOfferRef {
            startup_parameters: k.startup_parameters,
            shorthand: k.shorthand,
            capability_id: self.capability,
            kind_id: k.kind_id,
            kind_contract_revision: k.kind_contract_revision,
            inputs: k.inputs,
            outputs: k.outputs,
            semantic_contract: &EMPTY_SEMANTIC_CONTRACT,
            execution_profile_id: self.shape.profile(),
            implementation_id: self.implementation,
            artifact_id: self.artifact,
            state_retention: None,
            realization_properties: &[],
            host_calls: HostCallRequirementRefs::Borrowed(self.shape.calls()),
            resource_requirements: ResourceRequirementRefs::Borrowed(self.shape.resources()),
            authority_requirements: AuthorityRequirementRefs::Borrowed(self.shape.authority()),
            limits: k.limits,
        }
    }
}
pub(super) fn materialize(
    shape: Shape,
    capability_id: CapabilityId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
) -> CapabilityOffer {
    BackOfferBuilder::new(
        shape.kind().materialize(),
        Back {
            capability_id,
            implementation_id,
            artifact_id,
            execution_profile_id: shape.profile().into(),
            host_calls: shape
                .calls()
                .iter()
                .map(|r| HostCallRequirement {
                    contract_id: r.contract_id.into(),
                    target_kind: r.target_kind.map(Into::into),
                    maximum_in_flight: r.maximum_in_flight,
                    maximum_input_bytes: r.maximum_input_bytes,
                    maximum_output_bytes: r.maximum_output_bytes,
                })
                .collect(),
            resource_requirements: shape
                .resources()
                .iter()
                .map(|r| conduit_core::ResourceRequirement {
                    class_id: r.class_id.into(),
                    units: r.units,
                    protected_role: r.protected_role.map(Into::into),
                    compute: r.compute.cloned(),
                    content: r.content.cloned(),
                })
                .collect(),
            authority_requirements: shape
                .authority()
                .iter()
                .map(|r| AuthorityRequirement {
                    contract_id: r.contract_id.into(),
                    host_call_contract_id: r.host_call_contract_id.into(),
                    subject_kind: r.subject_kind.into(),
                })
                .collect(),
        },
    )
    .build()
}
/// Canonical immutable Kind/Back identity, minted only after ordinary complete
/// materialization/validation with the actual owner identities. No readiness,
/// grant, Source selection or native identity is carried by this token.
pub struct CheckedNetworkContract {
    shape: Shape,
}
impl PreparedCanonicalNetworkOffer<'_> {
    pub fn into_contract(self) -> CheckedNetworkContract {
        CheckedNetworkContract { shape: self.shape }
    }
}
impl CheckedNetworkContract {
    pub fn kind(&self) -> KindDefinitionRef<'static> {
        self.shape.kind()
    }
    pub fn semantic_contract(&self) -> &'static KindSemanticContract {
        &EMPTY_SEMANTIC_CONTRACT
    }
    pub fn offer<'a>(
        &self,
        capability: &'a str,
        implementation: &'a str,
        artifact: &'a str,
    ) -> conduit_core::CapabilityOfferRef<'a> {
        PreparedCanonicalNetworkOffer {
            shape: self.shape,
            capability,
            implementation,
            artifact,
        }
        .view()
    }
}
