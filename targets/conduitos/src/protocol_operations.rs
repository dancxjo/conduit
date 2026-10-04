//! Retained pure operation owners consumed together by a native protocol Play.
#[derive(Default)]
pub struct ProtocolOperations {
    pub joins: crate::flow_zip::FlowZipOperationFactory,
    pub states: crate::seeded_state::SeededStateOperationFactory,
}

#[cfg(test)]
mod tests;
