//! Exact planned realization contracts for the hosted Google adapter.

use conduit_core::{
    kind_id, resource_offer, resource_requirement, ArtifactId, AuthorityContractId, AuthorityGrant,
    AuthorityGrantId, AuthorityRequirement, BootId, CapabilityId, CapabilityLimits,
    CapabilityOffer, ExecutionProfileId, HostId, HostOperationContractId, HostOperationRequirement,
    ImplementationId, ImplementationOffer, ResourceOffer,
};

pub const GOOGLE_CALENDAR_RESOURCE_CLASS: &str = "conduit.resource/calendar/google-account@1";
pub const GOOGLE_CALENDAR_RESOURCE_ID: &str = "std/google-calendar-account";
const GOOGLE_CALENDAR_MAXIMUM_ACTIVE_OPERATIONS: u32 = 6;
const PROFILE: &str = "std/google-calendar-bounded@1";
const ARTIFACT: &str = "conduit-std-host/google-calendar@1";

pub const READ_OPERATION: &str = "conduit.host/calendar-read@1";
pub const FREE_BUSY_OPERATION: &str = "conduit.host/calendar-free-busy@1";
pub const CREATE_OPERATION: &str = "conduit.host/calendar-create@1";
pub const UPDATE_OPERATION: &str = "conduit.host/calendar-update@1";
pub const CANCEL_OPERATION: &str = "conduit.host/calendar-cancel@1";
pub const INVITE_OPERATION: &str = "conduit.host/calendar-invite@1";

pub const READ_AUTHORITY: &str = "conduit.authority/calendar-read@1";
pub const FREE_BUSY_AUTHORITY: &str = "conduit.authority/calendar-free-busy@1";
pub const CREATE_AUTHORITY: &str = "conduit.authority/calendar-create@1";
pub const UPDATE_CANCEL_AUTHORITY: &str = "conduit.authority/calendar-update-cancel@1";
pub const INVITE_AUTHORITY: &str = "conduit.authority/calendar-participant-invitation@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarHostedOperation {
    Read,
    FreeBusy,
    Create,
    Update,
    Cancel,
    Invite,
}

impl CalendarHostedOperation {
    pub const fn contract(self) -> &'static str {
        match self {
            Self::Read => READ_OPERATION,
            Self::FreeBusy => FREE_BUSY_OPERATION,
            Self::Create => CREATE_OPERATION,
            Self::Update => UPDATE_OPERATION,
            Self::Cancel => CANCEL_OPERATION,
            Self::Invite => INVITE_OPERATION,
        }
    }

    pub const fn implementation(self) -> &'static str {
        match self {
            Self::Read => "std/kernel-calendar-read-google@1",
            Self::FreeBusy => "std/kernel-calendar-free-busy-google@1",
            Self::Create => "std/kernel-calendar-create-google@1",
            Self::Update => "std/kernel-calendar-update-google@1",
            Self::Cancel => "std/kernel-calendar-cancel-google@1",
            Self::Invite => "std/kernel-calendar-invite-google@1",
        }
    }

    pub const fn authority(self) -> &'static str {
        match self {
            Self::Read => READ_AUTHORITY,
            Self::FreeBusy => FREE_BUSY_AUTHORITY,
            Self::Create => CREATE_AUTHORITY,
            Self::Update | Self::Cancel => UPDATE_CANCEL_AUTHORITY,
            Self::Invite => INVITE_AUTHORITY,
        }
    }

    pub fn from_contract(contract: &str) -> Option<Self> {
        [
            Self::Read,
            Self::FreeBusy,
            Self::Create,
            Self::Update,
            Self::Cancel,
            Self::Invite,
        ]
        .into_iter()
        .find(|operation| operation.contract() == contract)
    }

    pub fn from_implementation(implementation: &str) -> Option<Self> {
        [
            Self::Read,
            Self::FreeBusy,
            Self::Create,
            Self::Update,
            Self::Cancel,
            Self::Invite,
        ]
        .into_iter()
        .find(|operation| operation.implementation() == implementation)
    }
}

pub fn google_calendar_offers() -> Vec<CapabilityOffer> {
    conduit_semantic_catalog::calendar_provider_contracts()
        .into_iter()
        .zip([
            CalendarHostedOperation::Read,
            CalendarHostedOperation::FreeBusy,
            CalendarHostedOperation::Create,
            CalendarHostedOperation::Update,
            CalendarHostedOperation::Cancel,
            CalendarHostedOperation::Invite,
        ])
        .map(|(contract, operation)| offer(contract, operation))
        .collect()
}

