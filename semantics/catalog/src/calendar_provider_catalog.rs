//! Portable Form contracts for bounded calendar-provider interactions.
//!
//! The semantic JSON payloads are validated by the selected realization. They
//! contain portable calendar meaning only; provider, account, credential,
//! resource, host, and transport facts are deliberately absent.

use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, ConfigurationValue, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal, StructuredConfigurationValue, StructuredFieldType, StructuredFieldValue,
    StructuredInfoType, StructuredInfoValue,
};
use conduit_form::{
    ConfigurationField, ConfigurationRule, KindDefinition, KindSignature, StartupParameterSignature,
};

pub const CALENDAR_READ_KIND: &str = "calendar/read-events";
pub const CALENDAR_FREE_BUSY_KIND: &str = "calendar/query-free-busy";
pub const CALENDAR_CREATE_KIND: &str = "calendar/create-event";
pub const CALENDAR_UPDATE_KIND: &str = "calendar/update-event";
pub const CALENDAR_CANCEL_KIND: &str = "calendar/cancel-event";
pub const CALENDAR_INVITE_KIND: &str = "calendar/invite-participants";

pub const CALENDAR_READ_REVISION: &str = "calendar/read-events@1";
pub const CALENDAR_FREE_BUSY_REVISION: &str = "calendar/query-free-busy@1";
pub const CALENDAR_CREATE_REVISION: &str = "calendar/create-event@1";
pub const CALENDAR_UPDATE_REVISION: &str = "calendar/update-event@1";
pub const CALENDAR_CANCEL_REVISION: &str = "calendar/cancel-event@1";
pub const CALENDAR_INVITE_REVISION: &str = "calendar/invite-participants@1";

pub const CALENDAR_READ_REQUEST_TYPE: &str = "CalendarReadRequest";
pub const CALENDAR_FREE_BUSY_REQUEST_TYPE: &str = "CalendarFreeBusyRequest";
pub const CALENDAR_CREATE_REQUEST_TYPE: &str = "CalendarCreateRequest";
pub const CALENDAR_UPDATE_REQUEST_TYPE: &str = "CalendarUpdateRequest";
pub const CALENDAR_CANCEL_REQUEST_TYPE: &str = "CalendarCancelRequest";
pub const CALENDAR_INVITE_REQUEST_TYPE: &str = "CalendarInviteRequest";

pub const CALENDAR_READ_RESULT_TYPE: &str = "CalendarReadResult";
pub const CALENDAR_FREE_BUSY_RESULT_TYPE: &str = "CalendarFreeBusyResult";
pub const CALENDAR_WRITE_RECEIPT_TYPE: &str = "CalendarWriteReceipt";
pub const CALENDAR_CANCEL_RECEIPT_TYPE: &str = "CalendarCancelReceipt";

pub const CALENDAR_MAXIMUM_SEMANTIC_JSON_BYTES: u32 = 32 * 1_024;
pub const CALENDAR_MAXIMUM_RESULT_BYTES: u32 = 64 * 1_024;

#[derive(Clone, Copy)]
pub struct CalendarProviderKindContract {
    pub kind: &'static str,
    pub revision: &'static str,
    pub request_type_name: &'static str,
    pub input_type: Option<fn() -> StructuredInfoType>,
    pub input_port: Option<&'static str>,
    pub output_type: fn() -> StructuredInfoType,
    pub output_port: &'static str,
}

pub fn calendar_provider_contracts() -> [CalendarProviderKindContract; 6] {
    [
        contract(
            CALENDAR_READ_KIND,
            CALENDAR_READ_REVISION,
            CALENDAR_READ_REQUEST_TYPE,
            None,
            None,
            calendar_read_result_type,
            "events",
        ),
        contract(
            CALENDAR_FREE_BUSY_KIND,
            CALENDAR_FREE_BUSY_REVISION,
            CALENDAR_FREE_BUSY_REQUEST_TYPE,
            None,
            None,
            calendar_free_busy_result_type,
            "availability",
        ),
        contract(
            CALENDAR_CREATE_KIND,
            CALENDAR_CREATE_REVISION,
            CALENDAR_CREATE_REQUEST_TYPE,
            None,
            None,
            calendar_write_receipt_type,
            "receipt",
        ),
        contract(
            CALENDAR_UPDATE_KIND,
            CALENDAR_UPDATE_REVISION,
            CALENDAR_UPDATE_REQUEST_TYPE,
            Some(calendar_write_receipt_type),
            Some("prior"),
            calendar_write_receipt_type,
            "receipt",
        ),
        contract(
            CALENDAR_CANCEL_KIND,
            CALENDAR_CANCEL_REVISION,
            CALENDAR_CANCEL_REQUEST_TYPE,
            Some(calendar_write_receipt_type),
            Some("prior"),
            calendar_cancel_receipt_type,
            "receipt",
        ),
        contract(
            CALENDAR_INVITE_KIND,
            CALENDAR_INVITE_REVISION,
            CALENDAR_INVITE_REQUEST_TYPE,
            Some(calendar_write_receipt_type),
            Some("prior"),
            calendar_write_receipt_type,
            "receipt",
        ),
    ]
}

const fn contract(
    kind: &'static str,
    revision: &'static str,
    request_type_name: &'static str,
    input_type: Option<fn() -> StructuredInfoType>,
    input_port: Option<&'static str>,
    output_type: fn() -> StructuredInfoType,
    output_port: &'static str,
) -> CalendarProviderKindContract {
    CalendarProviderKindContract {
        kind,
        revision,
        request_type_name,
        input_type,
        input_port,
        output_type,
        output_port,
    }
}

