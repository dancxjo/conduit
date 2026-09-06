//! Pre-admitted matching and completion for presentation construction operations.

use super::{layout_operations, presentation_composition, InstalledScheduler};
use conduit_core::{kind_id, HostOperationContractId, KindId, PlanFragment};
use conduit_kernel::scheduler::HostOperationRequest;
use conduit_kernel::{BoundedValueRef, HostOperationDisposition, HostOperationOutcome};

pub(super) struct PresentationConstructionHost {
    layout_contract_id: HostOperationContractId,
    layout_target_kinds: [KindId; 5],
    presentation_composition_contract_id: HostOperationContractId,
    presentation_composition_target_kinds: [KindId; 2],
    graphics_contract_id: HostOperationContractId,
    graphics_target_kinds: [KindId; 3],
}

impl PresentationConstructionHost {
    pub(super) fn prepare() -> Self {
        let layout_contract_id =
            conduit_core::HostOperationContractId::from(conduit_std_offers::LAYOUT_HOST_OPERATION);
        let layout_target_kinds = [
            kind_id(conduit_semantic_catalog::LAYOUT_INSET_KIND),
            kind_id(conduit_semantic_catalog::LAYOUT_ROW_KIND),
            kind_id(conduit_semantic_catalog::LAYOUT_COLUMN_KIND),
            kind_id(conduit_semantic_catalog::LAYOUT_STACK_KIND),
            kind_id(conduit_semantic_catalog::LAYOUT_ALIGN_KIND),
        ];
        let presentation_composition_contract_id = conduit_core::HostOperationContractId::from(
            conduit_std_offers::PRESENTATION_COMPOSITION_HOST_OPERATION,
        );
        let presentation_composition_target_kinds = [
            kind_id(conduit_semantic_catalog::PRESENTATION_FRAME_KIND),
            kind_id(conduit_semantic_catalog::PRESENTATION_BADGE_KIND),
        ];
        let graphics_contract_id = conduit_core::HostOperationContractId::from(
            conduit_std_offers::GRAPHICS_HOST_OPERATION,
        );
        let graphics_target_kinds = [
            kind_id(conduit_semantic_catalog::GRAPHICS_RECT_KIND),
            kind_id(conduit_semantic_catalog::GRAPHICS_TEXT_KIND),
            kind_id(conduit_semantic_catalog::GRAPHICS_ICON_KIND),
        ];
        Self {
            layout_contract_id,
            layout_target_kinds,
            presentation_composition_contract_id,
            presentation_composition_target_kinds,
            graphics_contract_id,
            graphics_target_kinds,
        }
    }

    pub(super) fn matches(
        &self,
        contract: &HostOperationContractId,
        target: Option<&KindId>,
    ) -> bool {
        (contract == &self.layout_contract_id
            && target.is_some_and(|target| self.layout_target_kinds.contains(target)))
            || (contract == &self.graphics_contract_id
                && target.is_some_and(|target| self.graphics_target_kinds.contains(target)))
            || (contract == &self.presentation_composition_contract_id
                && target.is_some_and(|target| {
                    self.presentation_composition_target_kinds.contains(target)
                }))
    }

    pub(super) fn complete(
        &self,
        fragment: &PlanFragment,
        request: HostOperationRequest,
        contract: &HostOperationContractId,
        target: Option<&KindId>,
        scheduler: &mut InstalledScheduler,
        requests: &mut Vec<HostOperationRequest>,
    ) -> Result<(), String> {
        let input = scheduler
            .host_value(request.input.value)
            .map_err(|error| format!("read std host input: {error:?}"))?;
        if contract == &self.layout_contract_id
            && target.is_some_and(|target| self.layout_target_kinds.contains(target))
        {
            let placement = fragment
                .placements
                .get(usize::from(request.node.0))
                .ok_or_else(|| "layout request has no exact placement".to_string())?;
            let (encoded, encoded_len) = layout_operations::transform_bytes(placement, input)?;
            let value = scheduler
                .store_host_value(&encoded[..encoded_len])
                .map_err(|error| format!("store layout frame output: {error:?}"))?;
            requests.push(request);
            scheduler
                .complete_host_operation(
                    request.node,
                    request.request,
                    HostOperationOutcome {
                        disposition: HostOperationDisposition::Completed,
                        output: Some(
                            BoundedValueRef::new(
                                value,
                                conduit_presentation::MAX_LAYOUT_FRAME_BYTES as u32,
                            )
                            .map_err(|error| format!("bound layout frame output: {error:?}"))?,
                        ),
                        failure: None,
                    },
                )
                .map_err(|error| format!("complete layout frame host operation: {error:?}"))?;
            return Ok(());
        } else if contract == &self.graphics_contract_id
            && target.is_some_and(|target| self.graphics_target_kinds.contains(target))
        {
            let placement = fragment
                .placements
                .get(usize::from(request.node.0))
                .ok_or_else(|| "graphics request has no exact placement".to_string())?;
            let (encoded, encoded_len) =
                presentation_composition::transform_graphics_bytes(placement, input)?;
            let value = scheduler
                .store_host_value(&encoded[..encoded_len])
                .map_err(|error| format!("store graphics scene: {error:?}"))?;
            requests.push(request);
            scheduler
                .complete_host_operation(
                    request.node,
                    request.request,
                    HostOperationOutcome {
                        disposition: HostOperationDisposition::Completed,
                        output: Some(
                            BoundedValueRef::new(
                                value,
                                conduit_presentation::MAX_GRAPHICS_SCENE_BYTES as u32,
                            )
                            .map_err(|error| format!("bound graphics scene: {error:?}"))?,
                        ),
                        failure: None,
                    },
                )
                .map_err(|error| format!("complete graphics host operation: {error:?}"))?;
            return Ok(());
        } else if contract == &self.presentation_composition_contract_id
            && target
                .is_some_and(|target| self.presentation_composition_target_kinds.contains(target))
        {
            let placement = fragment
                .placements
                .get(usize::from(request.node.0))
                .ok_or_else(|| {
                    "presentation composition request has no exact placement".to_string()
                })?;
            let (encoded, encoded_len) =
                presentation_composition::transform_bytes(placement, input)?;
            let value = scheduler
                .store_host_value(&encoded[..encoded_len])
                .map_err(|error| format!("store presentation composition: {error:?}"))?;
            requests.push(request);
            scheduler
                .complete_host_operation(
                    request.node,
                    request.request,
                    HostOperationOutcome {
                        disposition: HostOperationDisposition::Completed,
                        output: Some(
                            BoundedValueRef::new(
                                value,
                                conduit_presentation::MAX_PRESENTATION_COMPOSITION_BYTES as u32,
                            )
                            .map_err(|error| {
                                format!("bound presentation composition: {error:?}")
                            })?,
                        ),
                        failure: None,
                    },
                )
                .map_err(|error| format!("complete presentation composition: {error:?}"))?;
            return Ok(());
        }
        Err("presentation construction request has no matched contract and target".to_string())
    }
}
