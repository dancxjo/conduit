//! Typed authoring and installed-Host contracts for finite recurrence expansion.

use alloc::{
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

pub const RECURRENCE_REQUEST_TYPE: &str = "RecurrenceExpansion";
pub const RECURRENCE_KIND: &str = "time/expand-recurrence";
pub const RECURRENCE_REVISION: &str = "time/expand-recurrence@1";
pub const RECURRENCE_STD_PROFILE: &str = "std/recurrence-kernel@1";
pub const RECURRENCE_STD_IMPLEMENTATION: &str = "std/kernel-expand-recurrence@1";
pub const RECURRENCE_STD_ARTIFACT: &str = "conduit-std-host/expand-recurrence@1";
pub const RECURRENCE_OCCURRENCE_KIND: &str = "time/recurrence-occurrence@1";
pub const RECURRENCE_RESULT_KIND: &str = "time/recurrence-expansion-result@1";
pub const RECURRENCE_MAXIMUM_RESULTS: u16 = 8;
pub const RECURRENCE_MAXIMUM_EXCEPTIONS: u16 = 4;
pub const RECURRENCE_MAXIMUM_RESOLUTIONS: u16 = 8;

fn leaf(kind: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(kind)).unwrap()
}

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).unwrap()
}

fn record(kind: &str, fields: Vec<StructuredFieldType>) -> StructuredInfoType {
    StructuredInfoType::record(kind_id(kind), fields).unwrap()
}

fn case(name: &str, value_type: StructuredInfoType) -> StructuredVariantCase {
    StructuredVariantCase::new(name, value_type).unwrap()
}

fn bounded(value_type: StructuredInfoType, maximum: u16) -> StructuredInfoType {
    StructuredInfoType::collection(value_type, Some(maximum)).unwrap()
}

pub fn recurrence_instant_type() -> StructuredInfoType {
    record(
        "time/recurrence-instant@1",
        vec![
            field("basis", leaf("value/text@1")),
            field("resolution_ticks", leaf("value/count@1")),
            field("scale", leaf("time/scale@1")),
            field("ticks", leaf("value/count@1")),
        ],
    )
}

pub fn recurrence_monotonic_type() -> StructuredInfoType {
    record(
        "time/recurrence-monotonic@1",
        vec![
            field("basis", leaf("value/text@1")),
            field("boot", leaf("value/text@1")),
            field("host", leaf("value/text@1")),
            field("resolution_ticks", leaf("value/count@1")),
            field("scale", leaf("time/scale@1")),
            field("ticks", leaf("value/count@1")),
            field("uncertainty_ticks", leaf("value/count@1")),
        ],
    )
}

fn civil_rule_type() -> StructuredInfoType {
    record(
        "time/civil-weekday-rule@1",
        vec![
            field(
                "excluded_dates",
                bounded(
                    StructuredInfoType::variant(
                        kind_id("time/civil-date-exception-slot@1"),
                        vec![
                            case("exclude", leaf("time/local-date@1")),
                            case("unused", leaf("value/unit@1")),
                        ],
                    )
                    .unwrap(),
                    RECURRENCE_MAXIMUM_EXCEPTIONS,
                ),
            ),
            field("first_date", leaf("time/local-date@1")),
            field("local_time", leaf("time/local-time@1")),
            field("rule_set", leaf("value/text@1")),
            field("weekdays", leaf("value/count@1")),
            field("zone", leaf("value/text@1")),
        ],
    )
}

pub fn recurrence_rule_type() -> StructuredInfoType {
    let one_shot = record(
        "time/one-shot-rule@1",
        vec![field("at", recurrence_instant_type())],
    );
    let fixed = record(
        "time/fixed-elapsed-rule@1",
        vec![
            field("every_ticks", leaf("value/count@1")),
            field("first", recurrence_monotonic_type()),
        ],
    );
    StructuredInfoType::variant(
        kind_id("time/recurrence-rule@1"),
        vec![
            case("civil_weekdays", civil_rule_type()),
            case("fixed_elapsed", fixed),
            case("one_shot", one_shot),
        ],
    )
    .unwrap()
}

pub fn recurrence_until_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("time/recurrence-until@1"),
        vec![
            case("civil_date", leaf("time/local-date@1")),
            case("monotonic", recurrence_monotonic_type()),
            case("none", leaf("value/unit@1")),
            case("wall", recurrence_instant_type()),
        ],
    )
    .unwrap()
}

pub fn recurrence_window_type() -> StructuredInfoType {
    let wall = record(
        "time/recurrence-wall-window@1",
        vec![
            field("end", recurrence_instant_type()),
            field("start", recurrence_instant_type()),
        ],
    );
    let monotonic = record(
        "time/recurrence-monotonic-window@1",
        vec![
            field("end", recurrence_monotonic_type()),
            field("start", recurrence_monotonic_type()),
        ],
    );
    StructuredInfoType::variant(
        kind_id("time/recurrence-window@1"),
        vec![case("monotonic", monotonic), case("wall", wall)],
    )
    .unwrap()
}

