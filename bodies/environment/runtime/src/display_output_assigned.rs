//! Compact assignment -> retained software -> unchanged ordinary output kernel.
//! Root separately owns admission, Fore value contracts and native possession.
use super::*;
use crate::assigned::{AssignedKernelRefusal, ResidentLayout};
#[derive(Clone, Copy)]
pub struct AssignedDisplayInput<'a> {
    pub header: &'a [u8],
    pub records: &'a [c::AssignedRecord<'a>],
    pub maxima: c::AssignedPlanMaxima,
}
/// Linked Make facts, never copied from the caller's incoming assignment.
pub struct AssignedDisplayProfile<'a> {
    pub source: &'a c::PlotIdentity,
    pub layout: ResidentLayout<'a>,
    pub computations: c::AssignedComputationProfile<'a>,
    pub fore: &'a [c::AssignedForeRequirement],
    pub host_calls: &'a [c::AssignedIdentity],
    pub resources: &'a [u16],
    pub write: k::NodeId,
    /// Linked Make Type ceiling, not a caller-supplied observed payload size.
    pub maximum_value_bytes: u32,
    pub sign_items: u16,
    pub sign_bytes: u32,
}
#[derive(Clone, Copy)]
pub struct AssignedDisplayScope<'a> {
    pub host: &'a c::HostId,
    pub boot: &'a c::BootId,
    pub generation: c::OfferGeneration,
}
pub struct PreparedAssignedDisplayKernel<S: k::ValueStorage, L: k::SignSink> {
    pub identity: c::AssignedPlanView,
    pub kernel: Box<DisplayKernel<S, L>>,
    pub selectors: [Option<PreparedCaptureSelector>; 4],
    pub write: k::NodeId,
    pub input: c::AssignedForeRequirement,
    pub refusal: c::AssignedForeRequirement,
}
#[derive(Debug, PartialEq, Eq)]
pub enum AssignedDisplayRefusal {
    Shape,
    Bounds,
    Tables,
    Assignment(AssignedKernelRefusal),
    Software(SoftwareRefusal),
}
impl<S: k::ValueStorage, L: k::SignSink> PreparedAssignedDisplayKernel<S, L> {
    /// No Plan, Source graph, advertised metadata clone or provider is loaded.
    /// Root must admit the stores, boxed scheduler and construction headroom.
    /// Every fallible validation precedes exclusive software transfer.
    pub fn prepare(
        input: AssignedDisplayInput<'_>,
        profile: &AssignedDisplayProfile<'_>,
        scope: AssignedDisplayScope<'_>,
        software: &mut ReadySoftwareBacks,
        maximum_frames: u32,
        values: S,
        signs: L,
    ) -> Result<Self, AssignedDisplayRefusal> {
        use AssignedDisplayRefusal as E;
        if maximum_frames == 0
            || values.item_capacity() < 12
            || profile.maximum_value_bytes == 0
            || profile.maximum_value_bytes > 1024
            || values.byte_capacity() < 12 * profile.maximum_value_bytes
            || values.used_items() != 0
            || values.used_bytes() != 0
            || profile.sign_items == 0
            || profile.sign_items > 256
            || profile.sign_bytes > 256 * core::mem::size_of::<k::KernelEvent>() as u32
            || signs.item_capacity() < profile.sign_items
            || signs.byte_capacity() < profile.sign_bytes
            || !signs.is_empty()
            || signs.used_bytes() != 0
            || signs.ensure_remote_capacity(256).is_err()
        {
            return Err(E::Bounds);
        }
        let admitted = ValidatedAssignedDisplay::admit(input, profile, scope)?;
        let identity = admitted.identity();
        let selected = software
            .assigned_output(
                identity,
                scope.generation,
                profile.source,
                input.records,
                profile.write,
            )
            .map_err(E::Software)?;
        let tables = admitted.tables;
        let mut operations = selected.take();
        let mut selectors = core::array::from_fn(|_| None);
        let drivers = core::array::from_fn(|i| match operations[i].take() {
            Some(op) => {
                selectors[i] = op.selector;
                DisplayBack::Software(op.driver)
            }
            None => DisplayBack::Write(WriteBack::new(maximum_frames)),
        });
        let kernel = DisplayKernel::new_boxed_with_active_counts_and_host_calls(
            4,
            5,
            tables.nodes,
            tables.cords,
            tables.routes,
            tables.calls,
            drivers,
            values,
            signs,
        )
        .expect("preflighted immutable output tables");
        Ok(Self {
            identity,
            kernel,
            selectors,
            write: profile.write,
            input: admitted.frame,
            refusal: admitted.refusal,
        })
    }
}
