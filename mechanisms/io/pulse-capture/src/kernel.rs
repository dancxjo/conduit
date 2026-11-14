//! Bounded native pulse fragment on the ordinary production scheduler.
//! Preparation is not whole-workset admission, authorized Start or hardware IO.
use crate::{
    installation::{PreparedAssignedPulse, ReadyPulseBase},
    owner::{PulseCallRefusal, PulseHostCall, PulseReadProvider},
    PulseReadContract, PULSE_READ_MAXIMUM_BYTES,
};
use alloc::boxed::Box;
use conduit_core as c;
use conduit_kernel::{
    assigned::{BoundedAssignedKernelTables, ResidentLayout},
    scheduler::*,
    *,
};
use conduit_plan_lowering::selected_assigned_operation::{
    AssignedOperationInput, SelectedAssignedForeOperationPlan,
};
const SIGNS: usize = 16;
pub type PulseKernel = FixedScheduler<
    HostCallBack,
    FixedValueStore<2, { PULSE_READ_MAXIMUM_BYTES as usize }>,
    FixedSignLog<SIGNS>,
    1,
    2,
    1,
    16,
    1,
    2,
    1,
    1,
>;
type Storage = PreparedKernelStorage<HostCallBack, PulseKernel, 1>;
#[derive(Debug, PartialEq, Eq)]
pub enum PulseKernelRefusal {
    Scope,
    Bounds,
    Assignment,
    Tables(conduit_kernel::assigned::AssignedKernelRefusal),
    Native(PulseCallRefusal),
    Storage(KernelPreparationError),
}
/// Already-issued local possession. Assignment labels cannot create this table.
pub struct PulsePossession<'a, P: PulseReadProvider> {
    pub slot: &'a mut Option<ReadyPulseBase<P>>,
    pub table: c::BaseCapabilityTable,
    pub handle: c::BaseCapabilityHandle,
    pub claim: c::BaseOperationClaim,
}
pub struct PreparedPulseKernel<'a, P: PulseReadProvider> {
    storage: Storage,
    native: PreparedAssignedPulse<'a, P>,
    bytes: usize,
}
pub struct BoundPulseKernel<P> {
    pub kernel: Box<PulseKernel>,
    pub call: PulseHostCall<P>,
}
impl<'a, P: PulseReadProvider> PreparedPulseKernel<'a, P> {
    /// The resident layout and Fore pins must be retained independently from the
    /// incoming assignment. Their comparison grants no capability or Start.
    /// The limit bounds successful retained scheduler storage and reachable
    /// codec/table/claim backing. Codec preparation allocates before its final
    /// comparison: callers must independently admit preparation peak workspace.
    /// Root separately funds inline state, temporary decode tables,
    /// allocator overhead, retained contract and codec preparation peak, installed
    /// provider, and the rest of the workset.
    pub fn prepare(
        possession: PulsePossession<'a, P>,
        contract: &PulseReadContract,
        selected: SelectedAssignedForeOperationPlan<'_>,
        layout: &ResidentLayout<'_>,
        maximum_bytes: usize,
    ) -> Result<Self, PulseKernelRefusal> {
        use PulseKernelRefusal as R;
        if selected.operation.node != NodeId(0) {
            return Err(R::Scope);
        }
        if maximum_bytes < core::mem::size_of::<PulseKernel>() {
            return Err(R::Bounds);
        }
        let AssignedOperationInput::Contiguous(bytes) = selected.operation.input else {
            return Err(R::Bounds);
        };
        let (maxima, requirements, computations, fore) = (
            selected.operation.maxima,
            selected.operation.requirements,
            selected.operation.computations,
            selected.fore,
        );
        let view =
            c::decode_assigned_plan_with_fore(bytes, maxima, requirements, computations, fore)
                .map_err(|_| R::Assignment)?;
        if view.counts[0] != 1
            || view.counts[usize::from(c::ASSIGNED_HOST_CALL - 1)] != 1
            || view.counts[usize::from(c::ASSIGNED_SIGN - 1)] as usize > SIGNS
        {
            return Err(R::Bounds);
        }
        let tables = BoundedAssignedKernelTables::<1, 2, 1, 1, 2, 1>::prepare_with_fore(
            bytes,
            maxima,
            requirements,
            computations,
            layout,
            fore,
        )
        .map_err(R::Tables)?;
        let cords = tables.active_cords();
        if tables.cords[..cords]
            .iter()
            .any(|cord| cord.maximum_value_bytes > PULSE_READ_MAXIMUM_BYTES)
            || tables.cords[..cords]
                .iter()
                .map(|cord| usize::from(cord.item_capacity))
                .sum::<usize>()
                > 2
        {
            return Err(R::Bounds);
        }
        let native = ReadyPulseBase::prepare_assigned_fore_retained(
            possession.slot,
            possession.table,
            possession.handle,
            possession.claim,
            contract,
            selected,
        )
        .map_err(R::Native)?;
        let backing = native.retained_backing_bytes().ok_or(R::Bounds)?;
        let remaining = maximum_bytes.checked_sub(backing).ok_or(R::Bounds)?;
        let storage = PulseKernel::prepare_boxed(
            1,
            cords,
            tables.nodes,
            tables.cords,
            tables.routes,
            tables.calls,
            FixedValueStore::new(2 * PULSE_READ_MAXIMUM_BYTES).map_err(|_| R::Bounds)?,
            FixedSignLog::new_with_remote_storage(
                (SIGNS * core::mem::size_of::<KernelEvent>()) as u32,
                SIGNS as u16,
                remote_sign_storage_bytes(SIGNS as u16).ok_or(R::Bounds)?,
            )
            .map_err(|_| R::Bounds)?,
            remaining,
        )
        .map_err(R::Storage)?;
        let bytes = backing
            .checked_add(storage.storage_bytes())
            .ok_or(R::Bounds)?;
        Ok(Self {
            storage,
            native,
            bytes,
        })
    }
    pub fn storage_bytes(&self) -> usize {
        self.bytes
    }
    /// Root independently completes admission and retains lawful Start/retirement.
    /// Transfer allocates nothing and performs no capture or scheduler advance.
    pub fn bind(self) -> BoundPulseKernel<P> {
        BoundPulseKernel {
            kernel: self
                .storage
                .bind([HostCallBack::new(PULSE_READ_MAXIMUM_BYTES)]),
            call: self.native.bind(),
        }
    }
}
