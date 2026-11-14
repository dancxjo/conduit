use super::*;
use core::cell::Cell;
const REQUIREMENTS: conduit_kernel::scheduler::StepWorkspaceRequirements =
    conduit_kernel::scheduler::StepWorkspaceRequirements {
        output_bytes: 12,
        scratch_bytes: 4,
    };
struct Back;
impl StepBack<1> for Back {
    fn workspace_requirements(
        &self,
    ) -> Option<conduit_kernel::scheduler::StepWorkspaceRequirements> {
        Some(REQUIREMENTS)
    }
    fn step(&mut self, _: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        StepOutcome::Await
    }
}
struct Residents<'a>(&'a Cell<bool>);
// SAFETY: exact immutable one-slot owner report; no allocation on bind.
unsafe impl PreparedResidents<1> for Residents<'_> {
    type Back = Back;
    fn nodes(&self) -> [Option<u16>; 1] {
        [Some(0)]
    }
    fn execution_workspace_requirements_at<const P: usize>(
        &self,
        slot: usize,
    ) -> Option<Option<conduit_kernel::scheduler::StepWorkspaceRequirements>> {
        (slot == 0).then_some(Some(REQUIREMENTS))
    }
    fn retained_backing_bytes(&self) -> Option<usize> {
        Some(0)
    }
    fn bind(self) -> [Option<(u16, Back)>; 1] {
        self.0.set(false);
        [Some((0, Back))]
    }
}
struct Calls;
// SAFETY: an empty immutable native inventory with no effects or backing.
unsafe impl PreparedCallSet for Calls {
    type Calls = ();
    fn nodes(&self) -> impl Iterator<Item = NativeCallNode> {
        core::iter::empty()
    }
    fn retained_backing_bytes(&self) -> Option<usize> {
        Some(0)
    }
    fn bind(self) {}
}
#[test]
fn native_factory_reports_match_original_bound_resident_without_allocating() {
    let available = Cell::new(true);
    let mut software = SoftwareInventory::<'_, 1> {
        slots: [None],
        identities: [None],
        backing: 0,
        configuration: &[],
    };
    let probe = crate::member_return_tests::return_allocation_probe::ObservationInterval::begin();
    let prepared = PreparedNativeDrivers::new(
        Residents(&available),
        [Some(0)],
        PreparedSoftware {
            owner: &mut software,
            selected: [false],
        },
        Calls,
    )
    .unwrap();
    let reports = <_ as PreparedBacks<1, 1>>::execution_workspace_requirements(&prepared).unwrap();
    assert!(available.get());
    assert_eq!(reports, [Some(REQUIREMENTS)]);
    let (drivers, ()) = <_ as PreparedBacks<1, 1>>::bind(prepared);
    assert_eq!(
        StepBack::<1>::workspace_requirements(&drivers[0]),
        reports[0]
    );
    assert!(!available.get());
    let cost = probe.finish();
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
}
