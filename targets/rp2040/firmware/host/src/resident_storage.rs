//! Attempt-owned kernel storage, followed by exact reserved Back binding.
#[path = "resident_binding.rs"]
mod binding;
use super::{CheckedDocument, ValidationRefusal as E};
use crate::resident::generated::{BorrowedBack, Initialized, PreparedResidentLoans, ENTRY_COUNT};
pub use binding::{BoundFragment, BoundKernel, BoundWorkset, CommittedWorkset, RetiredKernel};
use conduit_kernel::{scheduler::*, *};

// Explicit supported local representation. Larger signed envelopes refuse;
// construction and policy byte ceilings never grow to accommodate an assignment.
// Keep the installed logical Cord ceiling separate from physical Type storage.
const MAXIMUM_CORD_VALUE_BYTES: u32 = 2048;
const VALUE_BYTES: usize = {
    let prepared = crate::resident::generated::VALUE_SLOT_BYTES;
    let installed = MAXIMUM_CORD_VALUE_BYTES as usize;
    if prepared < installed {
        prepared
    } else {
        installed
    }
};
const SIGN_ITEMS: usize = 16;
type Kernel<'a, const N: usize, const C: usize, const V: usize> = FixedScheduler<
    BorrowedBack<'a>,
    FixedValueStore<V, VALUE_BYTES>,
    FixedSignLog<SIGN_ITEMS>,
    N,
    C,
    1,
    16,
    N,
    C,
    0,
    1,
>;
type Slot<'a, const N: usize, const C: usize, const V: usize> =
    PreparedKernelStorage<BorrowedBack<'a>, Kernel<'a, N, C, V>, N>;

