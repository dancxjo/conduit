//! One native operation and one resident computation in the production kernel.
//! Preparation retains both owners; complete workset admission precedes bind.
use crate::{
    native_preparation::{self as native, NativePossession, Refusal},
    resident::generated,
};
use alloc::boxed::Box;
use conduit_core as c;
use conduit_i2c_transaction::{
    contract::{I2cContract, I2C_MAXIMUM_BYTES, I2C_VALUE_SLOT_BYTES},
    installation::{PreparedAssignedI2c, ReadyI2cBase},
    owner::I2cHostCall,
    I2cProvider,
};
use conduit_kernel::{scheduler::*, *};
use conduit_plan_lowering::{
    assigned_play_scope::AssignedPlayScope,
    selected_assigned_expression::SelectedAssignedExpressionPlan,
    selected_assigned_operation::{AssignedOperationInput, SelectedAssignedForeOperationPlan},
};

const SIGNS: usize = 16;
#[path = "mixed_retirement.rs"]
mod retirement;
pub use retirement::RetiredMixedKernel;
pub enum MixedBack<'a> {
    Resident(generated::BorrowedBack<'a>),
    Native(HostCallBack),
}
impl<const P: usize> StepBack<P> for MixedBack<'_> {
    fn step(&mut self, io: &mut StepIo<P>, input: &StepInputBytes<'_, P>) -> StepOutcome {
        match self {
            Self::Resident(back) => back.step(io, input),
            Self::Native(back) => back.step(io, input),
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Resident(back) => StepBack::<P>::prepared_output(back, port),
            Self::Native(back) => StepBack::<P>::prepared_output(back, port),
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Resident(back) => StepBack::<P>::step_committed(back),
            Self::Native(back) => StepBack::<P>::step_committed(back),
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Resident(back) => StepBack::<P>::cancel(back),
            Self::Native(back) => StepBack::<P>::cancel(back),
        }
    }
}
pub type MixedKernel<'a> =
    FixedScheduler<MixedBack<'a>, HostedValueStore, FixedSignLog<SIGNS>, 2, 3, 1, 16, 2, 3, 2, 1>;
