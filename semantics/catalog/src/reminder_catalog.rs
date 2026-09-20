//! Canonical Form catalog and exact effect seam for reminder delivery.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, Kind, KindIdentity, PortDescriptor, PortDirection,
    PortTemporal, StructuredFieldType, StructuredInfoType, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_form::{KindProjection, KindSignature};

pub const REMINDER_OCCURRENCE_TYPE: &str = "ReminderOccurrence";
pub const REMINDER_FIXTURE_KIND: &str = "notification/deterministic-reminder";
pub const REMINDER_DELIVER_KIND: &str = "notification/deliver-reminder";
pub const REMINDER_REVISION: &str = "conduit.std/reminder-delivery@1";
pub const REMINDER_DELIVERY_AUTHORITY: &str = "conduit.authority/deliver-reminder@1";

pub fn reminder_semantic_contracts() -> Vec<Kind> {
    vec![
        contract(
            REMINDER_FIXTURE_KIND,
            vec![],
            vec![port("reminder", PortDirection::Output)],
        ),
        contract(
            REMINDER_DELIVER_KIND,
            vec![port("reminder", PortDirection::Input)],
            vec![],
        ),
    ]
}

pub fn reminder_occurrence_type() -> StructuredInfoType {
    let text = StructuredInfoType::leaf(kind_id("value/text")).unwrap();
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
    for contract in reminder_semantic_contracts() {
        insert_kind(startup, profile, contract)?;
    }
    Ok(())
}

fn insert_kind(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
    contract: Kind,
) -> Result<(), String> {
    startup
        .insert(KindSignature {
            kind: contract.kind_id.as_str().into(),
            startup_parameters: vec![],
        })
        .map_err(|error| error.to_string())?;
    profile
        .insert(KindProjection {
            kind_id: contract.kind_id,
            kind_contract_revision: contract.kind_contract_revision,
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}

fn contract(kind: &str, inputs: Vec<PortDescriptor>, outputs: Vec<PortDescriptor>) -> Kind {
    Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id(kind),
        kind_contract_revision: KindIdentity::from(REMINDER_REVISION),
        inputs,
        outputs,
        configuration: Default::default(),
        semantic_laws: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 4,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32,
        },
    }
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
