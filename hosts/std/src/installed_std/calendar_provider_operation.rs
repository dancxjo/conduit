//! One-request installed operations for exact planned calendar effects.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear, PortDirection, StructuredInfoValue};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId, ValueRef, ValueStorage,
};

pub(super) static CALENDAR_READ_FACTORY: InstalledFactory =
    factory(crate::hosted_calendar::CalendarHostedOperation::Read.implementation());
pub(super) static CALENDAR_FREE_BUSY_FACTORY: InstalledFactory =
    factory(crate::hosted_calendar::CalendarHostedOperation::FreeBusy.implementation());
pub(super) static CALENDAR_CREATE_FACTORY: InstalledFactory =
    factory(crate::hosted_calendar::CalendarHostedOperation::Create.implementation());
pub(super) static CALENDAR_UPDATE_FACTORY: InstalledFactory =
    factory(crate::hosted_calendar::CalendarHostedOperation::Update.implementation());
pub(super) static CALENDAR_CANCEL_FACTORY: InstalledFactory =
    factory(crate::hosted_calendar::CalendarHostedOperation::Cancel.implementation());
pub(super) static CALENDAR_INVITE_FACTORY: InstalledFactory =
    factory(crate::hosted_calendar::CalendarHostedOperation::Invite.implementation());

const fn factory(implementation_id: &'static str) -> InstalledFactory {
    InstalledFactory {
        implementation_id,
        budget,
        prepare,
    }
}

pub(super) struct CalendarProviderOperation {
    request: Option<ValueRef>,
    requires_prior: bool,
    pending: bool,
    emitted: bool,
}

impl CalendarProviderOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        if self.requires_prior {
            OperationAction::Await
        } else {
            let Some(value) = self.request else {
                return fail(FailureCode::InvalidLifecycle, 240);
            };
            self.request(value)
        }
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.requires_prior && !self.pending && !self.emitted => self.request(value),
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending && request == RequestId(0) =>
            {
                self.pending = false;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostOperationDisposition::Completed, Some(output), None) => {
                        self.emitted = true;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostOperationDisposition::Denied, _, _) => {
                        fail(FailureCode::HostOperationDenied, 241)
                    }
                    (HostOperationDisposition::Cancelled, _, _) => {
                        fail(FailureCode::Cancelled, 242)
                    }
                    (HostOperationDisposition::Failed, _, Some(failure)) => {
                        OperationAction::Fail(failure)
                    }
                    _ => fail(FailureCode::InvalidLifecycle, 243),
                }
            }
            _ => fail(FailureCode::InvalidLifecycle, 244),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if self.emitted {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = false;
    }

    fn request(&mut self, value: ValueRef) -> OperationAction {
        self.pending = true;
        let maximum_input_bytes = if self.requires_prior {
            conduit_std_catalog::CALENDAR_MAXIMUM_RESULT_BYTES
        } else {
            conduit_std_catalog::CALENDAR_MAXIMUM_SEMANTIC_JSON_BYTES
        };
        let Ok(input) = BoundedValueRef::new(value, maximum_input_bytes) else {
            return fail(FailureCode::InvalidInput, 245);
        };
        OperationAction::RequestHostOperation {
            request: RequestId(0),
            operation: HostOperationId(0),
            input,
        }
    }
}

pub(super) fn operation(
    placement: &PlannedGear,
) -> Option<crate::hosted_calendar::CalendarHostedOperation> {
    crate::hosted_calendar::CalendarHostedOperation::from_implementation(
        placement.implementation_id.as_str(),
    )
}

pub(super) fn request_value(placement: &PlannedGear) -> Result<StructuredInfoValue, String> {
    let [entry] = placement.configuration.as_slice() else {
        return Err("calendar provider operation requires one planned request".into());
    };
    let ("request", ConfigurationValue::Structured(configuration)) =
        (entry.key.as_str(), &entry.value)
    else {
        return Err("calendar provider planned request is malformed".into());
    };
    let value = StructuredInfoValue::from_canonical_bytes(configuration.canonical_value())
        .map_err(|error| format!("decode calendar provider request: {error:?}"))?;
    let operation = operation(placement)
        .ok_or_else(|| "calendar provider implementation is unknown".to_string())?;
    let offer = crate::hosted_calendar::google_calendar_offers()
        .into_iter()
        .find(|offer| offer.implementation.implementation_id == placement.implementation_id)
        .ok_or_else(|| "calendar provider offer is absent".to_string())?;
    let contract = conduit_std_catalog::calendar_provider_contracts()
        .into_iter()
        .find(|contract| contract.kind == offer.kind_id.as_str())
        .ok_or_else(|| "calendar provider portable contract is absent".to_string())?;
    let expected = conduit_std_catalog::calendar_request_type(&contract);
    if value.value_type() != &expected
        || operation.contract() != offer.host_operations[0].contract_id.as_str()
    {
        return Err("calendar provider request type differs from its realization".into());
    }
    Ok(value)
}

