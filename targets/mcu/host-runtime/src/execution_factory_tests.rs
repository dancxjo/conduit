use super::*;
use conduit_kernel::{BoundedValueRef, HostCallDisposition, HostCallOutcome, RequestId, ValueRef};

fn value() -> ValueRef {
    ValueRef {
        slot: 0,
        generation: 1,
        byte_len: 1,
    }
}
fn completion(output: bool) -> Option<(RequestId, HostCallOutcome)> {
    Some((
        RequestId(0),
        HostCallOutcome {
            disposition: HostCallDisposition::Completed,
            output: output.then(|| BoundedValueRef::new(value(), 1).unwrap()),
            failure: None,
        },
    ))
}
fn request(back: &mut Driver<'_, HostCallBack>) {
    let mut io = StepIo::test_frame([Some(value())], [false], [Some(1)], None, 4);
    assert_eq!(
        back.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    assert_eq!(io.test_host_request().unwrap().0, RequestId(0));
}

#[test]
fn effect_only_driver_accepts_empty_completion_and_preserves_committed_generations() {
    let input = StepInputBytes::test_frame([None], None);
    let mut back: Driver<'_, HostCallBack> = Driver::Write(WriteBack::new(2));
    request(&mut back);
    request(&mut back); // Discarded transactions do not advance request identity.
    <Driver<HostCallBack> as StepBack<1>>::step_committed(&mut back);
    for _ in 0..2 {
        let mut io = StepIo::test_frame([None], [false], [None], completion(false), 4);
        assert_eq!(back.step(&mut io, &input), StepOutcome::Progress);
        assert!(io.test_host_completion_consumed());
    }
    <Driver<HostCallBack> as StepBack<1>>::step_committed(&mut back);
    let mut io = StepIo::test_frame([Some(value())], [false], [None], None, 4);
    assert_eq!(back.step(&mut io, &input), StepOutcome::Progress);
    assert_eq!(io.test_host_request().unwrap().0, RequestId(1));
}

#[test]
fn completion_shape_remains_specific_to_checked_driver_kind() {
    for sink in [false, true] {
        let mut back: Driver<'_, HostCallBack> = if sink {
            Driver::Write(WriteBack::new(2))
        } else {
            Driver::Native(HostCallBack::new(1))
        };
        request(&mut back);
        <Driver<HostCallBack> as StepBack<1>>::step_committed(&mut back);
        let mut io = StepIo::test_frame([None], [false], [Some(1)], completion(sink), 4);
        assert!(matches!(
            back.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Fail(_)
        ));
        assert!(!io.test_host_completion_consumed());
    }
}

#[test]
fn composite_terminal_laws_survive_software_and_driver_wrappers() {
    let schema =
        conduit_core::StructuredInfoType::leaf(conduit_core::kind_id("value/u64")).unwrap();
    let actual = conduit_composite::FlowMergeFiniteBack::prepare(&schema, 8).unwrap();
    let expected =
        <conduit_composite::FlowMergeFiniteBack as StepBack<2>>::terminal_transductions(&actual);
    assert!(expected.iter().all(Option::is_some));
    let software = SoftwareBack::merge(&schema, 8).unwrap();
    let wrapped: Driver<'_, HostCallBack> = Driver::Software(software);
    assert_eq!(
        <Driver<HostCallBack> as StepBack<2>>::terminal_transductions(&wrapped),
        expected
    );
}

#[test]
fn terminal_state_constructor_refuses_a_schema_without_terminal_boolean() {
    let schema =
        conduit_core::StructuredInfoType::leaf(conduit_core::kind_id("value/u64")).unwrap();
    let contract = conduit_core::CheckedValueContract {
        value_kind: conduit_core::kind_id("value/u64"),
        maximum_bytes: 8,
        constraints: alloc::vec![],
    };
    assert!(SoftwareBack::seeded_until(&contract, &schema).is_err());
}

