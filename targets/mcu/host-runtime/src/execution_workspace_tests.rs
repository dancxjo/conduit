//! CPU custody proof; this constructs no Body approval or execution authority.
use super::*;
use core::cell::Cell;
struct Backs<'a> {
    binds: &'a Cell<usize>,
    inspectable: bool,
}
struct Back;
fn requirements() -> StepWorkspaceRequirements {
    StepWorkspaceRequirements {
        output_bytes: 8,
        scratch_bytes: 8,
    }
}
impl StepBack<1> for Back {
    fn workspace_requirements(&self) -> Option<StepWorkspaceRequirements> {
        Some(requirements())
    }
    fn step(&mut self, _: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        panic!("workspace missing")
    }
    fn step_with_workspace(
        &mut self,
        _: &mut StepIo<1>,
        _: &StepInputBytes<'_, 1>,
        workspace: &mut StepWorkspace<'_>,
    ) -> StepOutcome {
        let (out, scratch) = workspace.buffers();
        assert_eq!(out.len(), 8);
        assert_eq!(scratch.len(), 8);
        StepOutcome::Complete
    }
}
// SAFETY: immutable exact one-owner requirements; binding transfers this one
// fixture Back once, with no allocation, hardware effects or hidden backing.
unsafe impl PreparedBacks<1, 1> for Backs<'_> {
    type Driver = Back;
    type Calls = ();
    fn execution_workspace_requirements(&self) -> Option<[Option<StepWorkspaceRequirements>; 1]> {
        self.inspectable.then_some([Some(requirements())])
    }
    fn retained_backing_bytes(&self) -> Option<usize> {
        Some(0)
    }
    fn bind(self) -> ([Back; 1], ()) {
        self.binds.set(self.binds.get() + 1);
        ([Back], ())
    }
}
#[test]
fn original_workspace_is_funded_once_before_any_back_transfer() {
    type Kernel = ExecutionKernel<Back, 1, 1, 1, 1, 1, 1, 16>;
    let binds = Cell::new(0);
    let mut routes = FixedRoutes::new(1);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: CordEndpoint::local(NodeId(0), PortId(0)),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    let mut calls = FixedHostCallBindings::new(1);
    calls.seal().unwrap();
    let values = HostedValueStore::new(4, 8, 32).unwrap();
    let backing = values.reserved_storage_bytes();
    let storage = Kernel::prepare_boxed(
        1,
        1,
        [NodeSpec {
            input_cords: [Some(CordId(0))],
            maximum_step_fuel: 4,
        }],
        [CordSpec::local(
            CordId(0),
            (NodeId(0), PortId(0)),
            (NodeId(0), PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 1,
                pressure_policy: Default::default(),
            },
        )],
        routes,
        calls,
        values,
        FixedSignLog::new((16 * core::mem::size_of::<KernelEvent>()) as u32).unwrap(),
        usize::MAX,
    )
    .unwrap();
    let retained = backing + storage.storage_bytes();
    let address = storage.storage_address();
    let mut prepared = PreparedFragment::<Backs<'_>, 1, 1, 1, 1, 1, 1, 16> {
        storage,
        backs: Backs {
            binds: &binds,
            inspectable: false,
        },
        retained,
        maximum_retained_bytes: retained + 32,
    };
    let workspace = PreparedStepWorkspace::try_new(requirements(), 16).unwrap();
    let (error, workspace) = prepared.fund_execution_workspace(workspace).unwrap_err();
    assert_eq!(error, WorkspaceFundingRefusal::Uninspectable);
    prepared.backs.inspectable = true;
    let large = PreparedStepWorkspace::try_new(
        StepWorkspaceRequirements {
            output_bytes: 32,
            scratch_bytes: 32,
        },
        64,
    )
    .unwrap();
    let (error, _) = prepared.fund_execution_workspace(large).unwrap_err();
    assert_eq!(
        error,
        WorkspaceFundingRefusal::Capacity {
            required: retained + 64,
            available: retained + 32
        }
    );
    let small = PreparedStepWorkspace::try_new(
        StepWorkspaceRequirements {
            output_bytes: 4,
            scratch_bytes: 8,
        },
        12,
    )
    .unwrap();
    assert_eq!(
        prepared.fund_execution_workspace(small).unwrap_err().0,
        WorkspaceFundingRefusal::Requirements
    );
    assert_eq!(binds.get(), 0);
    assert_eq!(prepared.retained, retained);
    prepared.fund_execution_workspace(workspace).unwrap();
    let replacement = PreparedStepWorkspace::try_new(requirements(), 16).unwrap();
    assert_eq!(
        prepared
            .fund_execution_workspace(replacement)
            .unwrap_err()
            .0,
        WorkspaceFundingRefusal::AlreadyFunded
    );
    assert_eq!(prepared.retained, retained + 16);
    assert_eq!(prepared.storage.storage_address(), address);
    assert_eq!(binds.get(), 0);
    assert_eq!(
        crate::execution_workset::PreparedLocalKernels::retained_backing_bytes(&prepared),
        Some(retained + 16)
    );
    // SAFETY: this fixture owns the exact storage/Back and inspected immutable
    // reports. It tests physical transfer only, not a lawful Body Start.
    let (mut kernel, ()) =
        unsafe { crate::execution_workset::PreparedLocalKernels::bind(prepared) };
    assert_eq!(binds.get(), 1);
    assert_eq!(kernel.execution_workspace_backing_bytes(), 16);
    kernel.run(4).unwrap();
}
