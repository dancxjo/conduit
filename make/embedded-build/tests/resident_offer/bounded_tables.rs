//! Actual assigned tables feed the existing active-prefix kernel, not padding.
use super::*;
use conduit_kernel::{scheduler::*, *};
use conduit_plan_lowering::delegated_kernel::prepare_delegated_body_kernel_tables as prepare;

// A test tripwire: inactive capacity must never run any Back callback.
struct Slot<B>(Option<B>);
impl<B: StepBack<1>> StepBack<1> for Slot<B> {
    fn step(&mut self, io: &mut StepIo<1>, input: &StepInputBytes<'_, 1>) -> StepOutcome {
        self.0.as_mut().expect("inactive step").step(io, input)
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        self.0
            .as_ref()
            .expect("inactive output")
            .prepared_output(port)
    }
    fn step_committed(&mut self) {
        self.0.as_mut().expect("inactive commit").step_committed()
    }
    fn cancel(&mut self) {
        self.0.as_mut().expect("inactive cancel").cancel()
    }
}

pub fn run(
    fragment: &DelegatedBodyFragment<'_>,
    a: &crate::support::Assignment,
    library: &mut crate::library::Initialized,
) {
    let (_, program, _) = crate::support::context();
    let input = crate::support_kernel::frame(&program);
    let expected = program.evaluate(&input).unwrap();
    let schema = u16::from_le_bytes(a.encoded[8..10].try_into().unwrap());
    let pins: Vec<_> = c::AssignedPlanRecords::new(&a.encoded)
        .unwrap()
        .map(Result::unwrap)
        .filter(|r| {
            !matches!(
                r.tag,
                c::ASSIGNED_CONFIGURATION | c::ASSIGNED_SIGN | c::ASSIGNED_TERMINAL
            )
        })
        .map(|r| c::AssignedRecordRequirement::from_kernel_record(r, schema))
        .collect();
    let ((), cost) = crate::probe::observe(|| {
        for refused in [
            prepare::<128, 0, 8, 1, 4, 8, 0>(fragment, profile(a)).is_err(),
            prepare::<128, 4, 1, 1, 4, 8, 0>(fragment, profile(a)).is_err(),
            prepare::<1, 4, 8, 1, 4, 8, 0>(fragment, profile(a)).is_err(),
        ] {
            assert!(refused);
        }
        let p = profile(a);
        let layout = conduit_kernel::assigned::ResidentLayout {
            records: &pins,
            counts: a.tables.identity.counts,
        };
        // The existing exact-capacity entrance must retain its stricter law.
        assert_eq!(
            conduit_kernel::assigned::AssignedKernelTables::<4, 8, 1, 4, 8, 0>::prepare_with_fore(
                &a.encoded,
                p.maxima,
                p.requirements,
                p.computations,
                &layout,
                &a.fore,
            )
            .err(),
            Some(conduit_kernel::assigned::AssignedKernelRefusal::Layout)
        );
        let incomplete = conduit_kernel::assigned::ResidentLayout {
            records: &pins[..pins.len() - 1],
            counts: a.tables.identity.counts,
        };
        assert_eq!(conduit_kernel::assigned::BoundedAssignedKernelTables::<4, 8, 1, 4, 8, 0>::prepare_with_fore(
            &a.encoded, p.maxima, p.requirements, p.computations, &incomplete, &a.fore,
        ).err(), Some(conduit_kernel::assigned::AssignedKernelRefusal::Layout));
        assert!(library.offers().iter().all(Option::is_some));
        let tables = prepare::<128, 4, 8, 1, 4, 8, 0>(fragment, profile(a)).unwrap();
        assert_eq!(tables.identity(), a.tables.identity);
        assert_eq!((tables.active_nodes(), tables.active_cords()), (1, 2));
        assert_eq!(tables.startup_order(), &[NodeId(0)]);
        assert_eq!(&tables.nodes[..1], &a.tables.nodes);
        assert_eq!(&tables.cords[..2], &a.tables.cords);
        assert!(tables.nodes[1..].iter().all(|n| n.maximum_step_fuel == 0));
        assert!(tables.cords[2..].iter().all(|c| *c == CordSpec::inactive()));
        let (_, back) = library
            .borrow_entry_0(a.selected_scope(AssignedPlayScope::Delegated(fragment)))
            .unwrap();
        let mut back = Some(back);
        let mut visited = 0;
        let drivers = tables
            .prepare_backs(|node| {
                assert_eq!(node, NodeId(0));
                visited += 1;
                back.take().ok_or(())
            })
            .unwrap()
            .map(Slot);
        assert_eq!(visited, 1);
        let active = (tables.active_nodes(), tables.active_cords());
        let signs = FixedSignLog::<128>::new_with_remote_storage(
            (128 * core::mem::size_of::<KernelEvent>()) as u32,
            64,
            remote_sign_storage_bytes(64).unwrap(),
        )
        .unwrap();
        let mut kernel: FixedScheduler<_, _, _, 4, 8, 1, 2, 4, 8> =
            FixedScheduler::new_with_active_counts(
                active.0,
                active.1,
                tables.nodes,
                tables.cords,
                tables.routes,
                drivers,
                FixedValueStore::<2, 2048>::new(4096).unwrap(),
                signs,
            )
            .unwrap();
        let ingress = a
            .fore
            .iter()
            .find(|f| f.direction() == c::AssignedForeDirection::Input)
            .unwrap();
        let egress = a
            .fore
            .iter()
            .find(|f| f.direction() == c::AssignedForeDirection::Output)
            .unwrap();
        kernel
            .admit_remote_input(
                RemoteEndpointId(ingress.endpoint()),
                CordId(ingress.cord()),
                0,
                &input,
            )
            .unwrap();
        kernel.step().unwrap();
        let output = kernel
            .remote_egress_offer(RemoteEndpointId(egress.endpoint()), CordId(egress.cord()))
            .unwrap()
            .unwrap();
        assert_eq!(kernel.host_value(output.value).unwrap(), expected);
        kernel
            .remote_egress_accept(
                RemoteEndpointId(egress.endpoint()),
                CordId(egress.cord()),
                output.sequence,
            )
            .unwrap();
        kernel
            .remote_egress_delivered(
                RemoteEndpointId(egress.endpoint()),
                CordId(egress.cord()),
                output.sequence,
            )
            .unwrap();
        kernel.cancel().unwrap();
        let retired = match kernel.try_retire() {
            Ok(value) => value,
            Err(_) => panic!("drained active-prefix kernel did not retire"),
        };
        assert_eq!(retired.values.used_items(), 0);
        let mut drivers = retired.drivers.into_iter();
        assert!(drivers.next().unwrap().0.unwrap().finish_retired().is_ok());
        assert!(drivers.all(|slot| slot.0.is_none()));
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    assert!(library.offers().iter().all(Option::is_some));
    println!("approved exact topology in fixed capacity: only active Back visited; typed output, cancellation, retirement and same-owner return; zero scoped allocation; insufficient node/cord/layout capacity refused");
}
