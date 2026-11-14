use super::*;
use crate::{CordId, FixedSignLog, FixedValueStore, KernelEvent, RouteRange};
const INPUT: &[u8] = &[1, 2, 3, 4];
#[derive(Clone, Copy)]
enum Back {
    Source {
        sent: u8,
        staged: bool,
    },
    Work {
        add: u8,
        wide: bool,
        staged: bool,
    },
    Sink {
        received: u8,
        delay: u8,
        staged: bool,
    },
}
impl StepBack<1> for Back {
    fn workspace_requirements(&self) -> Option<StepWorkspaceRequirements> {
        match self {
            Self::Work { wide, .. } => Some(StepWorkspaceRequirements {
                output_bytes: if *wide { 8 } else { 4 },
                scratch_bytes: 8,
            }),
            _ => None,
        }
    }
    fn step(&mut self, io: &mut StepIo<1>, input: &StepInputBytes<'_, 1>) -> StepOutcome {
        match self {
            Self::Source { sent, staged } => {
                *staged = false;
                if *sent == 2 {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send_prepared(PortId(0), 4).unwrap();
                *staged = true;
                StepOutcome::Progress
            }
            Self::Work { .. } => panic!("workspace loan required"),
            Self::Sink {
                received,
                delay,
                staged,
            } => {
                *staged = false;
                if io.input(PortId(0)).is_none() {
                    if io.input_closed(PortId(0)) {
                        io.consume_closed(PortId(0)).unwrap();
                        return StepOutcome::Complete;
                    }
                    return StepOutcome::Await;
                }
                if *delay > 0 {
                    *delay -= 1;
                    io.exhaust_fuel();
                    return StepOutcome::Yield;
                }
                assert_eq!(input.input(PortId(0)), Some([4, 5, 6, 7].as_slice()));
                io.consume(PortId(0)).unwrap();
                *staged = true;
                let _ = received;
                StepOutcome::Progress
            }
        }
    }
    fn step_with_workspace(
        &mut self,
        io: &mut StepIo<1>,
        input: &StepInputBytes<'_, 1>,
        workspace: &mut StepWorkspace<'_>,
    ) -> StepOutcome {
        let Self::Work { add, staged, .. } = self else {
            return self.step(io, input);
        };
        *staged = false;
        if io.input(PortId(0)).is_none() {
            if io.input_closed(PortId(0)) {
                io.consume_closed(PortId(0)).unwrap();
                return StepOutcome::Complete;
            }
            return StepOutcome::Await;
        }
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let (out, scratch) = workspace.buffers();
        assert!(out.iter().chain(scratch.iter()).all(|b| *b == 0));
        scratch.fill(99);
        for (out, input) in out[..4].iter_mut().zip(input.input(PortId(0)).unwrap()) {
            *out = *input + *add;
        }
        io.send_workspace_output(PortId(0), 4).unwrap();
        io.consume(PortId(0)).unwrap();
        *staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, _: PortId) -> Option<&[u8]> {
        matches!(self, Self::Source { staged: true, .. }).then_some(INPUT)
    }
    fn step_committed(&mut self) {
        match self {
            Self::Source { sent, staged } => {
                *sent += u8::from(*staged);
                *staged = false
            }
            Self::Work { staged, .. } => *staged = false,
            Self::Sink {
                received, staged, ..
            } => {
                *received += u8::from(*staged);
                *staged = false
            }
        }
    }
}
type Kernel =
    FixedScheduler<Back, FixedValueStore<8, 4>, FixedSignLog<256>, 4, 3, 1, 3, 4, 3, 4, 1>;
fn topology() -> (
    [NodeSpec<1>; 4],
    [CordSpec; 3],
    FixedRoutes<4, 3>,
    [Back; 4],
) {
    let mut routes = FixedRoutes::new(1);
    for n in 0..3 {
        routes
            .install(
                NodeId(n),
                PortId(0),
                RouteRange { start: n, len: 1 },
                &[RouteTarget {
                    cord: CordId(n),
                    sink: CordEndpoint::local(NodeId(n + 1), PortId(0)),
                }],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    (
        core::array::from_fn(|n| NodeSpec {
            input_cords: [if n == 0 {
                None
            } else {
                Some(CordId((n - 1) as u16))
            }],
            maximum_step_fuel: 8,
        }),
        core::array::from_fn(|n| {
            CordSpec::local(
                CordId(n as u16),
                (NodeId(n as u16), PortId(0)),
                (NodeId(n as u16 + 1), PortId(0)),
                CordCapacity {
                    slot_start: n as u16,
                    item_capacity: 1,
                    byte_capacity: 4,
                    pressure_policy: Default::default(),
                },
            )
        }),
        routes,
        [
            Back::Source {
                sent: 0,
                staged: false,
            },
            Back::Work {
                add: 1,
                wide: false,
                staged: false,
            },
            Back::Work {
                add: 2,
                wide: true,
                staged: false,
            },
            Back::Sink {
                received: 0,
                delay: 4,
                staged: false,
            },
        ],
    )
}
fn signs() -> FixedSignLog<256> {
    FixedSignLog::new((256 * core::mem::size_of::<KernelEvent>()) as u32).unwrap()
}
fn workspace() -> PreparedStepWorkspace {
    PreparedStepWorkspace::try_new(
        StepWorkspaceRequirements {
            output_bytes: 8,
            scratch_bytes: 8,
        },
        16,
    )
    .unwrap()
}
#[test]
fn complete_workspace_is_required_before_any_step_and_refusal_returns_its_owner() {
    let (n, c, r, d) = topology();
    let mut k = Kernel::new(n, c, r, d, FixedValueStore::new(32).unwrap(), signs()).unwrap();
    assert_eq!(k.run(64), Err(SchedulerError::InvalidPlan));
    assert_eq!(k.logical_invocations(), 0);
    let small = PreparedStepWorkspace::try_new(
        StepWorkspaceRequirements {
            output_bytes: 4,
            scratch_bytes: 8,
        },
        12,
    )
    .unwrap();
    let (error, owner) = k.install_execution_workspace(small).unwrap_err();
    assert_eq!(error, SchedulerError::InvalidPlan);
    assert_eq!(owner.reserved_backing_bytes(), 12);
    k.install_execution_workspace(workspace()).unwrap();
    k.run(128).unwrap();
    assert!(matches!(k.drivers()[3], Back::Sink { received: 2, .. }));
    assert_eq!(k.values().used_items(), 0);
    assert_eq!(k.execution_workspace_backing_bytes(), 16);
    let (error, owner) = k.install_execution_workspace(workspace()).unwrap_err();
    assert_eq!(error, SchedulerError::InvalidPlan);
    assert_eq!(owner.reserved_backing_bytes(), 16);
}
#[test]
fn boxed_preparation_initializes_workspace_custody_before_driver_transfer() {
    let (n, c, r, d) = topology();
    let mut bindings = FixedHostCallBindings::new(1);
    bindings.seal().unwrap();
    let prepared = Kernel::prepare_boxed(
        4,
        3,
        n,
        c,
        r,
        bindings,
        FixedValueStore::new(32).unwrap(),
        signs(),
        usize::MAX,
    )
    .unwrap();
    let mut k = prepared.bind(d);
    assert_eq!(k.run(64), Err(SchedulerError::InvalidPlan));
    assert_eq!(k.logical_invocations(), 0);
    k.install_execution_workspace(workspace()).unwrap();
    k.run(128).unwrap();
    assert!(matches!(k.drivers()[3], Back::Sink { received: 2, .. }));
}
#[test]
fn funding_and_loan_extents_are_checked_before_storage_mutation() {
    assert_eq!(
        PreparedStepWorkspace::try_new(
            StepWorkspaceRequirements {
                output_bytes: 0,
                scratch_bytes: 0
            },
            0
        )
        .unwrap_err(),
        WorkspacePreparationError::Empty
    );
    assert_eq!(
        PreparedStepWorkspace::try_new(
            StepWorkspaceRequirements {
                output_bytes: 8,
                scratch_bytes: 8
            },
            15
        )
        .unwrap_err(),
        WorkspacePreparationError::Capacity {
            required: 16,
            available: 15
        }
    );
    let mut w = workspace();
    {
        let mut loan = w
            .loan(StepWorkspaceRequirements {
                output_bytes: 4,
                scratch_bytes: 2,
            })
            .unwrap();
        let (output, scratch) = loan.buffers();
        assert_eq!(output.len(), 4);
        assert_eq!(scratch.len(), 2);
        output.fill(7);
        scratch.fill(8);
    }
    assert!(w
        .loan(StepWorkspaceRequirements {
            output_bytes: 9,
            scratch_bytes: 8
        })
        .is_err());
    assert_eq!(w.output(4).unwrap(), [7; 4]);
    let mut loan = w
        .loan(StepWorkspaceRequirements {
            output_bytes: 4,
            scratch_bytes: 2,
        })
        .unwrap();
    let (out, scratch) = loan.buffers();
    assert_eq!(out, [0; 4]);
    assert_eq!(scratch, [0; 2]);
}

#[test]
fn cancellation_drains_values_and_does_not_reopen_workspace_installation() {
    let (n, c, r, d) = topology();
    let mut k = Kernel::new(n, c, r, d, FixedValueStore::new(32).unwrap(), signs()).unwrap();
    k.install_execution_workspace(workspace()).unwrap();
    // Start the pipeline while the delayed receiver retains pressure.
    k.step().unwrap();
    k.cancel().unwrap();
    let invocations = k.logical_invocations();
    assert_eq!(k.step().unwrap(), SchedulerStatus::Cancelled);
    assert_eq!(k.logical_invocations(), invocations);
    assert_eq!(k.values().used_items(), 0);
    assert_eq!(k.execution_workspace_backing_bytes(), 16);
    let (error, original_owner) = k.install_execution_workspace(workspace()).unwrap_err();
    assert_eq!(error, SchedulerError::InvalidPlan);
    assert_eq!(original_owner.reserved_backing_bytes(), 16);
}

#[test]
fn prebound_workspace_preserves_original_backing_and_checks_all_active_drivers() {
    for writer in [false, true] {
        for sufficient in [false, true] {
            let (n, c, r, drivers) = topology();
            let mut prepared = if writer {
                Kernel::prepare_boxed_with_table_writer(
                    FixedValueStore::new(32).unwrap(),
                    signs(),
                    usize::MAX,
                    &[[None]; 4],
                    &[[None]; 4],
                    |e| e,
                    move |nodes, cords, routes, calls| {
                        *nodes = n;
                        *cords = c;
                        *routes = r;
                        calls.seal().map_err(KernelPreparationError::from)?;
                        Ok((4, 3))
                    },
                )
                .unwrap()
            } else {
                let mut calls = FixedHostCallBindings::new(1);
                calls.seal().unwrap();
                Kernel::prepare_boxed(
                    4,
                    3,
                    n,
                    c,
                    r,
                    calls,
                    FixedValueStore::new(32).unwrap(),
                    signs(),
                    usize::MAX,
                )
                .unwrap()
            };
            let output_bytes = if sufficient { 8 } else { 4 };
            let workspace = PreparedStepWorkspace::try_new(
                StepWorkspaceRequirements {
                    output_bytes,
                    scratch_bytes: 8,
                },
                16,
            )
            .unwrap();
            let original_pointer = workspace.bytes.as_ptr();
            let backing = workspace.reserved_backing_bytes();
            let (error, workspace) = prepared
                .install_execution_workspace(workspace, backing - 1)
                .unwrap_err();
            assert_eq!(error, SchedulerError::InvalidPlan);
            assert_eq!(workspace.bytes.as_ptr(), original_pointer);
            prepared
                .install_execution_workspace(workspace, backing)
                .unwrap();
            let replacement = super::tests::workspace();
            let replacement_pointer = replacement.bytes.as_ptr();
            let (_, replacement) = prepared
                .install_execution_workspace(replacement, 16)
                .unwrap_err();
            assert_eq!(replacement.bytes.as_ptr(), replacement_pointer);
            let mut kernel = prepared.bind(drivers);
            assert_eq!(
                kernel.execution_workspace.as_ref().unwrap().bytes.as_ptr(),
                original_pointer
            );
            if sufficient {
                kernel.run(128).unwrap();
                assert!(matches!(
                    kernel.drivers()[3],
                    Back::Sink { received: 2, .. }
                ));
            } else {
                assert_eq!(kernel.run(128), Err(SchedulerError::InvalidPlan));
                assert_eq!(kernel.logical_invocations(), 0);
                assert_eq!(kernel.values().used_items(), 0);
            }
        }
    }
}
