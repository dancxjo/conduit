//! Root's native fragment storage using the same finite production scheduler.
//! Complete workset admission and authorized Start remain caller obligations.
use alloc::boxed::Box;
use conduit_core as c;
use conduit_i2c_transaction::{
    contract::{I2cContract, I2C_MAXIMUM_BYTES},
    installation::{PreparedAssignedI2c, ReadyI2cBase},
    owner::{I2cCallRefusal, I2cHostCall},
    I2cProvider,
};
use conduit_kernel::{
    assigned::{BoundedAssignedKernelTables, ResidentLayout},
    scheduler::*,
    *,
};
use conduit_plan_lowering::{
    assigned_play_scope::AssignedPlayScope,
    selected_assigned_operation::{AssignedOperationInput, SelectedAssignedForeOperationPlan},
};
const SIGNS: usize = 16;
pub type NativeKernel = FixedScheduler<
    HostCallBack,
    FixedValueStore<2, { I2C_MAXIMUM_BYTES as usize }>,
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
type Storage = PreparedKernelStorage<HostCallBack, NativeKernel, 1>;
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    Scope,
    Bounds,
    Assignment,
    Native(I2cCallRefusal),
    Storage(KernelPreparationError),
}
/// Already-issued local possession, never reconstructed from assignment labels.
pub struct NativePossession<'a, P: I2cProvider> {
    pub slot: &'a mut Option<ReadyI2cBase<P>>,
    pub table: c::BaseCapabilityTable,
    pub handle: c::BaseCapabilityHandle,
    pub claim: c::BaseOperationClaim,
}
pub struct PreparedNativeKernel<'a, P: I2cProvider> {
    storage: Storage,
    native: PreparedAssignedI2c<'a, P>,
    bytes: usize,
}
pub struct BoundNativeKernel<P> {
    pub kernel: Box<NativeKernel>,
    pub call: I2cHostCall<P>,
}
impl<'a, P: I2cProvider> PreparedNativeKernel<'a, P> {
    /// One native node with exact local Fore. Preparation retains the provider;
    /// Root must charge its installation and inline/allocator storage separately.
    pub fn prepare(
        possession: NativePossession<'a, P>,
        contract: &I2cContract,
        selected: SelectedAssignedForeOperationPlan<'_>,
        maximum_bytes: usize,
    ) -> Result<Self, Refusal> {
        if matches!(selected.operation.scope, AssignedPlayScope::Plot(_))
            || selected.operation.node != NodeId(0)
        {
            return Err(Refusal::Scope);
        }
        if maximum_bytes < core::mem::size_of::<NativeKernel>() {
            return Err(Refusal::Bounds);
        }
        let AssignedOperationInput::Contiguous(bytes) = selected.operation.input else {
            return Err(Refusal::Bounds);
        };
        let (maxima, requirements, computations, fore) = (
            selected.operation.maxima,
            selected.operation.requirements,
            selected.operation.computations,
            selected.fore,
        );
        let native = ReadyI2cBase::prepare_assigned_fore_retained(
            possession.slot,
            possession.table,
            possession.handle,
            possession.claim,
            contract,
            selected,
        )
        .map_err(Refusal::Native)?;
        let view =
            c::decode_assigned_plan_with_fore(bytes, maxima, requirements, computations, fore)
                .map_err(|_| Refusal::Assignment)?;
        if view.counts[0] != 1
            || view.counts[usize::from(c::ASSIGNED_HOST_CALL - 1)] != 1
            || view.counts[usize::from(c::ASSIGNED_SIGN - 1)] as usize > SIGNS
        {
            return Err(Refusal::Bounds);
        }
        let mut pins = [c::AssignedRecordRequirement {
            tag: 0,
            payload_digest: [0; 32],
        }; 128];
        let mut count = 0;
        let schema = u16::from_le_bytes(bytes[8..10].try_into().map_err(|_| Refusal::Assignment)?);
        for record in c::AssignedPlanRecords::new(bytes).map_err(|_| Refusal::Assignment)? {
            let record = record.map_err(|_| Refusal::Assignment)?;
            if matches!(
                record.tag,
                c::ASSIGNED_CONFIGURATION | c::ASSIGNED_SIGN | c::ASSIGNED_TERMINAL
            ) {
                continue;
            }
            *pins.get_mut(count).ok_or(Refusal::Bounds)? =
                c::AssignedRecordRequirement::from_kernel_record(record, schema);
            count += 1;
        }
        let tables = BoundedAssignedKernelTables::<1, 2, 1, 1, 2, 1>::prepare_with_fore(
            bytes,
            maxima,
            requirements,
            computations,
            &ResidentLayout {
                records: &pins[..count],
                counts: view.counts,
            },
            fore,
        )
        .map_err(|_| Refusal::Assignment)?;
        let cords = tables.active_cords();
        if tables.cords[..cords]
            .iter()
            .any(|c| c.maximum_value_bytes > I2C_MAXIMUM_BYTES)
            || tables.cords[..cords]
                .iter()
                .map(|c| usize::from(c.item_capacity))
                .sum::<usize>()
                > 2
        {
            return Err(Refusal::Bounds);
        }
        let backing = native.retained_backing_bytes().ok_or(Refusal::Bounds)?;
        let remaining = maximum_bytes.checked_sub(backing).ok_or(Refusal::Bounds)?;
        let storage = NativeKernel::prepare_boxed(
            1,
            cords,
            tables.nodes,
            tables.cords,
            tables.routes,
            tables.calls,
            FixedValueStore::new(2 * I2C_MAXIMUM_BYTES).map_err(|_| Refusal::Bounds)?,
            FixedSignLog::new_with_remote_storage(
                (SIGNS * core::mem::size_of::<KernelEvent>()) as u32,
                SIGNS as u16,
                remote_sign_storage_bytes(SIGNS as u16).ok_or(Refusal::Bounds)?,
            )
            .map_err(|_| Refusal::Bounds)?,
            remaining,
        )
        .map_err(Refusal::Storage)?;
        let bytes = backing
            .checked_add(storage.storage_bytes())
            .ok_or(Refusal::Bounds)?;
        Ok(Self {
            storage,
            native,
            bytes,
        })
    }
    pub fn storage_bytes(&self) -> usize {
        self.bytes
    }
    /// Only after complete workset admission. No allocation, bus IO or Start.
    pub fn bind(self) -> BoundNativeKernel<P> {
        BoundNativeKernel {
            kernel: self.storage.bind([HostCallBack::new(I2C_MAXIMUM_BYTES)]),
            call: self.native.bind(),
        }
    }
}
