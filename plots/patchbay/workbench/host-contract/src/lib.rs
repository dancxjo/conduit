#![no_std]

//! Host-neutral execution boundary for the resident Patchbay workbench Plot.

extern crate alloc;

use alloc::{string::String, vec::Vec};
use conduit_core::{
    ActivePlayId, BootId, HostAdvertisement, HostId, Observation, OfferGeneration, Plan,
    PlanFragment,
};
use conduit_plot::ExpandedCanonicalPlot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchbayHostProfile {
    Signal,
    Text,
    Reference,
    PicoSimulation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlReceiptProjection {
    pub request_id: String,
    pub disposition: String,
    pub active_play_id: ActivePlayId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayExecutionProjection {
    pub active_play_id: ActivePlayId,
    pub decisions: u32,
    pub kernel_events: u16,
    pub kernel_sign: Vec<conduit_kernel::KernelEvent>,
    pub observations: Vec<Observation>,
    pub control_receipts: Vec<ControlReceiptProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchbayHostExecution {
    pub projection: PlayExecutionProjection,
    pub output: Vec<u8>,
}

pub trait PatchbayHostAdapter: Send + Sync {
    fn advertisement(
        &self,
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        profile: PatchbayHostProfile,
    ) -> Result<HostAdvertisement, String>;

    fn plan_expanded_local(
        &self,
        advertisement: &HostAdvertisement,
        expanded: &ExpandedCanonicalPlot,
    ) -> Result<Plan, String>;

    fn run_fragment(
        &self,
        advertisement: &HostAdvertisement,
        fragment: PlanFragment,
    ) -> Result<PatchbayHostExecution, String>;
}