pub fn calendar_read_request_type() -> StructuredInfoType {
    semantic_envelope("calendar/read-request@1")
}

pub fn calendar_free_busy_request_type() -> StructuredInfoType {
    semantic_envelope("calendar/free-busy-request@1")
}

pub fn calendar_create_request_type() -> StructuredInfoType {
    semantic_envelope("calendar/create-request@1")
}

pub fn calendar_update_request_type() -> StructuredInfoType {
    semantic_envelope("calendar/update-request@1")
}

pub fn calendar_cancel_request_type() -> StructuredInfoType {
    semantic_envelope("calendar/cancel-request@1")
}

pub fn calendar_invite_request_type() -> StructuredInfoType {
    semantic_envelope("calendar/invite-request@1")
}

pub fn calendar_read_result_type() -> StructuredInfoType {
    realization_envelope("calendar/read-result@1")
}

pub fn calendar_free_busy_result_type() -> StructuredInfoType {
    realization_envelope("calendar/free-busy-result@1")
}

pub fn calendar_write_receipt_type() -> StructuredInfoType {
    realization_envelope("calendar/write-receipt@1")
}

pub fn calendar_cancel_receipt_type() -> StructuredInfoType {
    realization_envelope("calendar/cancel-receipt@1")
}

pub fn calendar_request_type(contract: &CalendarProviderKindContract) -> StructuredInfoType {
    match contract.kind {
        CALENDAR_READ_KIND => calendar_read_request_type(),
        CALENDAR_FREE_BUSY_KIND => calendar_free_busy_request_type(),
        CALENDAR_CREATE_KIND => calendar_create_request_type(),
        CALENDAR_UPDATE_KIND => calendar_update_request_type(),
        CALENDAR_CANCEL_KIND => calendar_cancel_request_type(),
        CALENDAR_INVITE_KIND => calendar_invite_request_type(),
        _ => unreachable!("closed calendar provider contract inventory"),
    }
}

pub fn install_calendar_provider_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for contract in calendar_provider_contracts() {
        let request_type = calendar_request_type(&contract);
        startup
            .insert_structured_type(contract.request_type_name, request_type.clone())
            .map_err(|error| error.to_string())?;
        startup
            .insert(KindSignature {
                kind: contract.kind.into(),
                startup_parameters: vec![StartupParameterSignature {
                    name: "request".into(),
                    value_type: contract.request_type_name.into(),
                    default: None,
                }],
            })
            .map_err(|error| error.to_string())?;
        let request_profile = request_type
            .profile()
            .map_err(|error| format!("{error:?}"))?;
        let inputs = match (contract.input_type, contract.input_port) {
            (Some(value_type), Some(port_name)) => {
                vec![port(port_name, &value_type(), PortDirection::Input)?]
            }
            (None, None) => Vec::new(),
            _ => return Err("calendar provider input contract is inconsistent".into()),
        };
        profile
            .insert(KindDefinition {
                kind_id: kind_id(contract.kind),
                kind_contract_revision: KindContractRevision::from(contract.revision),
                inputs,
                outputs: vec![port(
                    contract.output_port,
                    &(contract.output_type)(),
                    PortDirection::Output,
                )?],
                configuration: vec![ConfigurationField {
                    key: "request".into(),
                    default_value: ConfigurationValue::Structured(
                        StructuredConfigurationValue::new(
                            request_profile.value_kind().clone(),
                            default_envelope(request_type)?
                                .canonical_bytes()
                                .map_err(|error| {
                                    format!("encode default calendar provider request: {error:?}")
                                })?,
                        )
                        .ok_or_else(|| {
                            "default calendar provider request is invalid".to_string()
                        })?,
                    ),
                    validation: ConfigurationRule::Structured {
                        profile: request_profile.value_kind().clone(),
                    },
                }],
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn semantic_envelope(kind: &str) -> StructuredInfoType {
    record(kind, "semantic_json")
}

fn realization_envelope(kind: &str) -> StructuredInfoType {
    record(kind, "realization_json")
}

fn record(kind: &str, field_name: &str) -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id(kind),
        vec![StructuredFieldType::new(
            field_name,
            StructuredInfoType::leaf(kind_id("value/text@1")).expect("reviewed text leaf"),
        )
        .expect("reviewed calendar envelope field")],
    )
    .expect("reviewed calendar envelope")
}

fn default_envelope(value_type: StructuredInfoType) -> Result<StructuredInfoValue, String> {
    let value = StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id("value/text@1")).map_err(|error| format!("{error:?}"))?,
        b"{}".to_vec(),
    )
    .map_err(|error| format!("{error:?}"))?;
    StructuredInfoValue::record(
        value_type,
        vec![StructuredFieldValue::new("semantic_json", value)
            .map_err(|error| format!("{error:?}"))?],
    )
    .map_err(|error| format!("{error:?}"))
}

fn port(
    name: &str,
    value_type: &StructuredInfoType,
    direction: PortDirection,
) -> Result<PortDescriptor, String> {
    Ok(PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type
            .profile()
            .map_err(|error| format!("{error:?}"))?
            .value_kind()
            .clone(),
        direction,
        temporal: PortTemporal::Value,
    })
}
