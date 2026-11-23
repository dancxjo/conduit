//! Real scheduler custody proof. Payload bytes are opaque here; the model suite
//! separately proves canonical Home decoding. No Body route/IMAGE claim.
use super::*;
use conduit_kernel::{
    scheduler::{CordCapacity, CordSpec, FixedScheduler, HostCallRequest, NodeSpec},
    CordId, FixedHostCallBindings, FixedRoutes, FixedSignLog, FixedValueStore, HostCallBinding,
    HostCallOutcome, NodeId, RemoteEndpointId,
};

type Kernel = FixedScheduler<
    AmbientHomeBack,
    FixedValueStore<2, 8192>,
    FixedSignLog<128>,
    1,
    1,
    1,
    1,
    0,
    0,
    2,
    1,
>;

fn kernel(updates: u32) -> Kernel {
    let offer = conduit_std_offers::resident_ambient_home_face_offer().unwrap();
    let mut calls = FixedHostCallBindings::<2>::new(2);
    for (index, call) in offer.host_calls.iter().enumerate() {
        calls
            .install(
                NodeId(0),
                HostCallBinding {
                    call: HostCallId(index as u16),
                    maximum_input_bytes: call.maximum_input_bytes,
                    maximum_output_bytes: call.maximum_output_bytes,
                },
            )
            .unwrap();
    }
    calls.seal().unwrap();
    let mut routes = FixedRoutes::<0, 0>::new(1);
    routes.seal().unwrap();
    let signs = FixedSignLog::<128>::new_with_remote_storage(
        (128 * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32,
        32,
        conduit_kernel::remote_sign_storage_bytes(32).unwrap(),
    )
    .unwrap();
    Kernel::new_with_active_counts_and_host_calls(
        1,
        1,
        [NodeSpec {
            input_cords: [Some(CordId(0))],
            maximum_step_fuel: 32,
        }],
        [CordSpec::remote_ingress(
            CordId(0),
            RemoteEndpointId(0),
            (NodeId(0), INPUT),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: INPUT_BYTES,
                pressure_policy: Default::default(),
            },
        )],
        routes,
        calls,
        [AmbientHomeBack::prepare("home/kernel".into(), updates).unwrap()],
        FixedValueStore::new(INPUT_BYTES * 2).unwrap(),
        signs,
    )
    .unwrap()
}
fn request(kernel: &mut Kernel) -> HostCallRequest {
    for _ in 0..8 {
        kernel.step().unwrap();
        if let Some(request) = kernel.next_host_request() {
            return request;
        }
    }
    panic!("expected bounded Home Host Call");
}
fn complete(kernel: &mut Kernel, request: HostCallRequest) {
    kernel
        .complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            },
        )
        .unwrap();
}

#[test]
fn scheduler_retains_original_until_second_completion_and_rearms_next_update() {
    let mut kernel = kernel(2);
    for update in 0..2 {
        let bytes = [update as u8; 8];
        kernel
            .admit_remote_input(RemoteEndpointId(0), CordId(0), update, &bytes)
            .unwrap();
        let project = request(&mut kernel);
        assert_eq!(project.call, PROJECT);
        assert_eq!(project.request, RequestId(update as u32 * 2));
        assert!(kernel.host_call_is_pending(project));
        assert_eq!(kernel.host_value(project.input.value).unwrap(), &bytes);
        assert_eq!(kernel.cord_usage(CordId(0)).unwrap(), (1, 8));
        complete(&mut kernel, project);
        let custody = request(&mut kernel);
        assert_eq!(custody.call, ACCEPT_CUSTODY);
        assert_eq!(custody.request, RequestId(project.request.0 + 1));
        assert_eq!(custody.input, project.input);
        for _ in 0..8 {
            kernel.step().unwrap();
            assert!(kernel.host_call_is_pending(custody));
            assert_eq!(kernel.cord_usage(CordId(0)).unwrap(), (1, 8));
            assert_eq!(kernel.host_value(project.input.value).unwrap(), &bytes);
            assert_eq!(kernel.drivers()[0].accepted_updates, update as u32);
        }
        complete(&mut kernel, custody);
        kernel.step().unwrap();
        assert_eq!(kernel.cord_usage(CordId(0)).unwrap(), (0, 0));
        assert_eq!(kernel.drivers()[0].accepted_updates, update as u32 + 1);
        assert!(kernel.host_value(project.input.value).is_err());
        assert_eq!(kernel.pending_host_call_count(), 0);
    }
    kernel
        .close_remote_input(RemoteEndpointId(0), CordId(0))
        .unwrap();
    kernel.step().unwrap();
    assert!(kernel.drivers()[0].pending.is_none());
    assert!(!kernel.has_ready_work());
}