#[cfg(feature = "execution-workspace")]
#[test]
fn resident_and_extra_drivers_preserve_workspace_custody_through_the_kernel() {
    use conduit_kernel::{scheduler::*, FixedRoutes, FixedSignLog, FixedValueStore, KernelEvent};
    struct NeedsWorkspace(bool);
    impl StepBack<1> for NeedsWorkspace {
        fn workspace_requirements(&self) -> Option<StepWorkspaceRequirements> {
            Some(StepWorkspaceRequirements {
                output_bytes: 4,
                scratch_bytes: 3,
            })
        }
        fn canonical_input_schema(&self, _: PortId) -> Option<&[u8]> {
            Some(&[2, 3])
        }
        fn canonical_output_schema(&self, _: PortId) -> Option<&[u8]> {
            Some(&[4, 5])
        }
        fn step(&mut self, _: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
            panic!("workspace dispatch was lost")
        }
        fn step_with_workspace(
            &mut self,
            _: &mut StepIo<1>,
            _: &StepInputBytes<'_, 1>,
            workspace: &mut StepWorkspace<'_>,
        ) -> StepOutcome {
            let (out, scratch) = workspace.buffers();
            assert_eq!(out, [0; 4]);
            assert_eq!(scratch, [0; 3]);
            out.fill(7);
            scratch.fill(8);
            self.0 = true;
            StepOutcome::Complete
        }
    }
    type Wrapped = Driver<'static, NeedsWorkspace, NeedsWorkspace>;
    type Kernel =
        FixedScheduler<Wrapped, FixedValueStore<1, 1>, FixedSignLog<16>, 1, 1, 1, 1, 1, 1, 1, 1>;
    for extra in [false, true] {
        let back = if extra {
            Driver::Extra(NeedsWorkspace(false))
        } else {
            Driver::Resident(NeedsWorkspace(false))
        };
        assert_eq!(
            StepBack::<1>::canonical_input_schema(&back, PortId(0)),
            Some([2, 3].as_slice())
        );
        assert_eq!(
            StepBack::<1>::canonical_output_schema(&back, PortId(0)),
            Some([4, 5].as_slice())
        );
        let mut routes = FixedRoutes::new(1);
        routes
            .install(
                conduit_kernel::NodeId(0),
                PortId(0),
                conduit_kernel::RouteRange { start: 0, len: 1 },
                &[conduit_kernel::RouteTarget {
                    cord: conduit_kernel::CordId(0),
                    sink: conduit_kernel::CordEndpoint::local(conduit_kernel::NodeId(0), PortId(0)),
                }],
            )
            .unwrap();
        routes.seal().unwrap();
        let mut kernel = Kernel::new(
            [NodeSpec {
                input_cords: [Some(conduit_kernel::CordId(0))],
                maximum_step_fuel: 4,
            }],
            [CordSpec::local(
                conduit_kernel::CordId(0),
                (conduit_kernel::NodeId(0), PortId(0)),
                (conduit_kernel::NodeId(0), PortId(0)),
                CordCapacity {
                    slot_start: 0,
                    item_capacity: 1,
                    byte_capacity: 1,
                    pressure_policy: Default::default(),
                },
            )],
            routes,
            [back],
            FixedValueStore::new(1).unwrap(),
            FixedSignLog::new((16 * core::mem::size_of::<KernelEvent>()) as u32).unwrap(),
        )
        .unwrap();
        assert_eq!(kernel.step(), Err(SchedulerError::InvalidPlan));
        assert_eq!(kernel.logical_invocations(), 0);
        kernel
            .install_execution_workspace(
                PreparedStepWorkspace::try_new(
                    StepWorkspaceRequirements {
                        output_bytes: 4,
                        scratch_bytes: 3,
                    },
                    7,
                )
                .unwrap(),
            )
            .unwrap();
        kernel.run(4).unwrap();
        assert!(match &kernel.drivers()[0] {
            Driver::Resident(b) | Driver::Extra(b) => b.0,
            _ => false,
        });
    }
}

#[cfg(all(feature = "execution-workspace", feature = "schema-value-storage"))]
#[test]
fn static_merge_workspace_requirement_survives_software_and_driver_wrappers() {
    use conduit_core::{StructuredFieldType, StructuredInfoType};
    let schema = StructuredInfoType::record(
        conduit_core::kind_id("test/merge-workspace-reading"),
        alloc::vec![StructuredFieldType::new(
            "reading",
            StructuredInfoType::leaf(conduit_core::kind_id("value/u8")).unwrap(),
        )
        .unwrap()],
    )
    .unwrap();
    let canonical = alloc::boxed::Box::leak(schema.canonical_bytes().unwrap().into_boxed_slice());
    let direct =
        conduit_composite::FlowMergeFiniteBack::prepare_static(&schema, canonical, 512).unwrap();
    let expected =
        <conduit_composite::FlowMergeFiniteBack as StepBack<2>>::workspace_requirements(&direct);
    assert!(expected.is_some_and(|requirements| requirements.output_bytes != 0));
    let software = SoftwareBack::merge_static(&schema, canonical, 512).unwrap();
    let wrapped: Driver<'_, HostCallBack> = Driver::Software(software);
    assert_eq!(
        <Driver<HostCallBack> as StepBack<2>>::workspace_requirements(&wrapped),
        expected
    );
}

#[cfg(feature = "execution-workspace")]
#[test]
fn typed_zip_workspace_requirement_survives_software_and_driver_wrappers() {
    let left = conduit_core::StructuredInfoType::leaf(conduit_core::kind_id("value/u64")).unwrap();
    let right = conduit_core::StructuredInfoType::leaf(conduit_core::kind_id("value/u8")).unwrap();
    let contract = |ty: &conduit_core::StructuredInfoType, bytes| {
        let conduit_core::StructuredInfoTypeShape::Leaf(kind) = ty.shape() else {
            panic!("fixture uses the raw primitive ABI");
        };
        conduit_core::CheckedValueContract::new(kind.clone(), bytes, alloc::vec![]).unwrap()
    };
    let lc = contract(&left, 8);
    let rc = contract(&right, 1);
    for feedback in [false, true] {
        let direct = if feedback {
            conduit_composite::FlowZipBack::prepare_typed_feedback_workspace(
                &lc,
                left.clone(),
                &rc,
                right.clone(),
            )
        } else {
            conduit_composite::FlowZipBack::prepare_typed_finite_workspace(
                &lc,
                left.clone(),
                &rc,
                right.clone(),
            )
        }
        .unwrap();
        let expected =
            <conduit_composite::FlowZipBack as StepBack<2>>::workspace_requirements(&direct);
        assert!(expected.is_some_and(|requirements| requirements.output_bytes != 0));
        let software = SoftwareBack::zip(&lc, left.clone(), &rc, right.clone(), feedback).unwrap();
        let wrapped: Driver<'_, HostCallBack> = Driver::Software(software);
        assert_eq!(
            <Driver<HostCallBack> as StepBack<2>>::workspace_requirements(&wrapped),
            expected
        );
    }
}
