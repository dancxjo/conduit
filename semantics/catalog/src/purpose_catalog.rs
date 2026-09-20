//! Portable typed seams for finite Body purpose and deterministic readiness.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, KindIdentity, PortDescriptor, PortDirection, PortTemporal,
    StructuredFieldType, StructuredInfoType, StructuredVariantCase,
};
use conduit_form::{KindDefinition, KindSignature};

pub const PURPOSE_READINESS_KIND: &str = "purpose/fulfillment-readiness";
pub const PURPOSE_STATE_TYPE: &str = "PurposeState";
pub const FULFILLMENT_READINESS_TYPE: &str = "FulfillmentReadiness";

fn text() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id("value/text")).expect("reviewed text")
}

fn count() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id("value/count")).expect("reviewed count")
}

fn unit() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id("value/unit")).expect("reviewed unit")
}

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).expect("reviewed purpose field")
}

fn record(kind: &str, fields: Vec<StructuredFieldType>) -> StructuredInfoType {
    StructuredInfoType::record(kind_id(kind), fields).expect("reviewed purpose record")
}

fn enumeration(kind: &str, names: &[&str]) -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id(kind),
        names
            .iter()
            .map(|name| StructuredVariantCase::new(*name, unit()).expect("reviewed case"))
            .collect(),
    )
    .expect("reviewed purpose enumeration")
}

pub fn purpose_state_type() -> StructuredInfoType {
    let obligation = record(
        "purpose/obligation@1",
        vec![
            field("obligation_identity", text()),
            field("summary", text()),
            field(
                "state",
                enumeration(
                    "purpose/obligation-state@1",
                    &[
                        "pending",
                        "satisfied",
                        "repair-required",
                        "evidence-missing",
                        "uncertain",
                        "disputed",
                    ],
                ),
            ),
            field(
                "evidence_sign_identities",
                StructuredInfoType::collection(text(), Some(8)).expect("bounded evidence"),
            ),
        ],
    );
    record(
        "purpose/state@1",
        vec![
            field("purpose_identity", text()),
            field("revision", count()),
            field("summary", text()),
            field(
                "obligations",
                StructuredInfoType::collection(obligation, Some(32)).expect("bounded obligations"),
            ),
        ],
    )
}

pub fn fulfillment_readiness_type() -> StructuredInfoType {
    record(
        "purpose/fulfillment-readiness@1",
        vec![
            field("purpose_identity", text()),
            field("purpose_revision", count()),
            field(
                "disposition",
                enumeration(
                    "purpose/readiness-disposition@1",
                    &["not-ready", "ready", "unavailable"],
                ),
            ),
            field(
                "reason_obligation_identities",
                StructuredInfoType::collection(text(), Some(32)).expect("bounded reasons"),
            ),
        ],
    )
}

pub fn install_purpose_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in [
        (PURPOSE_STATE_TYPE, purpose_state_type()),
        (FULFILLMENT_READINESS_TYPE, fulfillment_readiness_type()),
    ] {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|error| error.to_string())?;
    }
    startup
        .insert(KindSignature {
            kind: PURPOSE_READINESS_KIND.into(),
            startup_parameters: vec![],
        })
        .map_err(|error| error.to_string())?;
    profile
        .insert(KindDefinition {
            kind_id: kind_id(PURPOSE_READINESS_KIND),
            kind_contract_revision: KindIdentity::from("conduit.purpose/fulfillment-readiness@1"),
            inputs: vec![port("purpose", &purpose_state_type(), PortDirection::Input)],
            outputs: vec![port(
                "readiness",
                &fulfillment_readiness_type(),
                PortDirection::Output,
            )],
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}

fn port(name: &str, value_type: &StructuredInfoType, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type
            .profile()
            .expect("reviewed profile")
            .value_kind()
            .clone(),
        direction,
        temporal: PortTemporal::Current,
    }
}
