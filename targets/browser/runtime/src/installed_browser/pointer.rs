//! Existing browser pointer offer installed in the ordinary Form runner.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, Operation, OperationAction,
    OperationInput, PortId, RequestId, ValueRef, ValueStorage,
};

pub(crate) const HOST_OPERATION: &str = "browser.host/pointer-source@1";
pub(super) static POINTER: BrowserInstallation = BrowserInstallation {
    implementation_id: "browser/form-pointer-source@1",
    offer,
    prepare,
    perform: None,
};

fn offer() -> conduit_core::CapabilityOffer {
    let mut offer = crate::browser_pointer::advertisement()
        .capabilities
        .into_iter()
        .find(|offer| offer.kind_id.as_str() == conduit_semantic_catalog::POINTER_SOURCE_KIND)
        .expect("existing pointer advertisement owns its exact source");
    // This installed envelope is smaller than the standalone pointer Host.
    // Keep its implementation/profile distinct instead of mutating that offer's identity.
    offer.capability_id = "browser-form-pointer-source@1".into();
    offer.implementation.implementation_id = "browser/form-pointer-source@1".into();
    offer.implementation.execution_profile_id = "browser/form-pointer-source@1".into();
    offer.implementation.artifact_id = "conduit-browser-runtime/form-pointer-source@1".into();
    offer.limits.max_queue_bytes = super::MAXIMUM_BROWSER_VALUE_BYTES as u32;
    offer.host_operations[0].maximum_output_bytes = super::MAXIMUM_BROWSER_VALUE_BYTES as u32;
    offer
}

fn prepare(
    placement: &conduit_core::PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &offer())?;
    let empty = values
        .store(&[])
        .map_err(|error| format!("pointer request: {error:?}"))?;
    Ok(BrowserOperation::installed(PointerSource {
        empty,
        pending: false,
        done: false,
    }))
}

struct PointerSource {
    empty: ValueRef,
    pending: bool,
    done: bool,
}

impl Operation for PointerSource {
    fn start(&mut self) -> OperationAction {
        if self.pending || self.done {
            return fail();
        }
        self.pending = true;
        OperationAction::RequestHostOperation {
            request: RequestId(0),
            operation: HostOperationId(0),
            input: BoundedValueRef::new(self.empty, 0).expect("empty pointer request"),
        }
    }
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        if let OperationInput::HostOperationCompleted {
            request: RequestId(0),
            outcome,
        } = input
        {
            if !self.pending {
                return fail();
            }
            self.pending = false;
            self.done = true;
            return match (outcome.disposition, outcome.output, outcome.failure) {
                (HostOperationDisposition::Completed, Some(output), None)
                    if output.admitted_bytes == super::MAXIMUM_BROWSER_VALUE_BYTES as u32 =>
                {
                    OperationAction::Emit {
                        port: PortId(0),
                        value: output.value,
                    }
                }
                (HostOperationDisposition::Failed, None, Some(failure)) => {
                    OperationAction::Fail(failure)
                }
                _ => fail(),
            };
        }
        fail()
    }
    fn advance(&mut self) -> OperationAction {
        if self.done {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }
    fn cancel(&mut self) {
        self.pending = false;
        self.done = true;
    }
}

fn fail() -> OperationAction {
    OperationAction::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidInput,
        detail: 21,
    })
}
