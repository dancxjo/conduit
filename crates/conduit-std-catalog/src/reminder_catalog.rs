//! Canonical Form catalog and exact effect seam for reminder delivery.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    StructuredFieldType, StructuredInfoType,
};
use conduit_form::{KindDefinition, KindSignature};

pub const REMINDER_OCCURRENCE_TYPE: &str = "ReminderOccurrence";
pub const REMINDER_FIXTURE_KIND: &str = "notification/deterministic-reminder";
pub const REMINDER_DELIVER_KIND: &str = "notification/deliver-reminder";
pub const REMINDER_REVISION: &str = "conduit.std/reminder-delivery@1";
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