fn validate(
    placement: &PlannedGear,
) -> Result<crate::hosted_calendar::CalendarHostedOperation, String> {
    let operation = operation(placement)
        .ok_or_else(|| "planned calendar implementation is not installed".to_string())?;
    let offer = crate::hosted_calendar::google_calendar_offers()
        .into_iter()
        .find(|offer| offer.implementation.implementation_id == placement.implementation_id)
        .ok_or_else(|| "planned calendar offer is absent".to_string())?;
    let expected_authorities =
        if operation == crate::hosted_calendar::CalendarHostedOperation::Invite {
            2
        } else {
            1
        };
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || placement.limits != offer.limits
        || placement.resources.len() != 1
        || placement.resources[0].class_id.as_str()
            != crate::hosted_calendar::GOOGLE_CALENDAR_RESOURCE_CLASS
        || placement.resources[0].units != 1
        || placement.resources[0].protected.is_some()
        || placement.resources[0].compute.is_some()
        || placement.authority.len() != expected_authorities
        || placement.authority.iter().any(|authority| {
            authority.host_id != placement.host_id
                || authority.boot_id != placement.boot_id
                || authority.capability_id != placement.capability_id
                || authority.host_operation_contract_id != placement.host_operations[0].contract_id
                || Some(&authority.subject_kind)
                    != placement.host_operations[0].target_kind.as_ref()
        })
        || placement
            .inputs
            .iter()
            .any(|port| port.direction != PortDirection::Input)
    {
        return Err("planned calendar provider identity/resource/authority mismatch".into());
    }
    let actual_authorities = placement
        .authority
        .iter()
        .map(|authority| authority.contract_id.as_str())
        .collect::<Vec<_>>();
    let expected = offer
        .authority_requirements
        .iter()
        .map(|authority| authority.contract_id.as_str())
        .collect::<Vec<_>>();
    if actual_authorities != expected {
        return Err("planned calendar provider authority set is not exact".into());
    }
    request_value(placement)?;
    Ok(operation)
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    let request_bytes = placement
        .configuration
        .first()
        .and_then(|entry| match &entry.value {
            ConfigurationValue::Structured(value) => Some(value.canonical_value().len()),
            _ => None,
        })
        .ok_or_else(|| "calendar request byte budget is absent".to_string())?;
    let request_bytes = u32::try_from(request_bytes)
        .map_err(|_| "calendar request byte budget overflow".to_string())?;
    Ok(OperationBudget {
        value_items: 2,
        value_bytes: request_bytes
            .checked_add(conduit_std_catalog::CALENDAR_MAXIMUM_RESULT_BYTES)
            .ok_or_else(|| "calendar value byte budget overflow".to_string())?,
        host_requests: 1,
        sign_items: 24,
        maximum_value_bytes: request_bytes.max(conduit_std_catalog::CALENDAR_MAXIMUM_RESULT_BYTES),
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    let operation = validate(placement)?;
    let request = if matches!(
        operation,
        crate::hosted_calendar::CalendarHostedOperation::Read
            | crate::hosted_calendar::CalendarHostedOperation::FreeBusy
            | crate::hosted_calendar::CalendarHostedOperation::Create
    ) {
        let [entry] = placement.configuration.as_slice() else {
            unreachable!("validated calendar request")
        };
        let ConfigurationValue::Structured(request) = &entry.value else {
            unreachable!("validated calendar request")
        };
        Some(
            values
                .store(request.canonical_value())
                .map_err(|error| format!("store calendar request: {error:?}"))?,
        )
    } else {
        None
    };
    Ok(InstalledOperation::CalendarProvider(
        CalendarProviderOperation {
            request,
            requires_prior: operation != crate::hosted_calendar::CalendarHostedOperation::Read
                && operation != crate::hosted_calendar::CalendarHostedOperation::FreeBusy
                && operation != crate::hosted_calendar::CalendarHostedOperation::Create,
            pending: false,
            emitted: false,
        },
    ))
}

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}
