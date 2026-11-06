//! Actual Root handoff; CPU-only storage/ledger custody, not wire Start proof.
use super::*;
pub fn run(
    receiver: &Receiver,
    bytes: &[u8],
    assignment: &crate::support::Assignment,
    library: &mut crate::library::Initialized,
    owner: &mut c::ResourceAdmissionOwner,
    pool: &str,
) {
    let before = owner.clone();
    let original = library.offers().map(|o| o.unwrap().as_ptr());
    let refused = crate::root_preparer::prepare_workset_storage(
        library,
        Some(&policy()),
        receiver.context(),
        receiver.command_with_contract(bytes, HOST_PREPARED_WORKSET_CONTRACT),
        assignment.maxima,
        16384,
    )
    .unwrap()
    .admit_and_bind(owner, &[], 16384);
    assert!(matches!(
        &refused,
        Err(DelegatedResourceRefusal::Admission(
            c::ResourceAdmissionRefusal::MissingObservation
        ))
    ));
    drop(refused);
    assert_eq!(owner, &before);
    assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);
    let workset = crate::root_preparer::prepare_workset_storage(
        library,
        Some(&policy()),
        receiver.context(),
        receiver.command_with_contract(bytes, HOST_PREPARED_WORKSET_CONTRACT),
        assignment.maxima,
        16384,
    )
    .unwrap()
    .admit_observed(owner, pool, 5000, 8, 16384)
    .unwrap();
    // The preparation borrow ended. The original ledger can join the kernels
    // in a retained execution owner independently of a delivery future.
    assert_eq!(owner.admissions().len(), 2);
    let keys: Vec<_> = owner
        .admissions()
        .iter()
        .map(|a| (a.plan_id.clone(), a.placement_id.clone()))
        .collect();
    let runtime = (workset, owner);
    let (workset, owner) = runtime;
    let (_, program, _) = crate::support::context();
    let input = crate::support_kernel::frame(&program);
    let expected = program.evaluate(&input).unwrap();
    let (terminal, execution) = crate::probe::observe(|| {
        let (fragments, _original_stamp) = workset.into_parts();
        let mut terminal: [Option<FixedSignLog<16>>; 2] = core::array::from_fn(|_| None);
        for (index, fragment) in fragments.into_iter().flatten().enumerate() {
            assert_eq!(
                fragment.approved.metadata().body_plan,
                super::assignment_body_plan(bytes)
            );
            let crate::root_preparer::BoundKernel::One(mut kernel) = fragment.kernel else {
                panic!("one node per CPU fixture fragment")
            };
            let ports = fragment.approved.metadata().fore_ports;
            let port = |direction| {
                let p = ports
                    .chunks_exact(c::ASSIGNED_LOCAL_FORE_BYTES)
                    .find(|p| p[4] == direction)
                    .unwrap();
                (
                    RemoteEndpointId(u16::from_le_bytes(p[..2].try_into().unwrap())),
                    CordId(u16::from_le_bytes(p[2..4].try_into().unwrap())),
                )
            };
            let (incoming, input_cord) = port(0);
            let (outgoing, output_cord) = port(1);
            kernel
                .admit_remote_input(incoming, input_cord, 0, &input)
                .unwrap();
            kernel.step().unwrap();
            let output = kernel
                .remote_egress_offer(outgoing, output_cord)
                .unwrap()
                .unwrap();
            assert_eq!(kernel.host_value(output.value).unwrap(), expected);
            kernel
                .remote_egress_accept(outgoing, output_cord, output.sequence)
                .unwrap();
            kernel
                .remote_egress_delivered(outgoing, output_cord, output.sequence)
                .unwrap();
            kernel.cancel().unwrap();
            let retired = (*kernel)
                .try_retire()
                .unwrap_or_else(|_| panic!("kernel not terminal"));
            assert!(retired
                .signs
                .events()
                .any(|e| e.kind == KernelEventKind::RunCancelled));
            terminal[index] = Some(retired.signs);
            let [back] = retired.drivers;
            assert!(back.finish_retired().is_ok());
        }
        assert_eq!(owner.admissions().len(), 2);
        for (plan, placement) in &keys {
            let released = owner
                .release_for(plan, placement, c::ResourceReleaseCause::Cancelled)
                .unwrap();
            assert_eq!(released.admission.plan_id, *plan);
            assert_eq!(released.admission.placement_id, *placement);
        }
        terminal
    });
    assert_eq!((execution.allocations, execution.reallocations), (0, 0));
    assert!(terminal.iter().all(Option::is_some));
    assert_eq!(owner, &before);
    assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);

    // Independent inert Host fixture: an abandoned committed kernel must not
    // release its ledger merely because its owning future has been dropped.
    let mut failure_owner = c::ResourceAdmissionOwner::new(owner.host().clone());
    let mut abandoned = crate::library::Initialized::prepare().unwrap();
    let committed = crate::root_preparer::prepare_workset_storage(
        &mut abandoned,
        Some(&policy()),
        receiver.context(),
        receiver.command_with_contract(bytes, HOST_PREPARED_WORKSET_CONTRACT),
        assignment.maxima,
        16384,
    )
    .unwrap()
    .admit_observed(&mut failure_owner, pool, 5000, 9, 16384)
    .unwrap();
    let pending = async move {
        core::future::pending::<()>().await;
        drop(committed);
    };
    let mut pending = Box::pin(pending);
    use core::future::Future;
    let mut cx = core::task::Context::from_waker(core::task::Waker::noop());
    assert!(pending.as_mut().poll(&mut cx).is_pending());
    let ((), cancellation) = crate::probe::observe(|| drop(pending));
    assert_eq!(
        (cancellation.allocations, cancellation.reallocations),
        (0, 0)
    );
    assert_eq!(failure_owner.admissions().len(), 2);
    assert!(abandoned.offers().iter().all(Option::is_none));
    println!("actual Root committed handoff: original ledger remains reserved after preparation/future exit, terminal Signs precede explicit release, abandoned kernels keep reservations/providers unavailable; zero execution/retirement allocation, CPU-only");
}
