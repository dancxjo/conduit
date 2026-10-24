//! Compact assignment -> retained software -> unchanged ordinary output kernel.
//! Root separately owns admission, Fore value contracts and native possession.
use super::*;
use crate::assigned::{AssignedKernelRefusal, AssignedKernelTables, ResidentLayout};
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
    pub sign_items: u16,
    pub sign_bytes: u32,
}
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
type Tables = AssignedKernelTables<4, 5, 1, 5, 5, 4>;
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
            || values.byte_capacity() < 12 * 1024
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
        if profile.fore.len() != 2 || profile.write.0 >= 4 || scope.generation.0 == 0 {
            return Err(E::Shape);
        }
        let fore = |name: &str, direction| {
            profile
                .fore
                .iter()
                .find(|f| {
                    f.direction() == direction
                        && f.payload()[5..21] == c::AssignedIdentity::from_text(name).0
                })
                .copied()
                .ok_or(E::Shape)
        };
        let frame = fore("frame", c::AssignedForeDirection::Input)?;
        let refusal = fore("invalid_brightness", c::AssignedForeDirection::Output)?;
        let required = c::AssignedPlanRequirements {
            host: c::AssignedIdentity::from_text(scope.host.as_str()),
            boot: c::AssignedIdentity::from_text(scope.boot.as_str()),
            host_calls: profile.host_calls,
            resources: profile.resources,
            remote_bindings: &[],
        };
        let tables = || {
            Tables::prepare_records_with_fore(
                input.header,
                input.records,
                input.maxima,
                required,
                profile.computations,
                &profile.layout,
                profile.fore,
            )
            .map_err(E::Assignment)
        };
        let first = tables()?;
        let identity = first.identity;
        if identity.counts[6] != 3
            || identity.counts[7] != 1
            || first.cords.iter().any(|c| {
                c.item_capacity != 1 || c.byte_capacity > 1024 || c.maximum_value_bytes > 1024
            })
        {
            return Err(E::Shape);
        }
        let mut write_calls = input.records.iter().filter(|r| {
            r.tag == c::ASSIGNED_HOST_CALL
                && r.payload.get(..2) == Some(&profile.write.0.to_le_bytes())
        });
        let call = write_calls.next().ok_or(E::Shape)?.payload;
        if write_calls.next().is_some()
            || call.len() != c::ASSIGNED_NATIVE_CALL_BYTES
            || call[4..20] != c::AssignedIdentity::from_text(conduit_tm1637_wire::WRITE_CALL).0
            || c::assigned_native_binding(call)
                .map_err(|_| E::Shape)?
                .is_none()
            || !call[2..4].iter().all(|b| *b == 0)
            || call[36..38] != 1u16.to_le_bytes()
            || call[38..42] != 1024u32.to_le_bytes()
            || call[42..46] != 0u32.to_le_bytes()
        {
            return Err(E::Shape);
        }
        type Check = FixedScheduler<
            NoStep,
            k::FixedValueStore<1, 1>,
            k::FixedSignLog<1>,
            4,
            5,
            1,
            5,
            5,
            5,
            4,
            4,
        >;
        Check::new_with_host_calls(
            first.nodes,
            first.cords,
            first.routes,
            first.calls,
            core::array::from_fn(|_| NoStep),
            k::FixedValueStore::new(1).map_err(|_| E::Bounds)?,
            k::FixedSignLog::new(core::mem::size_of::<k::KernelEvent>() as u32)
                .map_err(|_| E::Bounds)?,
        )
        .map_err(|_| E::Tables)?;
        let selected = software
            .assigned_output(
                identity,
                scope.generation,
                profile.source,
                input.records,
                profile.write,
            )
            .map_err(E::Software)?;
        // Rebuild the identical finite tables after consuming the unstepped
        // numeric preflight. Immutable borrowed records cannot change in between.
        let tables = tables()?;
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
            input: frame,
            refusal,
        })
    }
}
