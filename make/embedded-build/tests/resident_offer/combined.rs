//! CPU preparation/retirement proof, not a Body grant or physical execution.
use conduit_kernel::{scheduler::*, *};
use conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal as E;

pub fn run(first: &crate::support::Assignment) {
    let mut library = crate::library::Initialized::prepare().unwrap();
    let original = library.offers().map(|image| {
        let image = image.unwrap();
        (image.as_ptr(), image.len())
    });
    let second =
        crate::support::exercise(crate::library_checks::decode(library.offers()[1].unwrap()));
    let first_active = first.active();
    let second_active = second.active();
    let mut stale = second_active.clone();
    stale.play_sequence += 1;
    let ((), cost) = crate::probe::observe(|| {
        assert!(matches!(
            library.borrow_selected([
                Some(first.selected(&first_active)),
                Some(first.selected(&first_active)),
            ]),
            Err(E::Selection)
        ));
        assert!(matches!(
            library.borrow_selected([
                Some(second.selected(&second_active)),
                Some(second.selected(&second_active)),
            ]),
            Err(E::Selection)
        ));
        assert!(matches!(
            library.borrow_selected([
                Some(first.selected(&first_active)),
                Some(second.selected(&stale)),
            ]),
            Err(E::Scope)
        ));
        assert!(library.offers().iter().all(Option::is_some));
        let (a, b) = library.borrow_selected([None, None]).unwrap();
        assert!(a.is_none() && b.is_none());
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    let mut addresses = None;
    for _ in 0..20 {
        let ((), cost) = crate::probe::observe(|| {
            let (a, b) = library
                .borrow_selected([
                    Some(first.selected(&first_active)),
                    Some(second.selected(&second_active)),
                ])
                .unwrap();
            let (node_a, a) = a.unwrap();
            let (node_b, b) = b.unwrap();
            assert_eq!((node_a, node_b), (0, 0));
            // Two distinct Plot fragments retain their own numeric node scope.
            let actual = (
                &*a as *const crate::library::entry_0::Back as usize,
                &*b as *const crate::library::entry_1::Back as usize,
            );
            assert_ne!(actual.0, actual.1);
            assert_eq!(*addresses.get_or_insert(actual), actual);
            // Both loans coexist. Each must pass actual kernel retirement;
            // simply dropping a loan never restores its availability.
            let a = retire(a, first);
            let b = retire(b, &second);
            assert!(a.finish_retired().is_ok());
            assert!(b.finish_retired().is_ok());
            assert_eq!(
                library.offers().map(|image| {
                    let image = image.unwrap();
                    (image.as_ptr(), image.len())
                }),
                original
            );
        });
        assert_eq!(cost.allocations, 0);
        assert_eq!(cost.reallocations, 0);
    }
    let (a, b) = library
        .borrow_selected([None, Some(second.selected(&second_active))])
        .unwrap();
    assert!(a.is_none());
    drop(b); // Quarantine this abandoned loan; leave the other owner intact.
    assert!(matches!(
        library.borrow_selected([
            Some(first.selected(&first_active)),
            Some(second.selected(&second_active)),
        ]),
        Err(E::MissingOwner)
    ));
    assert!(library.offers()[0].is_some());
    assert!(library.offers()[1].is_none());
    println!("20 simultaneous retained-library loan pairs; exact refusal before mutation, unchanged backing/image addresses, kernel retirement, zero scoped allocations");
}

pub fn retire<D: StepBack<1>>(back: D, assignment: &crate::support::Assignment) -> D {
    finish(back, assignment, None)
}
pub fn execute_and_retire<D: StepBack<1>>(
    back: D,
    assignment: &crate::support::Assignment,
    input: &[u8],
    expected: &[u8],
) -> D {
    finish(back, assignment, Some((input, expected)))
}
fn finish<D: StepBack<1>>(
    back: D,
    assignment: &crate::support::Assignment,
    frame: Option<(&[u8], &[u8])>,
) -> D {
    let mut slot = Some(back);
    let drivers = assignment
        .tables
        .prepare_backs(|_| slot.take().ok_or(()))
        .unwrap();
    let mut selected = assignment
        .tables
        .routes
        .route(NodeId(0), PortId(0))
        .unwrap();
    let target = selected.next().unwrap();
    assert!(selected.next().is_none());
    let mut routes = FixedRoutes::<1, 1>::new(1);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[target],
        )
        .unwrap();
    routes.seal().unwrap();
    let signs = FixedSignLog::<128>::new_with_remote_storage(
        (128 * core::mem::size_of::<KernelEvent>()) as u32,
        64,
        remote_sign_storage_bytes(64).unwrap(),
    )
    .unwrap();
    let mut kernel: FixedScheduler<_, _, _, 1, 2, 1, 2, 1, 1> = FixedScheduler::new(
        assignment.tables.nodes,
        assignment.tables.cords,
        routes,
        drivers,
        FixedValueStore::<2, 2048>::new(4096).unwrap(),
        signs,
    )
    .unwrap();
    if let Some((input, expected)) = frame {
        use conduit_core::AssignedForeDirection;
        let ingress = assignment
            .fore
            .iter()
            .find(|f| f.direction() == AssignedForeDirection::Input)
            .unwrap();
        let egress = assignment
            .fore
            .iter()
            .find(|f| f.direction() == AssignedForeDirection::Output)
            .unwrap();
        kernel
            .admit_remote_input(
                RemoteEndpointId(ingress.endpoint()),
                CordId(ingress.cord()),
                0,
                input,
            )
            .unwrap();
        kernel.step().unwrap();
        let offer = kernel
            .remote_egress_offer(RemoteEndpointId(egress.endpoint()), CordId(egress.cord()))
            .unwrap()
            .unwrap();
        assert_eq!(kernel.host_value(offer.value).unwrap(), expected);
        kernel
            .remote_egress_accept(
                RemoteEndpointId(egress.endpoint()),
                CordId(egress.cord()),
                offer.sequence,
            )
            .unwrap();
        kernel
            .remote_egress_delivered(
                RemoteEndpointId(egress.endpoint()),
                CordId(egress.cord()),
                offer.sequence,
            )
            .unwrap();
    }
    kernel.cancel().unwrap();
    let retired = match kernel.try_retire() {
        Ok(retired) => retired,
        Err(_) => panic!("cancelled drained kernel refused retirement"),
    };
    assert!(retired.cancelled);
    assert_eq!(retired.values.used_items(), 0);
    let [back] = retired.drivers;
    back
}
