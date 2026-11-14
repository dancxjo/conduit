//! Actual Root binding, execution and retirement, with the Core scope held.
use super::*;
use conduit_kernel::{scheduler::*, *};

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
    let reserved = crate::root_preparer::prepare_workset_storage(
        library,
        Some(&policy()),
        receiver.context(),
        receiver.command_with_contract(bytes, HOST_PREPARED_WORKSET_CONTRACT),
        assignment.maxima,
        16384,
    )
    .unwrap()
    .reserve_observed(owner, pool, 5000, 4, 16384)
    .unwrap();
    let (bound, binding) = crate::probe::observe(|| reserved.bind());
    assert_eq!((binding.allocations, binding.reallocations), (0, 0));
    assert_eq!(bound.admissions().len(), 2);
    assert!(bound
        .admissions()
        .iter()
        .all(|r| r.plan_id.as_str() == super::assignment_body_plan(bytes)));
    let (_, program, _) = crate::support::context();
    let input = crate::support_kernel::frame(&program);
    let expected = program.evaluate(&input).unwrap();
    let (terminal, execution) = crate::probe::observe(|| {
        let (fragments, reservations, _stamp) = bound.into_parts();
        let mut terminal: [Option<FixedSignLog<16>>; 2] = core::array::from_fn(|_| None);
        for (index, fragment) in fragments.into_iter().flatten().enumerate() {
            assert_eq!(
                fragment.approved.metadata().body_plan,
                super::assignment_body_plan(bytes)
            );
            // Original approved Fore ABI supplies the endpoint/cord pair; this
            // CPU driver is not an installed ingress or a transport grant.
            let raw = fragment.approved.metadata().fore_ports;
            let port = |direction| {
                let p = raw
                    .chunks_exact(c::ASSIGNED_LOCAL_FORE_BYTES)
                    .find(|p| p[4] == direction)
                    .unwrap();
                (
                    RemoteEndpointId(u16::from_le_bytes(p[..2].try_into().unwrap())),
                    CordId(u16::from_le_bytes(p[2..4].try_into().unwrap())),
                )
            };
            let (ingress, incoming) = port(0);
            let (egress, outgoing) = port(1);
            let crate::root_preparer::BoundKernel::One(mut kernel) = fragment.kernel else {
                panic!("fixture selects one node per fragment")
            };
            kernel
                .admit_remote_input(ingress, incoming, 0, &input)
                .unwrap();
            kernel.step().unwrap();
            let output = kernel
                .remote_egress_offer(egress, outgoing)
                .unwrap()
                .unwrap();
            assert_eq!(kernel.host_value(output.value).unwrap(), expected);
            kernel
                .remote_egress_accept(egress, outgoing, output.sequence)
                .unwrap();
            kernel
                .remote_egress_delivered(egress, outgoing, output.sequence)
                .unwrap();
            kernel.cancel().unwrap();
            let retired = (*kernel)
                .try_retire()
                .unwrap_or_else(|_| panic!("kernel not terminal/drained"));
            assert!(retired.cancelled);
            assert_eq!(retired.values.used_items(), 0);
            assert!(retired
                .signs
                .events()
                .any(|e| e.kind == KernelEventKind::RunCancelled));
            // Retain the actual terminal Signs before restoring the Back.
            terminal[index] = Some(retired.signs);
            let [back] = retired.drivers;
            assert!(back.finish_retired().is_ok());
        }
        assert_eq!(terminal.iter().flatten().count(), 2);
        drop(reservations); // Both kernels have retired before the scope ends.
        terminal
    });
    assert_eq!((execution.allocations, execution.reallocations), (0, 0));
    assert!(terminal
        .iter()
        .flatten()
        .all(|s| s.events().any(|e| e.kind == KernelEventKind::RunCancelled)));
    assert_eq!(owner, &before);
    assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);

    committed::run(receiver, bytes, assignment, library, owner, pool);

    // Abandoning bound work ends execution/storage and releases its exact
    // reservation, but cannot silently republish an unretired initialized Back.
    let mut abandoned = crate::library::Initialized::prepare().unwrap();
    let pending = async {
        let bound = crate::root_preparer::prepare_workset_storage(
            &mut abandoned,
            Some(&policy()),
            receiver.context(),
            receiver.command_with_contract(bytes, HOST_PREPARED_WORKSET_CONTRACT),
            assignment.maxima,
            16384,
        )
        .unwrap()
        .reserve_observed(owner, pool, 5000, 5, 16384)
        .unwrap()
        .bind();
        core::future::pending::<()>().await;
        drop(bound);
    };
    let future_bytes = core::mem::size_of_val(&pending);
    assert!(future_bytes < 2048);
    let mut pending = Box::pin(pending);
    use core::future::Future;
    let mut cx = core::task::Context::from_waker(core::task::Waker::noop());
    assert!(pending.as_mut().poll(&mut cx).is_pending());
    let ((), cancellation) = crate::probe::observe(|| drop(pending));
    assert_eq!(
        (cancellation.allocations, cancellation.reallocations),
        (0, 0)
    );
    assert_eq!(owner, &before);
    assert!(abandoned.offers().iter().all(Option::is_none));
    println!("actual Root reserved binding: two exact kernels, typed output and terminal Signs retained before Back restoration/resource release; zero binding/execution allocation; {future_bytes}-byte CPU bound future releases its exact resources and leaves unretired providers unavailable");
}

#[path = "root_committed.rs"]
mod committed;
