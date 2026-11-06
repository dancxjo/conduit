//! Exact Back loans into already acquired, complete Core-reserved storage.
//! This prepares executable kernels; it does not publish or authorize Start.
use super::*;
use alloc::boxed::Box;
#[path = "resident_retirement.rs"]
mod retirement;
pub use retirement::RetiredKernel;

pub enum BoundKernel<'o> {
    One(Box<Kernel<'o, 1, 2, 2>>),
    Two(Box<Kernel<'o, 2, 4, 4>>),
    Three(Box<Kernel<'o, 3, 6, 6>>),
    Four(Box<Kernel<'o, 4, 8, 8>>),
    Five(Box<Kernel<'o, 5, 10, 10>>),
}
impl BoundKernel<'_> {
    /// One ordinary bounded scheduler decision, never a second execution loop.
    pub fn step(&mut self) -> Result<SchedulerStatus, SchedulerError> {
        match self {
            Self::One(kernel) => kernel.step(),
            Self::Two(kernel) => kernel.step(),
            Self::Three(kernel) => kernel.step(),
            Self::Four(kernel) => kernel.step(),
            Self::Five(kernel) => kernel.step(),
        }
    }
}
pub struct BoundFragment<'a, 'o> {
    pub approved: conduit_body::DelegatedBodyFragment<'a>,
    pub kernel: BoundKernel<'o>,
}
/// Field order destroys kernels before releasing their exact resource scope.
/// Dropping without explicit kernel retirement leaves provider offers withdrawn.
pub struct BoundWorkset<'a, 'o, 'r> {
    fragments: [Option<BoundFragment<'a, 'o>>; ENTRY_COUNT],
    reservations: conduit_core::ResourceAdmissionScope<'r>,
    stamp: conduit_body::RetainedHostCommandStamp,
}
/// The same original ledger retains reservations after preparation returns.
/// Dropping kernels cannot free them or republish unretired Backs. Root must
/// retain that ledger and the original command, then retire before release.
pub struct CommittedWorkset<'a, 'o> {
    fragments: [Option<BoundFragment<'a, 'o>>; ENTRY_COUNT],
    stamp: conduit_body::RetainedHostCommandStamp,
}
impl<'a, 'o> CommittedWorkset<'a, 'o> {
    pub fn into_parts(
        self,
    ) -> (
        [Option<BoundFragment<'a, 'o>>; ENTRY_COUNT],
        conduit_body::RetainedHostCommandStamp,
    ) {
        (self.fragments, self.stamp)
    }
}
impl<'a, 'o, 'r> BoundWorkset<'a, 'o, 'r> {
    pub fn admissions(&self) -> &[conduit_core::ResourceAdmission] {
        self.reservations.admissions()
    }
    /// Trusted Root must retain terminal evidence and retire every kernel before
    /// republishing Backs and releasing this resource scope. No Start is implied.
    pub fn into_parts(
        self,
    ) -> (
        [Option<BoundFragment<'a, 'o>>; ENTRY_COUNT],
        conduit_core::ResourceAdmissionScope<'r>,
        conduit_body::RetainedHostCommandStamp,
    ) {
        (self.fragments, self.reservations, self.stamp)
    }
}
impl<'a, 'o, 'r> ReservedAttempt<'a, 'o, 'r> {
    /// All assignment/storage/resource failures are over. Borrow each exact
    /// initialized entry once, then initialize the acquired kernel's last field.
    /// Root must still arrange authorized Start and current ingress/Line custody.
    pub fn bind(self) -> BoundWorkset<'a, 'o, 'r> {
        let Self {
            _storage,
            reservations,
            ..
        } = self;
        let (fragments, stamp) = bind_storage(_storage);
        BoundWorkset {
            fragments,
            reservations,
            stamp,
        }
    }
}
/// Called only after atomic admission succeeds; all remaining work is infallible.
pub(super) fn bind_admitted<'a, 'o>(storage: AttemptStorage<'a, 'o>) -> CommittedWorkset<'a, 'o> {
    let (fragments, stamp) = bind_storage(storage);
    CommittedWorkset { fragments, stamp }
}
fn bind_storage<'a, 'o>(
    _storage: AttemptStorage<'a, 'o>,
) -> (
    [Option<BoundFragment<'a, 'o>>; ENTRY_COUNT],
    conduit_body::RetainedHostCommandStamp,
) {
    let AttemptStorage {
        _owner,
        fragments,
        stamp,
        ..
    } = _storage;
    let mut loans = _owner.bind();
    let fragments = fragments.map(|fragment| {
        fragment.map(|fragment| {
            let Fragment {
                approved,
                _storage,
                entries,
            } = fragment;
            let kernel = match _storage {
                Storage::One(slot) => BoundKernel::One(bind(slot, &entries, &mut loans)),
                Storage::Two(slot) => BoundKernel::Two(bind(slot, &entries, &mut loans)),
                Storage::Three(slot) => BoundKernel::Three(bind(slot, &entries, &mut loans)),
                Storage::Four(slot) => BoundKernel::Four(bind(slot, &entries, &mut loans)),
                Storage::Five(slot) => BoundKernel::Five(bind(slot, &entries, &mut loans)),
            };
            BoundFragment { approved, kernel }
        })
    });
    debug_assert!(loans.iter().all(Option::is_none));
    (fragments, stamp)
}
fn bind<'o, const N: usize, const C: usize, const V: usize>(
    slot: Slot<'o, N, C, V>,
    entries: &[u8; ENTRY_COUNT],
    loans: &mut [Option<(u16, BorrowedBack<'o>)>; ENTRY_COUNT],
) -> Box<Kernel<'o, N, C, V>> {
    let drivers = core::array::from_fn(|node| {
        let (actual, back) = loans[usize::from(entries[node])]
            .take()
            .expect("complete checked workset owns each exact Back once");
        debug_assert_eq!(usize::from(actual), node);
        back
    });
    slot.bind(drivers)
}
