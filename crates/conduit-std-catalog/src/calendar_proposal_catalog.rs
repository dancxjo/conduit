//! Typed authoring contracts for finite, inert calendar meeting proposals.

use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, ConfigurationValue, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal, StructuredFieldType, StructuredFieldValue, StructuredInfoType,
    StructuredInfoTypeShape, StructuredInfoValue, StructuredVariantCase,
};
use conduit_form::{
    ConfigurationField, ConfigurationRule, KindDefinition, KindSignature, StartupParameterSignature,
};

pub const CALENDAR_PROPOSAL_REQUEST_TYPE: &str = "CalendarMeetingProposalRequest";
pub const CALENDAR_PROPOSAL_KIND: &str = "calendar/propose-meeting";
pub const CALENDAR_PROPOSAL_REVISION: &str = "calendar/propose-meeting@1";
pub const CALENDAR_PROPOSAL_MAXIMUM_PARTICIPANTS: u16 = 8;
pub const CALENDAR_PROPOSAL_MAXIMUM_INTERVALS: u16 = 8;
pub const CALENDAR_PROPOSAL_MAXIMUM_CANDIDATES: u16 = 8;
pub const CALENDAR_PROPOSAL_MAXIMUM_RESULTS: u16 = 3;

fn leaf(kind: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(kind)).expect("reviewed calendar leaf")
}

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).expect("reviewed calendar field")
}

fn record(kind: &str, fields: Vec<StructuredFieldType>) -> StructuredInfoType {
    StructuredInfoType::record(kind_id(kind), fields).expect("reviewed calendar record")
}

fn case(name: &str, value_type: StructuredInfoType) -> StructuredVariantCase {
    StructuredVariantCase::new(name, value_type).expect("reviewed calendar case")
}

fn slots(kind: &str, active: &str, payload: StructuredInfoType, maximum: u16) -> StructuredInfoType {
    let slot = StructuredInfoType::variant(
        kind_id(kind),
        vec![case(active, payload), case("unused", leaf("value/unit@1"))],
    )
    .expect("reviewed calendar slot");
    StructuredInfoType::collection(slot, Some(maximum)).expect("finite calendar slots")
}

pub fn calendar_instant_type() -> StructuredInfoType {
    record(
        "time/calendar-instant@1",
        vec![
            field("basis", leaf("value/text@1")),
            field("resolution_ticks", leaf("value/count@1")),
            field("scale", leaf("time/scale@1")),
            field("ticks", leaf("value/count@1")),
            field("uncertainty_ticks", leaf("value/count@1")),
        ],
    )
}

pub fn calendar_window_type() -> StructuredInfoType {
    record(
        "time/calendar-window@1",
        vec![
            field("end", calendar_instant_type()),
            field("start", calendar_instant_type()),
        ],
    )
}

pub fn calendar_candidate_type() -> StructuredInfoType {
    record(
        "calendar/meeting-candidate@1",
        vec![
            field("identity", leaf("value/text@1")),
            field("interval", calendar_window_type()),
            field("rationale", leaf("value/text@1")),
        ],
    )
}

pub fn calendar_availability_interval_type() -> StructuredInfoType {
    record(
        "calendar/availability-interval@1",
        vec![
            field("interval", calendar_window_type()),
            field("participant_identity", leaf("value/text@1")),
            field("state", leaf("calendar/availability-state@1")),
        ],
    )
}

pub fn calendar_participant_availability_type() -> StructuredInfoType {
    record(
        "calendar/participant-availability@1",
        vec![
            field("basis_identity", leaf("value/text@1")),
            field(
                "intervals",
                slots(
                    "calendar/availability-interval-slot@1",
                    "interval",
                    calendar_availability_interval_type(),
                    CALENDAR_PROPOSAL_MAXIMUM_INTERVALS,
                ),
            ),
            field("observed_at", calendar_instant_type()),
            field("participant_identity", leaf("value/text@1")),
            field("usable_until", calendar_instant_type()),
            field("zone", leaf("value/text@1")),
            field("zone_rule_set", leaf("value/text@1")),
        ],
    )
}

