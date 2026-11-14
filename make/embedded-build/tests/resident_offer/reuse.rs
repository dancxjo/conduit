//! CPU ownership proof across real kernel retirement; no device authority claim.
use conduit_core as c;
use conduit_kernel::{scheduler::*, *};

pub fn run(assignment: &crate::support::Assignment) {
    let mut library = crate::library::Initialized::prepare().unwrap();
    let offered = library.offers()[0].unwrap();
    let image = (offered.as_ptr(), offered.len());
    let size = core::mem::size_of::<crate::library::entry_0::Back>();
    assert_eq!(
        core::mem::size_of::<crate::library::entry_0::BorrowedBack<'_>>(),
        core::mem::size_of::<usize>()
    );
    assert_ne!(
        std::any::TypeId::of::<crate::library::entry_0::BorrowedBack<'static>>(),
        std::any::TypeId::of::<crate::library::entry_1::BorrowedBack<'static>>()
    );
    type OwnedKernel = FixedScheduler<
        crate::library::entry_0::Back,
        FixedValueStore<2, 2048>,
        FixedSignLog<128>,
        1,
        2,
        1,
        2,
        1,
        1,
    >;
    type BorrowedKernel<'a> = FixedScheduler<
        crate::library::BorrowedBack<'a>,
        FixedValueStore<2, 2048>,
        FixedSignLog<128>,
        1,
        2,
        1,
        2,
        1,
        1,
    >;
    let owned_kernel_bytes = core::mem::size_of::<OwnedKernel>();
    let borrowed_kernel_bytes = core::mem::size_of::<BorrowedKernel<'_>>();
    assert_eq!(
        owned_kernel_bytes - borrowed_kernel_bytes,
        size - core::mem::size_of::<crate::library::BorrowedBack<'_>>()
    );
    let mut backing_address = None;
    let (_, program, _) = crate::support::context();
    let mut input = crate::support_kernel::frame(&program);
    let ingress = assignment
        .fore
        .iter()
        .find(|f| f.direction() == c::AssignedForeDirection::Input)
        .unwrap();
    let egress = assignment
        .fore
        .iter()
        .find(|f| f.direction() == c::AssignedForeDirection::Output)
        .unwrap();
    let fragment = &assignment.model.fragments[0];
    for play_sequence in 1..=100 {
        *input.last_mut().unwrap() = play_sequence as u8;
        let expected = program.evaluate(&input).unwrap();
        let active = c::bind_active_play(
            &assignment.model.plan_id,
            &fragment.host_id,
            &fragment.boot_id,
            play_sequence,
        );
        let (_, cost) = crate::probe::observe(|| {
            let [Some((_, back)), None] = library
                .borrow_selected_array([Some(assignment.selected(&active)), None])
                .unwrap() else { panic!("wrong selected entry"); };
            let address = entry_address(&back);
            assert_eq!(*backing_address.get_or_insert(address), address);
            assert!(core::mem::size_of_val(&back) <= 2 * core::mem::size_of::<usize>());
            // A loan is not automatically retired merely because it is owned.
            let back = match back.finish_retired() {
                Err(back) => back,
                Ok(()) => panic!("fresh loan retired"),
            };
            let mut slot = Some(back);
            let backs = assignment
                .tables
                .prepare_backs(|_| slot.take().ok_or(()))
                .unwrap();
            let signs = FixedSignLog::<128>::new_with_remote_storage(
                (128 * core::mem::size_of::<KernelEvent>()) as u32,
                64,
                remote_sign_storage_bytes(64).unwrap(),
            )
            .unwrap();
            let mut selected_routes = assignment
                .tables
                .routes
                .route(NodeId(0), PortId(0))
                .unwrap();
            let target = selected_routes.next().unwrap();
            assert!(selected_routes.next().is_none());
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
            let mut kernel: FixedScheduler<_, _, _, 1, 2, 1, 2, 1, 1> = FixedScheduler::new(
                assignment.tables.nodes,
                assignment.tables.cords,
                routes,
                backs,
                FixedValueStore::<2, 2048>::new(4096).unwrap(),
                signs,
            )
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
            // Offered output still belongs to the old Play; it cannot retire.
            let mut kernel = match kernel.try_retire() {
                Err(kernel) => kernel,
                Ok(_) => panic!("live output retired"),
            };
            let offered = kernel
                .remote_egress_offer(RemoteEndpointId(egress.endpoint()), CordId(egress.cord()))
                .unwrap()
                .unwrap();
            assert_eq!(kernel.host_value(offered.value).unwrap(), expected);
            kernel
                .remote_egress_accept(
                    RemoteEndpointId(egress.endpoint()),
                    CordId(egress.cord()),
                    offered.sequence,
                )
                .unwrap();
            kernel
                .remote_egress_delivered(
                    RemoteEndpointId(egress.endpoint()),
                    CordId(egress.cord()),
                    offered.sequence,
                )
                .unwrap();
            let mut kernel = match kernel.try_retire() {
                Err(kernel) => kernel,
                Ok(_) => panic!("open input retired"),
            };
            if play_sequence % 2 == 0 {
                // Cancellation also drains an accepted but unconsumed input.
                kernel
                    .admit_remote_input(
                        RemoteEndpointId(ingress.endpoint()),
                        CordId(ingress.cord()),
                        1,
                        &input,
                    )
                    .unwrap();
                kernel.cancel().unwrap();
            } else {
                kernel
                    .close_remote_input(
                        RemoteEndpointId(ingress.endpoint()),
                        CordId(ingress.cord()),
                    )
                    .unwrap();
                for _ in 0..4 {
                    kernel.step().unwrap();
                }
            }
            let retired = match kernel.try_retire() {
                Ok(retired) => retired,
                Err(_) => panic!("terminal drained kernel refused"),
            };
            assert_eq!(retired.values.used_items(), 0);
            assert_eq!(retired.cancelled, play_sequence % 2 == 0);
            let RetiredExecution {
                drivers: [back],
                signs,
                ..
            } = retired;
            let address = entry_address(&back);
            assert_eq!(Some(address), backing_address);
            if back.finish_retired().is_err() {
                panic!("retired Back did not return");
            }
            assert_eq!(
                library.offers()[0].map(|o| (o.as_ptr(), o.len())),
                Some(image)
            );
            assert_eq!(core::mem::size_of::<crate::library::entry_0::Back>(), size);
            // Sign ownership stays with the caller, independently of Back restoration.
            drop(signs);
        });
        assert_eq!(cost.allocations, 0);
        assert_eq!(cost.reallocations, 0);
    }
    let active = assignment.active();
    let [Some((_, abandoned)), None] = library
        .borrow_selected_array([Some(assignment.selected(&active)), None])
        .unwrap() else { panic!("wrong selected entry"); };
    drop(abandoned);
    assert!(library.offers()[0].is_none());
    assert!(library.offers()[1].is_some());
    assert!(matches!(library.borrow_entry_0(assignment.selected(&active)), Err(conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal::MissingOwner)));
    println!("owned kernel {owned_kernel_bytes} B, borrowed kernel {borrowed_kernel_bytes} B, Back {size} B; 100 same-Plan Plays: 50 completed, 50 cancelled; same backing and exact resident image restored, uniform nominal loans, zero scoped allocation");
}

fn entry_address(back: &crate::library::BorrowedBack<'_>) -> usize {
    match back {
        crate::library::BorrowedBack::Entry0(back) => &**back as *const crate::library::entry_0::Back as usize,
        _ => panic!("selected entry changed nominal identity"),
    }
}
