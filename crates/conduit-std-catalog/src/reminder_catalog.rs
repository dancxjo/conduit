//! Canonical Form catalog and exact effect seam for reminder delivery.

use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, ArtifactId, AuthorityContractId, AuthorityRequirement, CapabilityId,
    CapabilityLimits, CapabilityOffer, ExecutionProfileId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, ImplementationOffer, KindContractRevision,
    PortDescriptor, PortDirection, PortTemporal, StructuredFieldType, StructuredInfoType,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_form::{KindDefinition, KindSignature};

pub const REMINDER_OCCURRENCE_TYPE: &str = "ReminderOccurrence";
pub const REMINDER_FIXTURE_KIND: &str = "notification/deterministic-reminder";
pub const REMINDER_DELIVER_KIND: &str = "notification/deliver-reminder";
pub const REMINDER_REVISION: &str = "conduit.std/reminder-delivery@1";
pub const REMINDER_PROFILE: &str = "std/reminder-delivery-hosted@1";
pub const REMINDER_ARTIFACT: &str = "conduit-std-host/reminder-delivery@1";
pub const REMINDER_FIXTURE_OPERATION: &str = "conduit.host/reminder-fixture@1";
pub const REMINDER_DELIVER_OPERATION: &str = "conduit.host/reminder-delivery@1";
pub const REMINDER_DELIVERY_AUTHORITY: &str = "conduit.authority/deliver-reminder@1";

pub fn reminder_occurrence_type() -> StructuredInfoType {
    let text = StructuredInfoType::leaf(kind_id("value/text@1")).unwrap();
    StructuredInfoType::record(
        kind_id("notification/reminder-occurrence@1"),
        [
            "delivery_kind",
            "event_identity",
            "identity",
            "reminder_identity",
        ]
        .into_iter()
        .map(|name| StructuredFieldType::new(name, text.clone()).unwrap())
        .collect(),
    )
    .expect("reviewed reminder occurrence")
}

pub fn install_reminder_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    startup
        .insert_structured_type(REMINDER_OCCURRENCE_TYPE, reminder_occurrence_type())
        .map_err(|error| error.to_string())?;
    insert_kind(
        startup,
        profile,
        REMINDER_FIXTURE_KIND,
        vec![],
        vec![port("reminder", PortDirection::Output)],
    )?;
    insert_kind(
        startup,
        profile,
        REMINDER_DELIVER_KIND,
        vec![port("reminder", PortDirection::Input)],
        vec![],
    )
}

pub fn reminder_std_offers() -> Vec<CapabilityOffer> {
    vec![
        offer(
            REMINDER_FIXTURE_KIND,
            REMINDER_FIXTURE_OPERATION,
            vec![],
            vec![port("reminder", PortDirection::Output)],
            false,
        ),
        offer(
            REMINDER_DELIVER_KIND,
            REMINDER_DELIVER_OPERATION,
            vec![port("reminder", PortDirection::Input)],
            vec![],
            true,
        ),
    ]
}

fn insert_kind(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
    kind: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
) -> Result<(), String> {
    startup
        .insert(KindSignature {
            kind: kind.into(),
            startup_parameters: vec![],
        })
        .map_err(|error| error.to_string())?;
    profile
        .insert(KindDefinition {
            kind_id: kind_id(kind),
            kind_contract_revision: KindContractRevision::from(REMINDER_REVISION),
            inputs,
            outputs,
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}

fn port(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: reminder_occurrence_type()
            .profile()
            .unwrap()
            .value_kind()
            .clone(),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn offer(
    kind: &str,
    operation: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
    delivers: bool,
) -> CapabilityOffer {
    let operation = HostOperationRequirement {
        contract_id: HostOperationContractId::from(operation),
        target_kind: Some(kind_id(kind)),
        maximum_in_flight: 1,
        maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    };
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(format!("std/{kind}@1")),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(REMINDER_REVISION),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(REMINDER_PROFILE),
            implementation_id: ImplementationId::from(format!("std/{kind}@1")),
            artifact_id: ArtifactId::from(REMINDER_ARTIFACT),
        },
        inputs,
        outputs,
        host_operations: vec![operation.clone()],
        resource_requirements: vec![],
        authority_requirements: delivers
            .then(|| AuthorityRequirement {
                contract_id: AuthorityContractId::from(REMINDER_DELIVERY_AUTHORITY),
                host_operation_contract_id: operation.contract_id,
                subject_kind: kind_id(REMINDER_DELIVER_KIND),
            })
            .into_iter()
            .collect(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 4,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32,
        },
    }
}
