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
    maximum_frames: u32,
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
    pub fn prepare<'a, 'm: 'a>(
        input: AssignedDisplayInput<'_>,
        profile: &AssignedDisplayProfile<'_>,
        scope: AssignedDisplayScope<'_>,
        software: impl Into<OutputSoftware<'a, 'm>>,
        maximum_frames: u32,
        values: S,
        signs: L,
    ) -> Result<Self, AssignedDisplayRefusal> {
        Self::prepare_bounded(
            input,
            profile,
            scope,
            software,
            maximum_frames,
            maximum_frames,
            values,
            signs,
        )
    }
    /// Prepare one bounded Value invocation at a time while retaining the total
    /// electrical work ceiling. The session owner must transfer complete evidence
    /// before rearm; this does not authorize a caller, output provider or Play.
    /// Reserve the full per-invocation trace, not just the ordinary retained-sign
    /// minimum: any retention gap explicitly prevents session rearm.
    #[cfg(feature = "display-session")]
    pub fn prepare_for_session<'a, 'm: 'a>(
        input: AssignedDisplayInput<'_>,
        profile: &AssignedDisplayProfile<'_>,
        scope: AssignedDisplayScope<'_>,
        software: impl Into<OutputSoftware<'a, 'm>>,
        maximum_frames: u32,
        values: S,
        signs: L,
    ) -> Result<Self, AssignedDisplayRefusal> {
        Self::prepare_bounded(
            input,
            profile,
            scope,
            software,
            maximum_frames,
            1,
            values,
            signs,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn prepare_bounded<'a, 'm: 'a>(
        input: AssignedDisplayInput<'_>,
        profile: &AssignedDisplayProfile<'_>,
        scope: AssignedDisplayScope<'_>,
        software: impl Into<OutputSoftware<'a, 'm>>,
        maximum_frames: u32,
        invocation_frames: u32,
        values: S,
        signs: L,
    ) -> Result<Self, AssignedDisplayRefusal> {
        use AssignedDisplayRefusal as E;
        let sign_budget =
            DisplaySignBudget::derive(invocation_frames, profile.sign_items, profile.sign_bytes)
                .map_err(|_| E::Bounds)?;
        let value_budget =
            DisplayValueBudget::derive(invocation_frames, profile.maximum_value_bytes)
                .map_err(|_| E::Bounds)?;
        if maximum_frames == 0
            || invocation_frames > maximum_frames
            || values.item_capacity() < value_budget.items()
            || profile.maximum_value_bytes == 0
            || profile.maximum_value_bytes > 1024
            || values.byte_capacity() < value_budget.bytes()
            || values.used_items() != 0
            || values.used_bytes() != 0
            || profile.sign_items == 0
            || profile.sign_items > 256
            || profile.sign_bytes > 256 * core::mem::size_of::<k::KernelEvent>() as u32
            || signs.item_capacity() < sign_budget.ordinary_items()
            || signs.byte_capacity() < sign_budget.ordinary_bytes()
            || !signs.is_empty()
            || signs.used_bytes() != 0
            || signs
                .ensure_remote_capacity(sign_budget.remote_items())
                .is_err()
        {
            return Err(E::Bounds);
        }
        let admitted = ValidatedAssignedDisplay::admit(input, profile, scope)?;
        let identity = admitted.identity();
        let mut software = software.into();
        let operations = software
            .take_assigned(
                identity,
                scope.generation,
                profile.source,
                input.records,
                profile.write,
            )
            .map_err(E::Software)?;
        let tables = admitted.tables;
        let mut operations = operations;
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
            maximum_frames,
            identity,
            kernel,
            selectors,
            write: profile.write,
            input: admitted.frame,
            refusal: admitted.refusal,
        })
    }
    pub fn maximum_frames(&self) -> u32 {
        self.maximum_frames
    }
}

/// Only these retained, Source-checked owners can supply output computations.
pub enum OutputSoftware<'a, 'm> {
    Owned(&'a mut ReadySoftwareBacks),
    Compact(&'a mut crate::appliance::software_compact::CompactOutputSoftware<'m>),
}
impl<'a, 'm> From<&'a mut ReadySoftwareBacks> for OutputSoftware<'a, 'm> {
    fn from(value: &'a mut ReadySoftwareBacks) -> Self {
        Self::Owned(value)
    }
}
impl<'a, 'm> From<&'a mut crate::appliance::software_compact::CompactOutputSoftware<'m>>
    for OutputSoftware<'a, 'm>
{
    fn from(value: &'a mut crate::appliance::software_compact::CompactOutputSoftware<'m>) -> Self {
        Self::Compact(value)
    }
}
impl OutputSoftware<'_, '_> {
    fn take_assigned(
        &mut self,
        identity: c::AssignedPlanView,
        generation: c::OfferGeneration,
        source: &c::PlotIdentity,
        records: &[c::AssignedRecord<'_>],
        write: k::NodeId,
    ) -> Result<[Option<crate::appliance::software::PreparedSoftwareOperation>; 4], SoftwareRefusal>
    {
        match self {
            Self::Owned(owner) => owner
                .assigned_output(identity, generation, source, records, write)
                .map(|selected| selected.take()),
            Self::Compact(owner) => {
                owner.take_assigned(identity, generation, source, records, write)
            }
        }
    }
}
