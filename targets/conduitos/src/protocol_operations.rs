//! Retained pure operation owners consumed together by a native protocol Play.
#[derive(Default)]
pub struct ProtocolOperations {
    pub joins: crate::flow_zip::FlowZipOperationFactory,
    pub states: crate::seeded_state::SeededStateOperationFactory,
    pub merges: crate::flow_merge_finite::FlowMergeFiniteOperationFactory,
}

#[cfg(test)]
mod tests;