pub fn calendar_proposal_request_type() -> StructuredInfoType {
    record(
        "calendar/meeting-proposal-request@1",
        vec![
            field(
                "availability",
                slots(
                    "calendar/participant-availability-slot@1",
                    "participant",
                    calendar_participant_availability_type(),
                    CALENDAR_PROPOSAL_MAXIMUM_PARTICIPANTS,
                ),
            ),
            field(
                "candidates",
                slots(
                    "calendar/meeting-candidate-slot@1",
                    "candidate",
                    calendar_candidate_type(),
                    CALENDAR_PROPOSAL_MAXIMUM_CANDIDATES,
                ),
            ),
            field("identity", leaf("value/text@1")),
            field("maximum_results", leaf("value/count@1")),
            field(
                "participant_identities",
                slots(
                    "calendar/participant-identity-slot@1",
                    "participant",
                    leaf("value/text@1"),
                    CALENDAR_PROPOSAL_MAXIMUM_PARTICIPANTS,
                ),
            ),
            field("reference_at", calendar_instant_type()),
        ],
    )
}

pub fn calendar_proposed_slot_type() -> StructuredInfoType {
    record(
        "calendar/proposed-meeting-slot@1",
        vec![
            field("candidate_identity", leaf("value/text@1")),
            field("interval", calendar_window_type()),
            field("rationale", leaf("value/text@1")),
            field(
                "tentative_participants",
                slots(
                    "calendar/tentative-participant-slot@1",
                    "participant",
                    leaf("value/text@1"),
                    CALENDAR_PROPOSAL_MAXIMUM_PARTICIPANTS,
                ),
            ),
        ],
    )
}

pub fn calendar_rejected_slot_type() -> StructuredInfoType {
    let conflict = record(
        "calendar/candidate-conflict@1",
        vec![
            field("participant_identity", leaf("value/text@1")),
            field("state", leaf("calendar/availability-state@1")),
        ],
    );
    record(
        "calendar/rejected-meeting-slot@1",
        vec![
            field("candidate_identity", leaf("value/text@1")),
            field(
                "conflicts",
                slots(
                    "calendar/candidate-conflict-slot@1",
                    "conflict",
                    conflict,
                    CALENDAR_PROPOSAL_MAXIMUM_PARTICIPANTS,
                ),
            ),
        ],
    )
}

pub fn calendar_proposal_result_type() -> StructuredInfoType {
    record(
        "calendar/meeting-proposal@1",
        vec![
            field(
                "availability_basis_identities",
                slots(
                    "calendar/availability-basis-slot@1",
                    "basis",
                    leaf("value/text@1"),
                    CALENDAR_PROPOSAL_MAXIMUM_PARTICIPANTS,
                ),
            ),
            field(
                "candidates",
                slots(
                    "calendar/proposed-meeting-slot-slot@1",
                    "candidate",
                    calendar_proposed_slot_type(),
                    CALENDAR_PROPOSAL_MAXIMUM_RESULTS,
                ),
            ),
            field("identity", leaf("value/text@1")),
            field("reference_at", calendar_instant_type()),
            field(
                "rejected",
                slots(
                    "calendar/rejected-meeting-slot-slot@1",
                    "rejected",
                    calendar_rejected_slot_type(),
                    CALENDAR_PROPOSAL_MAXIMUM_CANDIDATES,
                ),
            ),
        ],
    )
}