fn resolution_payload_type(kind: &str, instants: &[&str]) -> StructuredInfoType {
    let mut fields = vec![
        field("local_date", leaf("time/local-date@1")),
        field("local_time", leaf("time/local-time@1")),
        field("ordinal", leaf("value/count@1")),
        field("rule_set", leaf("value/text@1")),
        field("zone", leaf("value/text@1")),
    ];
    fields.extend(
        instants
            .iter()
            .map(|name| field(name, recurrence_instant_type())),
    );
    record(kind, fields)
}

pub fn recurrence_resolution_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("time/civil-occurrence-resolution@1"),
        vec![
            case(
                "ambiguous",
                resolution_payload_type("time/ambiguous-civil-resolution@1", &["earlier", "later"]),
            ),
            case(
                "nonexistent",
                resolution_payload_type(
                    "time/nonexistent-civil-resolution@1",
                    &["gap_after", "gap_before"],
                ),
            ),
            case(
                "unique",
                resolution_payload_type("time/unique-civil-resolution@1", &["instant"]),
            ),
            case("unused", leaf("value/unit@1")),
        ],
    )
    .unwrap()
}

pub fn recurrence_request_type() -> StructuredInfoType {
    let ordinal_slot = StructuredInfoType::variant(
        kind_id("time/ordinal-exception-slot@1"),
        vec![
            case("exclude", leaf("value/count@1")),
            case("unused", leaf("value/unit@1")),
        ],
    )
    .unwrap();
    record(
        "time/recurrence-expansion@1",
        vec![
            field(
                "excluded_ordinals",
                bounded(ordinal_slot, RECURRENCE_MAXIMUM_EXCEPTIONS),
            ),
            field("fold_policy", leaf("time/fold-policy@1")),
            field("gap_policy", leaf("time/gap-policy@1")),
            field("identity", leaf("value/text@1")),
            field("maximum_occurrences", leaf("value/count@1")),
            field("maximum_results", leaf("value/count@1")),
            field(
                "resolutions",
                bounded(recurrence_resolution_type(), RECURRENCE_MAXIMUM_RESOLUTIONS),
            ),
            field("rule", recurrence_rule_type()),
            field("until", recurrence_until_type()),
            field("window", recurrence_window_type()),
        ],
    )
}

pub fn recurrence_occurrence_instant_type() -> StructuredInfoType {
    let civil = record(
        "time/civil-occurrence-instant@1",
        vec![
            field("instant", recurrence_instant_type()),
            field("local_date", leaf("time/local-date@1")),
            field("local_time", leaf("time/local-time@1")),
            field("resolution", leaf("time/civil-resolution-choice@1")),
            field("rule_set", leaf("value/text@1")),
            field("zone", leaf("value/text@1")),
        ],
    );
    StructuredInfoType::variant(
        kind_id("time/occurrence-instant@1"),
        vec![
            case("civil", civil),
            case("monotonic", recurrence_monotonic_type()),
            case("wall", recurrence_instant_type()),
        ],
    )
    .unwrap()
}

pub fn recurrence_occurrence_type() -> StructuredInfoType {
    record(
        RECURRENCE_OCCURRENCE_KIND,
        vec![
            field("identity", leaf("value/text@1")),
            field("instant", recurrence_occurrence_instant_type()),
            field("ordinal", leaf("value/count@1")),
            field("recurrence_identity", leaf("value/text@1")),
        ],
    )
}

pub fn recurrence_result_type() -> StructuredInfoType {
    let slot = StructuredInfoType::variant(
        kind_id("time/recurrence-occurrence-slot@1"),
        vec![
            case("occurrence", recurrence_occurrence_type()),
            case("unused", leaf("value/unit@1")),
        ],
    )
    .unwrap();
    record(
        RECURRENCE_RESULT_KIND,
        vec![
            field("count", leaf("value/count@1")),
            field("occurrences", bounded(slot, RECURRENCE_MAXIMUM_RESULTS)),
        ],
    )
}

pub fn install_recurrence_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    let request = recurrence_request_type();
    startup
        .insert_structured_type(RECURRENCE_REQUEST_TYPE, request.clone())
        .map_err(|error| error.to_string())?;
    startup
        .insert(KindSignature {
            kind: RECURRENCE_KIND.into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "request".into(),
                value_type: RECURRENCE_REQUEST_TYPE.into(),
                default: None,
            }],
        })
        .map_err(|error| error.to_string())?;
    let request_profile = request
        .profile()
        .map_err(|error| alloc::format!("{error:?}"))?;
    profile
        .insert(KindDefinition {
            kind_id: kind_id(RECURRENCE_KIND),
            kind_contract_revision: KindContractRevision::from(RECURRENCE_REVISION),
            inputs: vec![],
            outputs: vec![PortDescriptor {
                port_id: port_id("occurrences"),
                value_kind: recurrence_result_type()
                    .profile()
                    .map_err(|error| alloc::format!("{error:?}"))?
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
                        default_recurrence_request()?
                            .canonical_bytes()
                            .map_err(|error| {
                                alloc::format!("encode default recurrence request: {error:?}")
                            })?,
                    )
                    .ok_or_else(|| "default recurrence configuration is invalid".to_string())?,
                ),
                validation: ConfigurationRule::Structured {
                    profile: request_profile.value_kind().clone(),
                },
            }],
        })
        .map_err(|error| error.to_string())
}

