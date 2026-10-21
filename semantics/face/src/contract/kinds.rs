//! Canonical renderer/relay/input Kind assembly over the shared Fore contracts.
use super::*;

pub(super) fn renderer_contract() -> Kind {
    semantic_contract(
        RENDERER_KIND,
        RENDERER_CONTRACT_REVISION,
        renderer_inputs(),
        renderer_outputs(),
        MAX_RENDERER_VALUE_BYTES * u32::from(MAX_FACE_QUEUE_ITEMS),
    )
}

pub(super) fn show_resource_source_contract() -> Kind {
    let mut contract = semantic_contract(
        SHOW_RESOURCE_SOURCE_KIND,
        SHOW_RESOURCE_SOURCE_CONTRACT_REVISION,
        alloc::vec::Vec::new(),
        show_resource_source_outputs(),
        MAX_RENDERER_VALUE_BYTES * u32::from(MAX_FACE_QUEUE_ITEMS),
    );
    contract
        .semantic_laws
        .push(KindSemanticLaw::ResourcePorts(vec![
            show_resource_contract(),
        ]));
    contract
}

pub(super) fn resource_renderer_contract() -> Kind {
    let mut contract = semantic_contract(
        RESOURCE_RENDERER_KIND,
        RESOURCE_RENDERER_CONTRACT_REVISION,
        resource_renderer_inputs(),
        renderer_outputs(),
        MAX_RENDERER_VALUE_BYTES * u32::from(MAX_FACE_QUEUE_ITEMS),
    );
    contract
        .semantic_laws
        .push(KindSemanticLaw::ResourcePorts(vec![
            show_resource_contract(),
        ]));
    contract
}

pub(super) fn interaction_contract() -> Kind {
    semantic_contract(
        FACE_INTERACTION_KIND,
        FACE_INTERACTION_CONTRACT_REVISION,
        interaction_inputs(),
        interaction_outputs(),
        // This Fore receives a complete Face as well as a Show. Its semantic
        // capacity ceiling must cover those inputs, not only the smaller
        // outgoing action payload. Each Host still narrows its actual storage.
        MAX_RENDERER_VALUE_BYTES * u32::from(MAX_FACE_QUEUE_ITEMS),
    )
}

pub(super) fn face_tee_contract() -> Kind {
    semantic_contract(
        FACE_TEE_KIND,
        FACE_TEE_CONTRACT_REVISION,
        face_tee_inputs(),
        face_tee_outputs(),
        MAX_RENDERER_VALUE_BYTES * u32::from(MAX_FACE_QUEUE_ITEMS),
    )
}

pub(super) fn presenter_stage_contract() -> Kind {
    semantic_contract(
        PRESENTER_STAGE_KIND,
        PRESENTER_STAGE_CONTRACT_REVISION,
        presenter_stage_inputs(),
        presenter_stage_outputs(),
        MAX_RENDERER_VALUE_BYTES * u32::from(MAX_FACE_QUEUE_ITEMS),
    )
}