pub fn install_calendar_proposal_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    let request = calendar_proposal_request_type();
    startup
        .insert_structured_type(CALENDAR_PROPOSAL_REQUEST_TYPE, request.clone())
        .map_err(|error| error.to_string())?;
    startup
        .insert(KindSignature {
            kind: CALENDAR_PROPOSAL_KIND.into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "request".into(),
                value_type: CALENDAR_PROPOSAL_REQUEST_TYPE.into(),
                default: None,
            }],
        })
        .map_err(|error| error.to_string())?;
    let request_profile = request.profile().map_err(|error| format!("{error:?}"))?;
    profile
        .insert(KindDefinition {
            kind_id: kind_id(CALENDAR_PROPOSAL_KIND),
            kind_contract_revision: KindContractRevision::from(CALENDAR_PROPOSAL_REVISION),
            inputs: vec![],
            outputs: vec![PortDescriptor {
                port_id: port_id("proposal"),
                value_kind: calendar_proposal_result_type()
                    .profile()
                    .map_err(|error| format!("{error:?}"))?
                    .value_kind()
                    .clone(),
                direction: PortDirection::Output,
                temporal: PortTemporal::Value,
            }],
            configuration: vec![ConfigurationField {
                key: "request".into(),
                default_value: ConfigurationValue::Structured(
                    conduit_core::StructuredConfigurationValue::new(
                        request_profile.value_kind().clone(),
                        default_calendar_proposal_request()?
                            .canonical_bytes()
                            .map_err(|error| format!("encode calendar request: {error:?}"))?,
                    )
                    .ok_or_else(|| "default calendar request is invalid".to_string())?,
                ),
                validation: ConfigurationRule::Structured {
                    profile: request_profile.value_kind().clone(),
                },
            }],
        })
        .map_err(|error| error.to_string())
}

pub fn default_calendar_proposal_request() -> Result<StructuredInfoValue, String> {
    let request_type = calendar_proposal_request_type();
    record_value(
        request_type.clone(),
        vec![
            (
                "availability",
                unused_collection(&request_type, "availability")?,
            ),
            ("candidates", unused_collection(&request_type, "candidates")?),
            ("identity", leaf_value("value/text@1", "proposal/default")?),
            ("maximum_results", leaf_value("value/count@1", "1")?),
            (
                "participant_identities",
                unused_collection(&request_type, "participant_identities")?,
            ),
            ("reference_at", instant_value(0)?),
        ],
    )
}

pub fn instant_value(ticks: u64) -> Result<StructuredInfoValue, String> {
    record_value(
        calendar_instant_type(),
        vec![
            ("basis", leaf_value("value/text@1", "utc")?),
            ("resolution_ticks", leaf_value("value/count@1", "1")?),
            ("scale", leaf_value("time/scale@1", "seconds")?),
            ("ticks", leaf_value("value/count@1", &ticks.to_string())?),
            ("uncertainty_ticks", leaf_value("value/count@1", "0")?),
        ],
    )
}

pub fn record_value(
    value_type: StructuredInfoType,
    fields: Vec<(&str, StructuredInfoValue)>,
) -> Result<StructuredInfoValue, String> {
    StructuredInfoValue::record(
        value_type,
        fields
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value).map_err(value_error))
            .collect::<Result<Vec<_>, _>>()?,
    )
    .map_err(value_error)
}

pub fn leaf_value(kind: &str, value: &str) -> Result<StructuredInfoValue, String> {
    StructuredInfoValue::leaf(leaf(kind), value.as_bytes().to_vec()).map_err(value_error)
}

pub fn unused_collection(
    record_type: &StructuredInfoType,
    name: &str,
) -> Result<StructuredInfoValue, String> {
    let StructuredInfoTypeShape::Record { fields, .. } = record_type.shape() else {
        return Err("calendar slots require a record".into());
    };
    let collection_type = fields
        .iter()
        .find(|field| field.name() == name)
        .ok_or_else(|| format!("calendar field '{name}' is missing"))?
        .value_type()
        .clone();
    let StructuredInfoTypeShape::Collection { element, length } = collection_type.shape() else {
        return Err("calendar slot field is not a collection".into());
    };
    let values = (0..length)
        .map(|_| {
            StructuredInfoValue::variant(
                element.clone(),
                "unused",
                leaf_value("value/unit@1", "")?,
            )
            .map_err(value_error)
        })
        .collect::<Result<Vec<_>, String>>()?;
    StructuredInfoValue::collection(collection_type, values).map_err(value_error)
}

fn value_error(error: conduit_core::StructuredInfoRefusal) -> String {
    format!("calendar structured Info refusal: {error:?}")
}