fn default_recurrence_request() -> Result<StructuredInfoValue, String> {
    let unit = || leaf_value("value/unit@1", "");
    let instant = instant_value(0)?;
    let rule_type = recurrence_rule_type();
    let one_shot_type = variant_payload_type(&rule_type, "one_shot")?;
    let rule = StructuredInfoValue::variant(
        rule_type,
        "one_shot",
        record_value(one_shot_type, vec![("at", instant.clone())])?,
    )
    .map_err(value_error)?;
    let until_type = recurrence_until_type();
    let until = StructuredInfoValue::variant(until_type, "none", unit()?).map_err(value_error)?;
    let window_type = recurrence_window_type();
    let wall_type = variant_payload_type(&window_type, "wall")?;
    let window = StructuredInfoValue::variant(
        window_type,
        "wall",
        record_value(
            wall_type,
            vec![("end", instant.clone()), ("start", instant)],
        )?,
    )
    .map_err(value_error)?;
    let request_type = recurrence_request_type();
    let ordinal_slot = collection_element_type(&request_type, "excluded_ordinals")?;
    let resolution_slot = collection_element_type(&request_type, "resolutions")?;
    record_value(
        request_type,
        vec![
            (
                "excluded_ordinals",
                unused_slots(ordinal_slot, RECURRENCE_MAXIMUM_EXCEPTIONS)?,
            ),
            ("fold_policy", leaf_value("time/fold-policy@1", "refuse")?),
            ("gap_policy", leaf_value("time/gap-policy@1", "refuse")?),
            (
                "identity",
                leaf_value("value/text@1", "recurrence/default")?,
            ),
            ("maximum_occurrences", leaf_value("value/count@1", "1")?),
            ("maximum_results", leaf_value("value/count@1", "1")?),
            (
                "resolutions",
                unused_slots(resolution_slot, RECURRENCE_MAXIMUM_RESOLUTIONS)?,
            ),
            ("rule", rule),
            ("until", until),
            ("window", window),
        ],
    )
}

fn instant_value(ticks: u64) -> Result<StructuredInfoValue, String> {
    record_value(
        recurrence_instant_type(),
        vec![
            ("basis", leaf_value("value/text@1", "utc")?),
            ("resolution_ticks", leaf_value("value/count@1", "1")?),
            ("scale", leaf_value("time/scale@1", "seconds")?),
            ("ticks", leaf_value("value/count@1", &ticks.to_string())?),
        ],
    )
}

fn unused_slots(slot_type: StructuredInfoType, length: u16) -> Result<StructuredInfoValue, String> {
    let values = (0..length)
        .map(|_| {
            StructuredInfoValue::variant(
                slot_type.clone(),
                "unused",
                leaf_value("value/unit@1", "")?,
            )
            .map_err(value_error)
        })
        .collect::<Result<Vec<_>, String>>()?;
    StructuredInfoValue::collection(
        StructuredInfoType::collection(slot_type, Some(length)).map_err(value_error)?,
        values,
    )
    .map_err(value_error)
}

fn record_value(
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

fn leaf_value(kind: &str, value: &str) -> Result<StructuredInfoValue, String> {
    StructuredInfoValue::leaf(leaf(kind), value.as_bytes().to_vec()).map_err(value_error)
}

fn variant_payload_type(
    value_type: &StructuredInfoType,
    tag: &str,
) -> Result<StructuredInfoType, String> {
    let StructuredInfoTypeShape::Variant { cases, .. } = value_type.shape() else {
        return Err("default recurrence expected a variant type".into());
    };
    cases
        .iter()
        .find(|case| case.tag() == tag)
        .map(|case| case.payload_type().clone())
        .ok_or_else(|| "default recurrence variant case is missing".into())
}

fn collection_element_type(
    record_type: &StructuredInfoType,
    name: &str,
) -> Result<StructuredInfoType, String> {
    let StructuredInfoTypeShape::Record { fields, .. } = record_type.shape() else {
        return Err("default recurrence expected a record type".into());
    };
    let field = fields
        .iter()
        .find(|field| field.name() == name)
        .ok_or_else(|| "default recurrence collection field is missing".to_string())?;
    let StructuredInfoTypeShape::Collection { element, .. } = field.value_type().shape() else {
        return Err("default recurrence field is not a collection".into());
    };
    Ok(element.clone())
}

fn value_error(error: conduit_core::StructuredInfoRefusal) -> String {
    alloc::format!("default recurrence structured Info refusal: {error:?}")
}
