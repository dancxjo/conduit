//! Exact native selected request binding; no new operation authority.
use super::*;
impl<P: EnvironmentalSourceProvider> EnvironmentalSourceHostCall<P> {
    /// # Safety
    /// Root must retain the actual exclusive initialized provider and checked
    /// configuration independently of received Plan bytes. Its finite transport,
    /// cancellation and memory bounds must already be admitted. A claimed ID or
    /// a diagnostic packet cannot establish possession at this native boundary.
    #[allow(clippy::result_large_err)] // Failed preparation must return every affine owner.
    pub unsafe fn bind_selected(
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        provider: P,
        contract: &ObservationContract,
        selected: SelectedOperationPlan<'_>,
    ) -> Result<
        Self,
        (
            BaseCapabilityTable,
            BaseCapabilityHandle,
            BaseOperationClaim,
            P,
            Refusal,
        ),
    > {
        unsafe {
            Self::bind_selected_request_inner(
                table, handle, claim, provider, contract, selected, None,
            )
        }
    }

    /// Bind one admitted kernel request to a freshly retained single-call owner.
    /// Request correlation is copied from the original affine work; operation
    /// authority remains the independently selected finite Base capability.
    /// # Safety
    /// The requirements of `bind_selected` apply. `request` must be the original
    /// request held by that selected native work, never a fabricated/rebased ID.
    #[allow(clippy::result_large_err)] // Failed preparation must return every affine owner.
    pub unsafe fn bind_selected_request(
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        provider: P,
        contract: &ObservationContract,
        selected: SelectedOperationPlan<'_>,
        request: conduit_kernel::scheduler::HostCallRequest,
    ) -> Result<
        Self,
        (
            BaseCapabilityTable,
            BaseCapabilityHandle,
            BaseOperationClaim,
            P,
            Refusal,
        ),
    > {
        unsafe {
            Self::bind_selected_request_inner(
                table,
                handle,
                claim,
                provider,
                contract,
                selected,
                Some(request),
            )
        }
    }

    #[allow(clippy::result_large_err)] // Failed preparation must return every affine owner.
    unsafe fn bind_selected_request_inner(
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        provider: P,
        contract: &ObservationContract,
        selected: SelectedOperationPlan<'_>,
        request: Option<conduit_kernel::scheduler::HostCallRequest>,
    ) -> Result<
        Self,
        (
            BaseCapabilityTable,
            BaseCapabilityHandle,
            BaseOperationClaim,
            P,
            Refusal,
        ),
    > {
        let prepared = (|| -> Result<_, Refusal> {
            let mut inspections = table.inspections();
            let entry = inspections.next().ok_or(Refusal::Possession)?;
            if inspections.next().is_some()
                || entry.scope.authority_contract_id.as_str() != AUTHORITY_CONTRACT
                || entry.scope.maximum_in_flight != 1
                || entry.scope.maximum_operations == 0
                || entry.scope.maximum_parameter_bytes < 1
                || entry.scope.maximum_result_bytes < MAXIMUM_BYTES as u32
                || claim.parameter_bytes != 1
                || claim.work_units != 1
            {
                return Err(Refusal::Possession);
            }
            drop(inspections);
            let gear = selected
                .fragment
                .placements
                .iter()
                .find(|p| &p.placement_id == selected.placement_id)
                .ok_or(Refusal::Possession)?;
            if gear.resources.len() != 1
                || gear.resources[0].class_id.as_str() != RESOURCE_CLASS
                || gear.resources[0].units != 1
            {
                return Err(Refusal::Possession);
            }
            let kind = contract.kind(false).map_err(Refusal::Encoding)?;
            let scope = crate::environmental_publication::PublicationScope {
                host: selected.fragment.host_id.clone(),
                boot: selected.fragment.boot_id.clone(),
                plan: selected.fragment.plan_id.clone(),
                play: selected.active.active_play_id.clone(),
                source: selected.fragment.source_document_id.clone(),
                checked: selected.fragment.checked_plot_id.clone(),
                expanded: selected.fragment.expanded_plot_id.clone(),
            };
            let fragment = selected.fragment.fragment_id.clone();
            let placement = selected.placement_id.clone();
            let node = bind_selected_operation(
                &table,
                &claim,
                SelectedOperationContract {
                    kind: &kind,
                    call: HOST_CALL,
                    input_bytes: 1,
                    output_bytes: MAXIMUM_BYTES as u32,
                    resource_bytes: MAXIMUM_BYTES as u64,
                },
                selected,
            )
            .map_err(Refusal::Binding)?;
            if request.is_some_and(|request| !request_matches(node, request)) {
                return Err(Refusal::Request);
            }
            let codec = PreparedObservationCodec::prepare(contract).map_err(Refusal::Encoding)?;
            Ok((scope, fragment, placement, node, codec))
        })();
        let (scope, fragment, placement, node, codec) = match prepared {
            Ok(prepared) => prepared,
            Err(reason) => return Err((table, handle, claim, provider, reason)),
        };
        let operation = match BoundBaseOperation::bind(table, handle, claim) {
            Ok(operation) => operation,
            Err((table, handle, claim, reason)) => {
                return Err((table, handle, claim, provider, Refusal::Capability(reason)))
            }
        };
        Ok(Self {
            scope,
            fragment,
            placement,
            operation,
            provider,
            node,
            next_request: request.map_or(0, |request| request.request.0),
            codec,
        })
    }
}

fn request_matches(node: NodeId, request: conduit_kernel::scheduler::HostCallRequest) -> bool {
    request.node == node
        && request.call == HostCallId(0)
        && request.input.value.byte_len == 0
        && request.input.admitted_bytes == 1
        && request.request.0.checked_add(1).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn correlation_binding_refuses_other_node_call_payload_bound_or_exhaustion() {
        let request = conduit_kernel::scheduler::HostCallRequest {
            node: NodeId(3),
            call: HostCallId(0),
            request: RequestId(9),
            input: conduit_kernel::BoundedValueRef::new(
                conduit_kernel::ValueRef {
                    slot: 0,
                    generation: 0,
                    byte_len: 0,
                },
                1,
            )
            .unwrap(),
        };
        assert!(request_matches(NodeId(3), request));
        assert!(!request_matches(NodeId(4), request));
        assert!(!request_matches(
            NodeId(3),
            conduit_kernel::scheduler::HostCallRequest {
                call: HostCallId(1),
                ..request
            }
        ));
        assert!(!request_matches(
            NodeId(3),
            conduit_kernel::scheduler::HostCallRequest {
                request: RequestId(u32::MAX),
                ..request
            }
        ));
        let altered = conduit_kernel::BoundedValueRef::new(
            conduit_kernel::ValueRef {
                slot: 0,
                generation: 0,
                byte_len: 0,
            },
            0,
        )
        .unwrap();
        assert!(!request_matches(
            NodeId(3),
            conduit_kernel::scheduler::HostCallRequest {
                input: altered,
                ..request
            }
        ));
    }
}
