//! Pre-admitted matching and completion for face construction operations.

use super::{face_composition, layout_backs, InstalledScheduler};
use conduit_core::{kind_id, HostCallContractId, KindId, PlanFragment};
use conduit_kernel::scheduler::HostCallRequest;
use conduit_kernel::{BoundedValueRef, HostCallDisposition, HostCallOutcome};

pub(super) struct FaceConstructionHost {
    layout_contract_id: HostCallContractId,
    layout_target_kinds: [KindId; 5],
    face_composition_contract_id: HostCallContractId,
    face_composition_target_kinds: [KindId; 2],
    graphics_contract_id: HostCallContractId,
    graphics_target_kinds: [KindId; 3],
}

impl FaceConstructionHost {
    pub(super) fn prepare() -> Self {
        let layout_contract_id =
            conduit_core::HostCallContractId::from(conduit_std_offers::LAYOUT_HOST_CALL);
        let layout_target_kinds = [
            kind_id(conduit_semantic_catalog::LAYOUT_INSET_KIND),
            kind_id(conduit_semantic_catalog::LAYOUT_ROW_KIND),
            kind_id(conduit_semantic_catalog::LAYOUT_COLUMN_KIND),
            kind_id(conduit_semantic_catalog::LAYOUT_STACK_KIND),
            kind_id(conduit_semantic_catalog::LAYOUT_ALIGN_KIND),
        ];
        let face_composition_contract_id =
            conduit_core::HostCallContractId::from(conduit_std_offers::FACE_COMPOSITION_HOST_CALL);
        let face_composition_target_kinds = [
            kind_id(conduit_semantic_catalog::FACE_FRAME_KIND),
            kind_id(conduit_semantic_catalog::FACE_BADGE_KIND),
        ];
        let graphics_contract_id =
            conduit_core::HostCallContractId::from(conduit_std_offers::GRAPHICS_HOST_CALL);
        let graphics_target_kinds = [
            kind_id(conduit_semantic_catalog::GRAPHICS_RECT_KIND),
            kind_id(conduit_semantic_catalog::GRAPHICS_TEXT_KIND),
            kind_id(conduit_semantic_catalog::GRAPHICS_ICON_KIND),
        ];
        Self {
            layout_contract_id,
            layout_target_kinds,
            face_composition_contract_id,
            face_composition_target_kinds,
            graphics_contract_id,
            graphics_target_kinds,
        }
    }

    pub(super) fn matches(&self, contract: &HostCallContractId, target: Option<&KindId>) -> bool {
        (contract == &self.layout_contract_id
            && target.is_some_and(|target| self.layout_target_kinds.contains(target)))
            || (contract == &self.graphics_contract_id
                && target.is_some_and(|target| self.graphics_target_kinds.contains(target)))
            || (contract == &self.face_composition_contract_id
                && target.is_some_and(|target| self.face_composition_target_kinds.contains(target)))
    }

    pub(super) fn complete(
        &self,
        fragment: &PlanFragment,
        request: HostCallRequest,
        contract: &HostCallContractId,
        target: Option<&KindId>,
        scheduler: &mut InstalledScheduler,
        requests: &mut Vec<HostCallRequest>,
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
            let (encoded, encoded_len) = layout_backs::transform_bytes(placement, input)?;
            let value = scheduler
                .store_host_value(&encoded[..encoded_len])
                .map_err(|error| format!("store layout frame output: {error:?}"))?;
            requests.push(request);
            scheduler
                .complete_host_call(
                    request.node,
                    request.request,
                    HostCallOutcome {
                        disposition: HostCallDisposition::Completed,
                        output: Some(
                            BoundedValueRef::new(
                                value,
                                conduit_face::MAX_LAYOUT_FRAME_BYTES as u32,
                            )
                            .map_err(|error| format!("bound layout frame output: {error:?}"))?,
                        ),
                        failure: None,
                    },
                )
                .map_err(|error| format!("complete layout frame host-call: {error:?}"))?;
            return Ok(());
        } else if contract == &self.graphics_contract_id
            && target.is_some_and(|target| self.graphics_target_kinds.contains(target))
        {
            let placement = fragment
                .placements
                .get(usize::from(request.node.0))
                .ok_or_else(|| "graphics request has no exact placement".to_string())?;
            let (encoded, encoded_len) =
                face_composition::transform_graphics_bytes(placement, input)?;
            let value = scheduler
                .store_host_value(&encoded[..encoded_len])
                .map_err(|error| format!("store graphics scene: {error:?}"))?;
            requests.push(request);
            scheduler
                .complete_host_call(
                    request.node,
                    request.request,
                    HostCallOutcome {
                        disposition: HostCallDisposition::Completed,
                        output: Some(
                            BoundedValueRef::new(
                                value,
                                conduit_face::MAX_GRAPHICS_SCENE_BYTES as u32,
                            )
                            .map_err(|error| format!("bound graphics scene: {error:?}"))?,
                        ),
                        failure: None,
                    },
                )
                .map_err(|error| format!("complete graphics host-call: {error:?}"))?;
            return Ok(());
        } else if contract == &self.face_composition_contract_id
            && target.is_some_and(|target| self.face_composition_target_kinds.contains(target))
        {
            let placement = fragment
                .placements
                .get(usize::from(request.node.0))
                .ok_or_else(|| "face composition request has no exact placement".to_string())?;
            let (encoded, encoded_len) = face_composition::transform_bytes(placement, input)?;
            let value = scheduler
                .store_host_value(&encoded[..encoded_len])
                .map_err(|error| format!("store face composition: {error:?}"))?;
            requests.push(request);
            scheduler
                .complete_host_call(
                    request.node,
                    request.request,
                    HostCallOutcome {
                        disposition: HostCallDisposition::Completed,
                        output: Some(
                            BoundedValueRef::new(
                                value,
                                conduit_face::MAX_FACE_COMPOSITION_BYTES as u32,
                            )
                            .map_err(|error| format!("bound face composition: {error:?}"))?,
                        ),
                        failure: None,
                    },
                )
                .map_err(|error| format!("complete face composition: {error:?}"))?;
            return Ok(());
        }
        Err("face construction request has no matched contract and target".to_string())
    }
}
