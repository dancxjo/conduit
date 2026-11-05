//! Real storage before Core reservations and actual resident Back transfers.
use super::*;
use conduit_kernel::{scheduler::*, *};
type Kernel<D> =
    FixedScheduler<D, FixedValueStore<2, 2048>, FixedSignLog<128>, 1, 2, 1, 2, 1, 1, 0, 1>;
type Prepared<D> = PreparedKernelStorage<D, Kernel<D>, 1>;
// Independent fixed CPU fixture budget, not an MCU grant or measured Root heap.
const FIXTURE_PREPARATION_BYTES: usize = 32 * 1024;
fn prepare<D: StepBack<1>>(
    a: &crate::support::Assignment,
    ceiling: usize,
) -> Result<Prepared<D>, KernelPreparationError> {
    let target = a
        .tables
        .routes
        .route(NodeId(0), PortId(0))
        .unwrap()
        .next()
        .unwrap();
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
    let mut calls = FixedHostCallBindings::<0>::new(1);
    calls.seal().unwrap();
    Kernel::<D>::prepare_boxed(
        1,
        2,
        a.tables.nodes,
        a.tables.cords,
        routes,
        calls,
        FixedValueStore::<2, 2048>::new(4096).unwrap(),
        FixedSignLog::<128>::new_with_remote_storage(
            (128 * size_of::<KernelEvent>()) as u32,
            64,
            remote_sign_storage_bytes(64).unwrap(),
        )
        .unwrap(),
        ceiling,
    )
}
fn finish<D: StepBack<1>>(
    mut kernel: Box<Kernel<D>>,
    a: &crate::support::Assignment,
    input: &[u8],
    expected: &[u8],
) -> D {
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
            input,
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
    let retired = match (*kernel).try_retire() {
        Ok(r) => r,
        Err(_) => panic!("drained kernel did not retire"),
    };
    assert!(retired.cancelled);
    assert_eq!(retired.values.used_items(), 0);
    let [back] = retired.drivers;
    back
}
pub fn run(
    a: PreparedBodyFragment<'_>,
    b: PreparedBodyFragment<'_>,
    first: &crate::support::Assignment,
    second: &crate::support::Assignment,
) {
    let docs = [document(a, first), document(b, second)];
    let bytes = encode_body_workset_document(&[docs[0].bytes(), docs[1].bytes()], 8192).unwrap();
    let mut receiver = Receiver::new(a);
    let bindings = [
        &a.fragment().placements[0].resources[0],
        &b.fragment().placements[0].resources[0],
    ];
    assert_eq!(bindings[0].pool_id, bindings[1].pool_id);
    receiver.actual.resources.push(c::ResourceOffer {
        pool_id: bindings[0].pool_id.clone(),
        class_id: bindings[0].class_id.clone(),
        capacity_units: bindings.iter().map(|b| b.units).sum(),
        compute: None,
        content: None,
    });
    let workset = receive_delegated_body_workset::<2>(
        Some(&policy()),
        receiver.context(),
        receiver.command_with_contract(&bytes, HOST_PREPARED_WORKSET_CONTRACT),
    )
    .unwrap();
    let left = workset
        .receive_fragment_with_approved_fore::<2>(
            0,
            Some(&policy()),
            receiver.context(),
            AssignedBodyPreparationProfile {
                fore: None,
                ..profile(first)
            },
        )
        .unwrap()
        .0;
    let right = workset
        .receive_fragment_with_approved_fore::<2>(
            1,
            Some(&policy()),
            receiver.context(),
            AssignedBodyPreparationProfile {
                fore: None,
                ..profile(second)
            },
        )
        .unwrap()
        .0;
    let mut library = crate::library::Initialized::prepare().unwrap();
    library
        .check_entry(0, first.selected_scope(AssignedPlayScope::Delegated(&left)))
        .unwrap();
    library
        .check_entry(
            1,
            second.selected_scope(AssignedPlayScope::Delegated(&right)),
        )
        .unwrap();
    let original = library.offers().map(|o| o.unwrap().as_ptr());
    let mut owner = c::ResourceAdmissionOwner::new(receiver.actual.clone());
    let before = owner.clone();
    let observed = observation(owner.host());
    let total = size_of::<Kernel<crate::library::entry_0::BorrowedBack<'_>>>()
        + size_of::<Kernel<crate::library::entry_1::BorrowedBack<'_>>>();
    let (result, cost) = crate::probe::observe(|| {
        let left = prepare::<crate::library::entry_0::BorrowedBack<'_>>(first, total).unwrap();
        let remaining = total - 1 - left.storage_bytes();
        prepare::<crate::library::entry_1::BorrowedBack<'_>>(second, remaining)
    });
    assert!(matches!(
        result,
        Err(KernelPreparationError::Capacity { .. })
    ));
    assert_eq!((cost.allocations, cost.deallocations), (1, 1));
    assert_eq!(owner, before);
    assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);
    // A late Core shortage also drops both complete storage preparations before
    // taking either provider. No rollback ledger or compensating Start is used.
    let mut short = observed.clone();
    short.unreserved_units -= 1;
    let ((), cost) = crate::probe::observe(|| {
        let ka =
            prepare::<crate::library::entry_0::BorrowedBack<'_>>(first, FIXTURE_PREPARATION_BYTES)
                .unwrap();
        let kb = prepare::<crate::library::entry_1::BorrowedBack<'_>>(
            second,
            FIXTURE_PREPARATION_BYTES - ka.storage_bytes(),
        )
        .unwrap();
        assert!(admit_delegated_plain_resources(
            &[&left, &right],
            &mut owner,
            core::slice::from_ref(&short)
        )
        .is_err());
        drop((ka, kb));
    });
    assert_eq!(cost.allocations, cost.deallocations);
    assert_eq!(owner, before);
    assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);
    let ((ka, kb), cost) = crate::probe::observe(|| {
        let ka =
            prepare::<crate::library::entry_0::BorrowedBack<'_>>(first, FIXTURE_PREPARATION_BYTES)
                .unwrap();
        let kb = prepare::<crate::library::entry_1::BorrowedBack<'_>>(
            second,
            FIXTURE_PREPARATION_BYTES - ka.storage_bytes(),
        )
        .unwrap();
        admit_delegated_plain_resources(
            &[&left, &right],
            &mut owner,
            core::slice::from_ref(&observed),
        )
        .unwrap();
        (ka, kb)
    });
    let peak = cost.peak_bytes;
    assert!(peak <= FIXTURE_PREPARATION_BYTES);
    assert_eq!(owner.admissions().len(), 2);
    let (_, program, _) = crate::support::context();
    let input = crate::support_kernel::frame(&program);
    let expected = program.evaluate(&input).unwrap();
    let ((), execution) = crate::probe::observe(|| {
        let (a, b) = library
            .borrow_selected([
                Some(first.selected_scope(AssignedPlayScope::Delegated(&left))),
                Some(second.selected_scope(AssignedPlayScope::Delegated(&right))),
            ])
            .unwrap();
        let ka = ka.bind([a.unwrap().1]);
        let kb = kb.bind([b.unwrap().1]);
        assert!(finish(ka, first, &input, &expected)
            .finish_retired()
            .is_ok());
        assert!(finish(kb, second, &input, &expected)
            .finish_retired()
            .is_ok());
    });
    assert_eq!((execution.allocations, execution.reallocations), (0, 0));
    assert_eq!(execution.deallocations, 2);
    assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);
    for scope in [a, b] {
        owner
            .release_for(
                &scope.plan().plan_id,
                &scope.fragment().placements[0].placement_id,
                c::ResourceReleaseCause::Cancelled,
            )
            .unwrap();
    }
    assert!(owner.admissions().is_empty());
    println!("complete checked workset storage before Core reservations/Back transfer: {total} kernel payload bytes, {peak} peak preparation bytes; late storage/resource refusal preserves owners; typed output, retirement and exact releases, zero execution allocation");
}