type Storage<'a> = PreparedKernelStorage<MixedBack<'a>, MixedKernel<'a>, 2>;
pub struct PreparedMixedKernel<'a, 'o, P: I2cProvider> {
    storage: Storage<'o>,
    residents: generated::PreparedResidentLoans<'o>,
    native: PreparedAssignedI2c<'a, P>,
    bytes: usize,
}
pub struct BoundMixedKernel<'a, P> {
    pub kernel: Box<MixedKernel<'a>>,
    pub call: I2cHostCall<P>,
}
impl<P: I2cProvider> BoundMixedKernel<'_, P> {
    /// Root independently authorizes Start; this never advances the scheduler.
    pub fn dispatch_one(&mut self) -> Result<native::NativeDispatch, SchedulerError> {
        native::dispatch_one(&mut self.kernel, &mut self.call)
    }
}
impl<'a, 'o, P: I2cProvider> PreparedMixedKernel<'a, 'o, P> {
    /// Exact two-node representation, with one independently retained native
    /// identity and one actual available resident. This grants no Start or IO.
    pub fn prepare(
        owner: &'o mut generated::Initialized,
        possession: NativePossession<'a, P>,
        contract: &I2cContract,
        selected: SelectedAssignedForeOperationPlan<'_>,
        work_resource: u16,
        maximum_bytes: usize,
    ) -> Result<Self, Refusal> {
        if matches!(selected.operation.scope, AssignedPlayScope::Plot(_))
            || selected.operation.node.0 >= 2
        {
            return Err(Refusal::Scope);
        }
        if maximum_bytes < core::mem::size_of::<MixedKernel<'o>>() {
            return Err(Refusal::Bounds);
        }
        let AssignedOperationInput::Contiguous(bytes) = selected.operation.input else {
            return Err(Refusal::Bounds);
        };
        let native_id = possession
            .slot
            .as_ref()
            .ok_or(Refusal::Assignment)?
            .assigned_node_identity(selected.operation.node.0);
        let residents = owner
            .identify_mixed_assignments(bytes, &[native_id])
            .map_err(Refusal::Resident)?;
        if residents.nodes().count() != 1 {
            return Err(Refusal::Bounds);
        }
        let entry = residents.nodes().next().ok_or(Refusal::Bounds)?.entry;
        let value_bytes = owner
            .available_value_slot_bytes(entry)
            .ok_or(Refusal::Bounds)?
            .max(I2C_VALUE_SLOT_BYTES);
        let value_bytes = u32::try_from(value_bytes).map_err(|_| Refusal::Bounds)?;
        let computations = c::AssignedComputationProfile {
            maximum_text_bytes: residents
                .computations()
                .iter()
                .map(|p| p.text_bytes)
                .max()
                .ok_or(Refusal::Bounds)?,
            maximum_total_text_bytes: residents
                .computations()
                .iter()
                .try_fold(0u32, |sum, p| sum.checked_add(p.text_bytes))
                .ok_or(Refusal::Bounds)?,
            requirements: residents.computations(),
        };
        // Rebuild the profile from local initialized facts; incoming computation
        // identities do not establish the resident owner's selected program.
        let native_selected = SelectedAssignedForeOperationPlan {
            operation:
                conduit_plan_lowering::selected_assigned_operation::SelectedAssignedOperationPlan {
                    input: selected.operation.input,
                    maxima: selected.operation.maxima,
                    requirements: selected.operation.requirements,
                    computations,
                    fragment_id: selected.operation.fragment_id,
                    scope: selected.operation.scope,
                    node: selected.operation.node,
                },
            fore: selected.fore,
        };
        let selections = core::array::from_fn(|entry| {
            residents
                .nodes()
                .find(|n| n.entry == entry)
                .map(|n| SelectedAssignedExpressionPlan {
                    input: selected.operation.input,
                    maxima: selected.operation.maxima,
                    requirements: selected.operation.requirements,
                    computations,
                    fore: Some(selected.fore),
                    fragment_id: selected.operation.fragment_id,
                    scope: selected.operation.scope,
                    node: NodeId(n.node),
                    work_resource,
                })
        });
        let prepared_residents = owner
            .prepare_selected_array(selections)
            .map_err(Refusal::Resident)?;
        let prepared_native = ReadyI2cBase::prepare_assigned_fore_retained(
            possession.slot,
            possession.table,
            possession.handle,
            possession.claim,
            contract,
            native_selected,
        )
        .map_err(Refusal::Native)?;
        let view = c::decode_assigned_plan_with_fore(
            bytes,
            selected.operation.maxima,
            selected.operation.requirements,
            computations,
            selected.fore,
        )
        .map_err(|_| Refusal::Assignment)?;
        if view.counts[0] != 2
            || view.counts[usize::from(c::ASSIGNED_HOST_CALL - 1)] != 1
            || view.counts[usize::from(c::ASSIGNED_SIGN - 1)] as usize > SIGNS
        {
            return Err(Refusal::Bounds);
        }
        let tables = native::tables::prepare::<2, 3>(
            bytes,
            selected.operation.maxima,
            selected.operation.requirements,
            computations,
            selected.fore,
        )?;
        let cords = tables.active_cords();
        if tables.cords[..cords]
            .iter()
            .any(|c| c.maximum_value_bytes > I2C_MAXIMUM_BYTES)
            || tables.cords[..cords]
                .iter()
                .map(|c| usize::from(c.item_capacity))
                .sum::<usize>()
                > 3
        {
            return Err(Refusal::Bounds);
        }
        let backing = prepared_native
            .retained_backing_bytes()
            .ok_or(Refusal::Bounds)?;
        let remaining = maximum_bytes
            .checked_sub(backing)
            .and_then(|n| n.checked_sub(core::mem::size_of::<MixedKernel<'o>>()))
            .ok_or(Refusal::Bounds)?;
        let values = HostedValueStore::try_new_with_slot_capacities(
            &[value_bytes; 3],
            value_bytes.checked_mul(3).ok_or(Refusal::Bounds)?,
            remaining,
        )
        .map_err(Refusal::Values)?;
        let backing = backing
            .checked_add(values.reserved_storage_bytes())
            .ok_or(Refusal::Bounds)?;
        let storage = MixedKernel::prepare_boxed(
            2,
            cords,
            tables.nodes,
            tables.cords,
            tables.routes,
            tables.calls,
            values,
            FixedSignLog::new_with_remote_storage(
                (SIGNS * core::mem::size_of::<KernelEvent>()) as u32,
                SIGNS as u16,
                remote_sign_storage_bytes(SIGNS as u16).ok_or(Refusal::Bounds)?,
            )
            .map_err(|_| Refusal::Bounds)?,
            maximum_bytes.checked_sub(backing).ok_or(Refusal::Bounds)?,
        )
        .map_err(Refusal::Storage)?;
        let bytes = backing
            .checked_add(storage.storage_bytes())
            .ok_or(Refusal::Bounds)?;
        Ok(Self {
            storage,
            residents: prepared_residents,
            native: prepared_native,
            bytes,
        })
    }
    pub fn storage_bytes(&self) -> usize {
        self.bytes
    }
    /// Root must have admitted the entire workset before calling. Exact checked
    /// loans bind without allocation; no controller IO, Start or grant occurs.
    pub fn bind(self) -> BoundMixedKernel<'o, P> {
        let native_node = self.native.node();
        let mut loans = self.residents.bind();
        let drivers = core::array::from_fn(|node| {
            if node == usize::from(native_node.0) {
                MixedBack::Native(HostCallBack::new(I2C_MAXIMUM_BYTES))
            } else {
                let loan = loans
                    .iter_mut()
                    .find(|loan| loan.as_ref().is_some_and(|(n, _)| usize::from(*n) == node))
                    .expect("checked dense mixed resident node");
                MixedBack::Resident(loan.take().expect("checked exclusive resident loan").1)
            }
        });
        debug_assert!(loans.iter().all(Option::is_none));
        BoundMixedKernel {
            kernel: self.storage.bind(drivers),
            call: self.native.bind(),
        }
    }
}