// The selected package installs five unary protocol entries. Exact active-node
// specialization avoids charging every one-node fragment five nodes' storage.
enum Storage<'a> {
    One(Slot<'a, 1, 2, 2>),
    Two(Slot<'a, 2, 4, 4>),
    Three(Slot<'a, 3, 6, 6>),
    Four(Slot<'a, 4, 8, 8>),
    Five(Slot<'a, 5, 10, 10>),
}
struct Fragment<'a, 'o> {
    approved: conduit_body::DelegatedBodyFragment<'a>,
    _storage: Storage<'o>,
    entries: [u8; ENTRY_COUNT],
}
/// The exclusive inventory, original borrowed documents and actual unbound
/// allocations survive together. Cancellation frees only this preparation;
/// no resource reservation or provider loan has yet been taken.
pub struct AttemptStorage<'a, 'o> {
    _owner: PreparedResidentLoans<'o>,
    fragments: [Option<Fragment<'a, 'o>>; ENTRY_COUNT],
    bytes: usize,
    stamp: conduit_body::RetainedHostCommandStamp,
}
impl AttemptStorage<'_, '_> {
    pub fn storage_bytes(&self) -> usize {
        self.bytes
    }
}

/// Storage is destroyed before releasing this unstarted attempt's reservations.
/// No provider loan, native operation, capability possession or Start is implied.
pub struct ReservedAttempt<'a, 'o, 'r> {
    _storage: AttemptStorage<'a, 'o>,
    reservations: conduit_core::ResourceAdmissionScope<'r>,
    preparation_bytes: usize,
}
impl ReservedAttempt<'_, '_, '_> {
    /// End this unstarted feasibility attempt before retaining its input. No
    /// kernel or reservation is claimed installed after this transition.
    pub fn release_for_retention(self) -> conduit_body::RetainedHostCommandStamp {
        let Self {
            _storage,
            reservations,
            ..
        } = self;
        let AttemptStorage {
            fragments, stamp, ..
        } = _storage;
        drop(fragments);
        drop(reservations);
        stamp
    }
    pub fn admissions(&self) -> &[conduit_core::ResourceAdmission] {
        self.reservations.admissions()
    }
    pub fn preparation_bytes(&self) -> usize {
        self.preparation_bytes
    }
}
impl<'a, 'o> AttemptStorage<'a, 'o> {
    /// Admit the complete prepared set into the same retained ledger and bind
    /// immediately. No fallible work or await remains after atomic admission.
    /// The returned kernels neither start nor release their own reservations.
    pub fn admit_observed(
        self,
        owner: &mut conduit_core::ResourceAdmissionOwner,
        pool_id: &str,
        capacity: u32,
        sequence: u64,
        maximum_bytes: usize,
    ) -> Result<CommittedWorkset<'a, 'o>, conduit_body::DelegatedResourceRefusal> {
        let observed = super::resources::observe_private_work(
            &self,
            owner,
            pool_id,
            capacity,
            sequence,
            maximum_bytes
                .checked_sub(self.bytes)
                .ok_or(conduit_body::DelegatedResourceRefusal::PreparationCapacity)?,
        )?;
        let budget = maximum_bytes
            .checked_sub(observed.owned_bytes)
            .ok_or(conduit_body::DelegatedResourceRefusal::PreparationCapacity)?;
        self.admit_and_bind(owner, core::slice::from_ref(&observed.value), budget)
    }

    pub fn admit_and_bind(
        self,
        owner: &mut conduit_core::ResourceAdmissionOwner,
        observations: &[conduit_core::ResourceObservation],
        maximum_bytes: usize,
    ) -> Result<CommittedWorkset<'a, 'o>, conduit_body::DelegatedResourceRefusal> {
        let first = self
            .fragments
            .first()
            .and_then(Option::as_ref)
            .ok_or(conduit_body::DelegatedResourceRefusal::Scope)?;
        let mut fragments = [&first.approved; ENTRY_COUNT];
        let mut count = 0;
        for fragment in self.fragments.iter().flatten() {
            fragments[count] = &fragment.approved;
            count += 1;
        }
        let reservation_bytes = conduit_body::delegated_plain_resource_preparation_bytes(
            &fragments[..count],
            owner,
            observations,
        )?;
        self.bytes
            .checked_add(reservation_bytes)
            .filter(|bytes| *bytes <= maximum_bytes)
            .ok_or(conduit_body::DelegatedResourceRefusal::PreparationCapacity)?;
        conduit_body::admit_delegated_plain_resources(&fragments[..count], owner, observations)?;
        Ok(binding::bind_admitted(self))
    }

    /// Complete the synchronous observation/reservation transition before an
    /// async response can suspend. Snapshot and temporary tables do not become
    /// retained coroutine state; their requested bytes remain funded at peak.
    pub fn reserve_observed<'r>(
        self,
        owner: &'r mut conduit_core::ResourceAdmissionOwner,
        pool_id: &str,
        capacity: u32,
        sequence: u64,
        maximum_bytes: usize,
    ) -> Result<ReservedAttempt<'a, 'o, 'r>, conduit_body::DelegatedResourceRefusal> {
        let observed = super::resources::observe_private_work(
            &self,
            owner,
            pool_id,
            capacity,
            sequence,
            maximum_bytes
                .checked_sub(self.bytes)
                .ok_or(conduit_body::DelegatedResourceRefusal::PreparationCapacity)?,
        )?;
        let budget = maximum_bytes
            .checked_sub(observed.owned_bytes)
            .ok_or(conduit_body::DelegatedResourceRefusal::PreparationCapacity)?;
        self.reserve(owner, core::slice::from_ref(&observed.value), budget)
    }

    pub fn reserve<'r>(
        self,
        owner: &'r mut conduit_core::ResourceAdmissionOwner,
        observations: &[conduit_core::ResourceObservation],
        maximum_preparation_bytes: usize,
    ) -> Result<ReservedAttempt<'a, 'o, 'r>, conduit_body::DelegatedResourceRefusal> {
        let first = self
            .fragments
            .first()
            .and_then(Option::as_ref)
            .ok_or(conduit_body::DelegatedResourceRefusal::Scope)?;
        let mut fragments = [&first.approved; ENTRY_COUNT];
        let mut count = 0;
        for fragment in self.fragments.iter().flatten() {
            fragments[count] = &fragment.approved;
            count += 1;
        }
        let reservation_bytes = conduit_body::delegated_plain_resource_preparation_bytes(
            &fragments[..count],
            owner,
            observations,
        )?;
        let preparation_bytes = self
            .bytes
            .checked_add(reservation_bytes)
            .filter(|bytes| *bytes <= maximum_preparation_bytes)
            .ok_or(conduit_body::DelegatedResourceRefusal::PreparationCapacity)?;
        let reservations = conduit_body::reserve_delegated_plain_resources(
            &fragments[..count],
            owner,
            observations,
        )?;
        Ok(ReservedAttempt {
            _storage: self,
            reservations,
            preparation_bytes,
        })
    }
}

