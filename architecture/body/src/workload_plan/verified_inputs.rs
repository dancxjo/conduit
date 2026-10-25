//! Borrowed verified inputs; no lifecycle orchestration or new authority.
use super::{BodyFaceSelector, PlacementId, ResidentPlot};
use alloc::vec::Vec;
use conduit_core::VerifiedPlanMetadata;

#[derive(Debug)]
pub struct VerifiedBodyPlotPlan<'a> {
    pub plot: ResidentPlot,
    pub plan: &'a VerifiedPlanMetadata,
}
#[derive(Debug)]
pub struct VerifiedBodyMaskChain<'a> {
    pub plan: &'a VerifiedPlanMetadata,
    pub stage_placement_ids: Vec<PlacementId>,
}
#[derive(Debug)]
pub struct VerifiedBodyMaskTopology<'a> {
    pub face: BodyFaceSelector,
    pub chains: Vec<VerifiedBodyMaskChain<'a>>,
}
