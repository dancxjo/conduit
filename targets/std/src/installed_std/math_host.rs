//! Exact hosted math request matching and completion.

use super::{math_operations, InstalledScheduler};
use conduit_core::{kind_id, HostOperationContractId, KindId, PlanFragment, SCALAR_ENCODED_LEN};
use conduit_kernel::scheduler::HostOperationRequest;
use conduit_kernel::{BoundedValueRef, HostOperationDisposition, HostOperationOutcome};

pub(super) struct MathHost {
    bindings: [(HostOperationContractId, KindId); 4],
}

impl MathHost {
    pub(super) fn new() -> Self {
        Self {
            bindings: [
                (
                    conduit_std_offers::QUANTITY_MAP_HOST_OPERATION,
                    conduit_semantic_catalog::QUANTITY_MAP_KIND,
                ),
                (
                    conduit_std_offers::MATH_CLAMP_HOST_OPERATION,
                    conduit_semantic_catalog::MATH_CLAMP_KIND,
                ),
                (
                    conduit_std_offers::MATH_SCALE_HOST_OPERATION,
                    conduit_semantic_catalog::MATH_SCALE_KIND,
                ),
                (
                    conduit_std_offers::MATH_DEADBAND_HOST_OPERATION,
                    conduit_semantic_catalog::MATH_DEADBAND_KIND,
                ),
            ]
            .map(|(contract, kind)| (HostOperationContractId::from(contract), kind_id(kind))),
        }
    }

    pub(super) fn matches(
        &self,
        contract: &HostOperationContractId,
        target: Option<&KindId>,
    ) -> bool {
        self.bindings
            .iter()
            .any(|(expected_contract, expected_target)| {
                contract == expected_contract && target == Some(expected_target)
            })
    }

    pub(super) fn complete(
        &self,
        fragment: &PlanFragment,
        request: HostOperationRequest,
        scheduler: &mut InstalledScheduler,
        requests: &mut Vec<HostOperationRequest>,
    ) -> Result<(), String> {
        let placement = fragment
            .placements
            .get(usize::from(request.node.0))
            .ok_or_else(|| "math request has no exact placement".to_string())?;
        let input = scheduler
            .host_value(request.input.value)
            .map_err(|error| format!("read math scalar input: {error:?}"))?;
        if placement.kind_id.as_str() == conduit_semantic_catalog::QUANTITY_MAP_KIND {
            let mapping = super::quantity_mapping::configuration(placement)?;
            let outcome = match super::quantity_mapping::transform(mapping, input) {
                Ok(encoded) => {
                    let value = scheduler
                        .store_host_value(&encoded)
                        .map_err(|error| format!("store quantity output: {error:?}"))?;
                    HostOperationOutcome {
                        disposition: HostOperationDisposition::Completed,
                        output: Some(
                            BoundedValueRef::new(value, conduit_core::QUANTITY_ENCODED_LEN as u32)
                                .map_err(|error| format!("bound quantity output: {error:?}"))?,
                        ),
                        failure: None,
                    }
                }
                Err(failure) => HostOperationOutcome {
                    disposition: HostOperationDisposition::Failed,
                    output: None,
                    failure: Some(failure),
                },
            };
            requests.push(request);
            return scheduler
                .complete_host_operation(request.node, request.request, outcome)
                .map_err(|error| format!("complete quantity mapping: {error:?}"));
        }
        let encoded = math_operations::transform_bytes(placement, input)?;
        let value = scheduler
            .store_host_value(&encoded)
            .map_err(|error| format!("store math scalar output: {error:?}"))?;
        requests.push(request);
        scheduler
            .complete_host_operation(
                request.node,
                request.request,
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(value, SCALAR_ENCODED_LEN as u32)
                            .map_err(|error| format!("bound math scalar output: {error:?}"))?,
                    ),
                    failure: None,
                },
            )
            .map_err(|error| format!("complete math scalar transform: {error:?}"))
    }
}
