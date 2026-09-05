//! Ordinary Form-facing contract for typed hysteresis decisions.

use alloc::{string::ToString, vec};
use conduit_core::{
    kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    StructuredInfoType,
};
use conduit_form::{KindDefinition, KindSignature, ProfileCatalog, StartupCatalog};

use crate::{
    measurement_summary_type, MEASUREMENT_THRESHOLD_DECISION_INFO_ID,
    MEASUREMENT_THRESHOLD_POLICY_INFO_ID,
};

pub const MEASUREMENT_HYSTERESIS_KIND: &str = "data/measurement-hysteresis";
pub const MEASUREMENT_HYSTERESIS_CONTRACT_REVISION: &str = "conduit.data/measurement-hysteresis@1";

pub fn install_measurement_threshold_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup
        .insert_structured_type(
            "MeasurementThresholdPolicy",
            measurement_threshold_policy_type(),
        )
        .map_err(|error| error.to_string())?;
    startup
        .insert_structured_type(
            "MeasurementThresholdDecision",
            measurement_threshold_decision_type(),
        )
        .map_err(|error| error.to_string())?;
    startup.insert(KindSignature {
        kind: MEASUREMENT_HYSTERESIS_KIND.to_string(),
        startup_parameters: vec![],
    })?;
    profile
        .insert(measurement_hysteresis_kind_definition())
        .map_err(|error| error.to_string())
}

pub fn measurement_hysteresis_kind_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(MEASUREMENT_HYSTERESIS_KIND),
        kind_contract_revision: KindContractRevision::from(
            MEASUREMENT_HYSTERESIS_CONTRACT_REVISION,
        ),
        inputs: vec![
            port("summary", &measurement_summary_type(), PortDirection::Input),
            port(
                "policy",
                &measurement_threshold_policy_type(),
                PortDirection::Input,
            ),
        ],
        outputs: vec![port(
            "decision",
            &measurement_threshold_decision_type(),
            PortDirection::Output,
        )],
        configuration: vec![],
    }
}

pub fn measurement_threshold_policy_type() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(MEASUREMENT_THRESHOLD_POLICY_INFO_ID))
        .expect("the measurement threshold policy leaf identity is finite")
}

pub fn measurement_threshold_decision_type() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(MEASUREMENT_THRESHOLD_DECISION_INFO_ID))
        .expect("the measurement threshold decision leaf identity is finite")
}

fn port(name: &str, value_type: &StructuredInfoType, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type.profile().unwrap().value_kind().clone(),
        direction,
        temporal: PortTemporal::Value,
    }
}
