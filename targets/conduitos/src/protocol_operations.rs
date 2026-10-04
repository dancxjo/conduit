//! Retained generic operation owners supplied during native preparation.
#[derive(Default)]
pub struct ProtocolOperations {
    pub joins: crate::flow_zip::FlowZipOperationFactory,
    pub states: crate::seeded_state::SeededStateOperationFactory,
}