pub(super) fn prepare<'a, 'o>(
    owner: &'o mut Initialized,
    checked: [Option<CheckedDocument<'a>>; ENTRY_COUNT],
    stamp: conduit_body::RetainedHostCommandStamp,
    maximum_bytes: usize,
) -> Result<AttemptStorage<'a, 'o>, E> {
    // Hold the independently checked inventory exclusively through fallible
    // storage and Core reservation. No Back is withdrawn by this ticket.
    let selected = core::array::from_fn(|entry| {
        checked.iter().flatten().find_map(|checked| {
            checked.selected.nodes().find(|choice| choice.entry == entry).map(|choice| {
                let profile = checked.profile();
                conduit_plan_lowering::selected_assigned_expression::SelectedAssignedExpressionPlan {
                    input: conduit_plan_lowering::selected_assigned_operation::AssignedOperationInput::Contiguous(checked.fragment.projection()),
                    maxima: checked.maxima,
                    requirements: profile.requirements,
                    computations: profile.computations,
                    fore: profile.fore,
                    fragment_id: checked.fragment.metadata().fragment,
                    scope: conduit_plan_lowering::assigned_play_scope::AssignedPlayScope::Delegated(&checked.fragment),
                    node: NodeId(choice.node),
                    work_resource: 0,
                }
            })
        })
    });
    let prepared_loans = owner
        .prepare_selected_array(selected)
        .map_err(|_| E::Selection)?;
    let mut output = core::array::from_fn(|_| None);
    let mut bytes = 0usize;
    for (target, checked) in output.iter_mut().zip(checked) {
        let Some(checked) = checked else { continue };
        let remaining = maximum_bytes.checked_sub(bytes).ok_or(E::Bounds)?;
        let (storage, charge) = match checked.fragment.view().counts[0] {
            1 => {
                let s = slot::<1, 2, 2>(&checked, remaining)?;
                let n = s.storage_bytes();
                (Storage::One(s), n)
            }
            2 => {
                let s = slot::<2, 4, 4>(&checked, remaining)?;
                let n = s.storage_bytes();
                (Storage::Two(s), n)
            }
            3 => {
                let s = slot::<3, 6, 6>(&checked, remaining)?;
                let n = s.storage_bytes();
                (Storage::Three(s), n)
            }
            4 => {
                let s = slot::<4, 8, 8>(&checked, remaining)?;
                let n = s.storage_bytes();
                (Storage::Four(s), n)
            }
            5 => {
                let s = slot::<5, 10, 10>(&checked, remaining)?;
                let n = s.storage_bytes();
                (Storage::Five(s), n)
            }
            _ => return Err(E::Bounds),
        };
        bytes = bytes.checked_add(charge).ok_or(E::Bounds)?;
        *target = Some(Fragment {
            entries: core::array::from_fn(|node| {
                checked
                    .selected
                    .nodes()
                    .find(|choice| usize::from(choice.node) == node)
                    .map(|choice| choice.entry as u8)
                    .unwrap_or(u8::MAX)
            }),
            approved: checked.fragment,
            _storage: storage,
        });
    }
    Ok(AttemptStorage {
        _owner: prepared_loans,
        fragments: output,
        bytes,
        stamp,
    })
}
fn slot<'o, const N: usize, const C: usize, const V: usize>(
    checked: &CheckedDocument<'_>,
    maximum_bytes: usize,
) -> Result<Slot<'o, N, C, V>, E> {
    let tables = conduit_plan_lowering::delegated_kernel::prepare_delegated_body_kernel_tables::<
        128,
        N,
        C,
        1,
        N,
        C,
        0,
    >(&checked.fragment, checked.profile())
    .map_err(|_| E::Kernel)?;
    let (nodes, cords) = (tables.active_nodes(), tables.active_cords());
    if nodes != N
        || checked.selected.nodes().count() != N
        || tables.cords[..cords]
            .iter()
            .any(|c| c.maximum_value_bytes > MAXIMUM_CORD_VALUE_BYTES)
        || tables.cords[..cords]
            .iter()
            .map(|c| usize::from(c.item_capacity))
            .sum::<usize>()
            > V
        || checked.fragment.view().counts[8] as usize > SIGN_ITEMS
    {
        return Err(E::Bounds);
    }
    Kernel::<N, C, V>::prepare_boxed(
        nodes,
        cords,
        tables.nodes,
        tables.cords,
        tables.routes,
        tables.calls,
        FixedValueStore::new((V * VALUE_BYTES) as u32).map_err(|_| E::Bounds)?,
        FixedSignLog::new_with_remote_storage(
            (SIGN_ITEMS * core::mem::size_of::<KernelEvent>()) as u32,
            SIGN_ITEMS as u16,
            remote_sign_storage_bytes(SIGN_ITEMS as u16).ok_or(E::Bounds)?,
        )
        .map_err(|_| E::Bounds)?,
        maximum_bytes,
    )
    .map_err(E::Storage)
}