pub fn google_calendar_resource_offer() -> ResourceOffer {
    resource_offer(
        GOOGLE_CALENDAR_RESOURCE_ID,
        GOOGLE_CALENDAR_RESOURCE_CLASS,
        GOOGLE_CALENDAR_MAXIMUM_ACTIVE_OPERATIONS,
    )
}

pub fn google_calendar_authority_grant(
    offer: &CapabilityOffer,
    requirement_index: usize,
    grant_id: &str,
    host_id: &HostId,
    boot_id: &BootId,
) -> Result<AuthorityGrant, String> {
    let requirement = offer
        .authority_requirements
        .get(requirement_index)
        .ok_or_else(|| "calendar authority requirement index is absent".to_string())?;
    Ok(AuthorityGrant {
        grant_id: AuthorityGrantId::from(grant_id),
        contract_id: requirement.contract_id.clone(),
        host_operation_contract_id: requirement.host_operation_contract_id.clone(),
        subject_kind: requirement.subject_kind.clone(),
        host_id: host_id.clone(),
        boot_id: boot_id.clone(),
        capability_id: offer.capability_id.clone(),
    })
}

fn offer(
    contract: conduit_semantic_catalog::CalendarProviderKindContract,
    operation: CalendarHostedOperation,
) -> CapabilityOffer {
    let request = conduit_semantic_catalog::calendar_request_type(&contract);
    let subject_kind = request
        .profile()
        .expect("reviewed calendar request profile")
        .value_kind()
        .clone();
    let host_operation = HostOperationRequirement {
        contract_id: HostOperationContractId::from(operation.contract()),
        target_kind: Some(subject_kind.clone()),
        maximum_in_flight: 1,
        maximum_input_bytes: if contract.input_type.is_some() {
            conduit_semantic_catalog::CALENDAR_MAXIMUM_RESULT_BYTES
        } else {
            conduit_semantic_catalog::CALENDAR_MAXIMUM_SEMANTIC_JSON_BYTES
        },
        maximum_output_bytes: conduit_semantic_catalog::CALENDAR_MAXIMUM_RESULT_BYTES,
    };
    let mut authority_requirements = vec![authority(
        operation.authority(),
        &host_operation,
        subject_kind.clone(),
    )];
    if operation == CalendarHostedOperation::Invite {
        authority_requirements.insert(
            0,
            authority(
                UPDATE_CANCEL_AUTHORITY,
                &host_operation,
                subject_kind.clone(),
            ),
        );
    }
    let inputs = match (contract.input_type, contract.input_port) {
        (Some(value_type), Some(port)) => vec![port_descriptor(
            port,
            &value_type(),
            conduit_core::PortDirection::Input,
        )],
        (None, None) => Vec::new(),
        _ => unreachable!("reviewed calendar contract input shape"),
    };
    CapabilityOffer {
        startup_parameters: vec![conduit_core::FaceStartupParameter {
            name: "request".into(),
            value_type: contract.request_type_name.into(),
            has_default: false,
        }],
        shorthand: None,
        capability_id: CapabilityId::from(format!("google-{}", contract.kind.replace('/', "-"))),
        kind_id: kind_id(contract.kind),
        kind_contract_revision: conduit_core::KindContractRevision::from(contract.revision),
        inputs,
        outputs: vec![port_descriptor(
            contract.output_port,
            &(contract.output_type)(),
            conduit_core::PortDirection::Output,
        )],
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(operation.implementation()),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        host_operations: vec![host_operation],
        resource_requirements: vec![resource_requirement(GOOGLE_CALENDAR_RESOURCE_CLASS, 1)],
        authority_requirements,
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_semantic_catalog::CALENDAR_MAXIMUM_RESULT_BYTES,
        },
    }
}

fn authority(
    contract: &str,
    operation: &HostOperationRequirement,
    subject_kind: conduit_core::KindId,
) -> AuthorityRequirement {
    AuthorityRequirement {
        contract_id: AuthorityContractId::from(contract),
        host_operation_contract_id: operation.contract_id.clone(),
        subject_kind,
    }
}

fn port_descriptor(
    name: &str,
    value_type: &conduit_core::StructuredInfoType,
    direction: conduit_core::PortDirection,
) -> conduit_core::PortDescriptor {
    conduit_core::PortDescriptor {
        port_id: conduit_core::port_id(name),
        value_kind: value_type
            .profile()
            .expect("reviewed calendar Info profile")
            .value_kind()
            .clone(),
        direction,
        temporal: conduit_core::PortTemporal::Value,
    }
}
