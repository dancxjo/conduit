//! Exact hosted math request matching and completion.

use super::{math_operations, InstalledScheduler};
use conduit_core::{kind_id, HostOperationContractId, KindId, PlanFragment, SCALAR_ENCODED_LEN};
use conduit_kernel::scheduler::HostOperationRequest;
use conduit_kernel::{BoundedValueRef, HostOperationDisposition, HostOperationOutcome};

pub(super) struct MathHost {
    bindings: [(HostOperationContractId, KindId); 4],
    failures: [Option<(HostOperationRequest, conduit_kernel::Failure)>; super::MAX_NODES],
    terminal_failure: Option<(HostOperationRequest, conduit_kernel::Failure)>,
}

impl MathHost {
    pub(super) fn new() -> Self {
        Self {
            failures: [None; super::MAX_NODES],
            terminal_failure: None,
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
        &mut self,
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
            if let Some(failure) = outcome.failure {
                self.failures[usize::from(request.node.0)] = Some((request, failure));
            }
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

    pub(super) fn accept_failure(
        &mut self,
        node: Option<conduit_kernel::NodeId>,
        detail: u16,
    ) -> bool {
        let failure = node
            .and_then(|node| self.failures.get(usize::from(node.0)))
            .copied()
            .flatten()
            .filter(|(_, failure)| failure.detail == detail);
        self.terminal_failure = failure;
        failure.is_some()
    }

    pub(super) fn failure_observation(
        &self,
        host: &conduit_core::HostAdvertisement,
        fragment: &PlanFragment,
        play: &conduit_core::ActivePlayIdentity,
        identity: &mut conduit_plan_lowering::lowering::KernelExecutionIdentityMap,
        sequence: &mut u64,
    ) -> Result<Option<conduit_core::Observation>, String> {
        let Some((request, failure)) = self.terminal_failure else {
            return Ok(None);
        };
        let sign = conduit_core::bind_sign(
            &host.host_id,
            &host.boot_id,
            Some(&play.active_play_id),
            *sequence,
        );
        *sequence = sequence
            .checked_add(1)
            .ok_or("quantity failure Sign sequence exhausted")?;
        identity
            .bind_sign(&sign, Some(request.node), Some(request.request), None)
            .map_err(|error| format!("bind quantity failure Sign: {error:?}"))?;
        let code = match failure.detail {
            1 => "math/map-quantity:malformed-scalar",
            2 => "math/map-quantity:invalid-range",
            3 => "math/map-quantity:out-of-range",
            4 => "math/map-quantity:inexact",
            5 => "math/map-quantity:overflow",
            _ => return Err("unknown quantity failure detail".into()),
        };
        Ok(Some(conduit_core::Observation {
            sign_id: sign.sign_id,
            active_play_id: Some(play.active_play_id.clone()),
            presentation_id: None,
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            plan_id: Some(fragment.plan_id.clone()),
            placement_id: Some(
                fragment.placements[usize::from(request.node.0)]
                    .placement_id
                    .clone(),
            ),
            connection_id: None,
            kind: conduit_core::ObservationKind::Failure {
                reason: conduit_core::FailureReason::RequiredBranchFailed,
                message: Some(code.into()),
            },
        }))
    }
}
