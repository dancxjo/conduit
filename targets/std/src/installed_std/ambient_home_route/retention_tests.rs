//! Bounded step-by-step pressure fixture using the installed kernel/Back/route.
//! Kernel, input and Plan storage are test-owned; whole IMAGE admission is
//! separately exercised by the ordinary Body execution tests.
use super::*;
use crate::body_execution::face_contribution::FaceContributionContext;
use crate::installed_std::{
    ambient_home_back::AmbientHomeBack, InstalledBack, InstalledScheduler, HOST_BINDING_SLOTS,
    HOST_CALLS_PER_NODE, MAX_CORDS, MAX_NODES, PORTS, ROUTE_SLOTS, ROUTE_TARGETS,
};
use conduit_kernel::{
    scheduler::{CordCapacity, CordSpec, HostCallRequest, NodeSpec},
    CordId, FixedHostCallBindings, FixedRoutes, HostCallId, HostedSignLog, HostedValueStore,
    NodeId, RemoteEndpointId,
};
use std::sync::atomic::Ordering;

struct Latest {
    current: Option<CommittedFaceContribution>,
    accepted: usize,
}
impl BodyFaceContributionObserver for Latest {
    fn contribute(
        &mut self,
        frame: CommittedFaceContribution,
    ) -> Result<(), ReturnedFaceContribution> {
        if let Some(current) = &self.current {
            assert_eq!(current.fragment().subjects, frame.fragment().subjects);
        }
        self.current = Some(frame);
        self.accepted += 1;
        Ok(())
    }
}

#[test]
fn ambient_home_many_refreshes_keep_a_fixed_frame_envelope_in_original_image() {
    let mut host = common::host(64 * 1024 * 1024);
    // Large finite transition allowance, while the receiver retains only its
    // current frame. Do not reserve 4096 independent escaped Face fragments.
    let (plan, wake) = common::planned(&host, 4096);
    let inputs = (0..8)
        .map(|minute| common::clock_input(1_794_805_200_000 + minute * 60_000))
        .collect::<Vec<_>>();
    let memory = host.kernel_resources.image_memory_observer();
    let baseline = memory.load(Ordering::Acquire);
    let starts = Cell::new(0);
    let mut latest = Latest {
        current: None,
        accepted: 0,
    };
    let result = host
        .run_selected_image_body_plan_with_face_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            &inputs,
            true,
            &mut NoOutput,
            &mut latest,
            &mut Vec::new(),
            &mut ThreadTimer,
            |_, _| {
                starts.set(starts.get() + 1);
                Ok(())
            },
        )
        .unwrap();
    assert!(
        result.report().failure.is_none(),
        "{:?}",
        result.report().failure
    );
    assert_eq!(
        result.report().terminal,
        conduit_core::TerminalDisposition::Completed
    );
    assert_eq!(starts.get(), 1);
    assert_eq!(latest.accepted, 8);
    drop(result);
    assert!(memory.load(Ordering::Acquire) > baseline);
    drop(latest);
    assert_eq!(memory.load(Ordering::Acquire), baseline);
}

#[test]
fn ambient_home_authored_daily_clock_allowance_admits_and_cancels_at_start() {
    let mut host = common::clock_host(64 * 1024 * 1024);
    let (plan, wake) = common::planned_clock(&host, 1440);
    let memory = host.kernel_resources.image_memory_observer();
    let baseline = memory.load(Ordering::Acquire);
    let control = RunControl::default();
    let stop = crate::RunControlRequestId::new("home/daily-preparation-cancel").unwrap();
    let starts = Cell::new(0);
    let result = host
        .run_selected_image_body_plan_with_face_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &[],
            false,
            &mut NoOutput,
            &mut NoObservation,
            &mut Vec::new(),
            &mut ThreadTimer,
            |_, _| {
                starts.set(starts.get() + 1);
                control.request_stop(stop.clone()).unwrap();
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(starts.get(), 1);
    assert!(matches!(
        result.report().terminal,
        conduit_core::TerminalDisposition::Cancelled { .. }
    ));
    drop(result);
    assert_eq!(memory.load(Ordering::Acquire), baseline);
}

fn request(kernel: &mut InstalledScheduler) -> HostCallRequest {
    for _ in 0..16 {
        kernel.step().unwrap();
        if let Some(request) = kernel.next_host_request() {
            return request;
        }
    }
    panic!("expected next finite Home operation");
}

#[test]
fn ambient_home_retention_pressure_releases_or_cancels_the_same_pending_projection() {
    for cancel in [false, true] {
        let mut host = common::host(64 * 1024 * 1024);
        let (plan, wake) = common::planned(&host, 4);
        let play = host.prospective_body_play(&wake, &plan).unwrap();
        let fragment = &plan.plots[0].plan.fragments[0];
        let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment).unwrap();
        let node = NodeId(0);
        let back = AmbientHomeBack::prepare(
            lowered.identity.placement_for_node(node).unwrap().clone(),
            4,
        )
        .unwrap();
        let observation = back.observation();
        let mut calls = FixedHostCallBindings::<HOST_BINDING_SLOTS>::new(HOST_CALLS_PER_NODE);
        for operation in &lowered.host_calls {
            calls.install(node, operation.binding).unwrap();
        }
        calls.seal().unwrap();
        let mut routes = FixedRoutes::<ROUTE_SLOTS, ROUTE_TARGETS>::new(PORTS as u16);
        routes.seal().unwrap();
        let mut nodes = [NodeSpec {
            input_cords: [None; PORTS],
            maximum_step_fuel: 32,
        }; MAX_NODES];
        nodes[0].input_cords[0] = Some(CordId(0));
        let mut cords = [CordSpec::inactive(); MAX_CORDS];
        cords[0] = CordSpec::remote_ingress(
            CordId(0),
            RemoteEndpointId(0),
            (node, conduit_kernel::PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 8192,
                pressure_policy: Default::default(),
            },
        );
        let mut drivers = core::array::from_fn(|_| InstalledBack::Inactive);
        drivers[0] = InstalledBack::AmbientHome(back);
        let mut kernel = InstalledScheduler::new_boxed_with_active_counts_and_host_calls(
            1,
            1,
            nodes,
            cords,
            routes,
            calls,
            drivers,
            HostedValueStore::new(2, 8192, 16384).unwrap(),
            HostedSignLog::new_with_remote_storage(
                128,
                (128 * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32,
                32,
                conduit_kernel::remote_sign_storage_bytes(32).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let mut route = super::super::AmbientHomeRoute::prepare(
            FaceContributionContext {
                wake: &wake,
                plan: &plan,
                play: &play,
                host: host.advertisement(),
                partition: &lowered.identity,
                node,
                call: HostCallId(0),
                contract: &lowered.host_calls[0].contract_id,
            },
            observation,
            "Home",
            4,
        )
        .unwrap();
        let memory = host.kernel_resources.image_memory_observer();
        let baseline = memory.load(Ordering::Acquire);
        let lease = host
            .kernel_resources
            .reserve_managed_base_memory(
                route.additional_backing_bytes().unwrap() + route.escaped_backing_bytes().unwrap(),
            )
            .unwrap();
        route.attach_image_memory(&lease);
        let mut capture = Capture {
            frames: Vec::with_capacity(2),
            pressure: 0,
            attempts: 0,
        };
        let input = common::input();
        for sequence in 0..2 {
            kernel
                .admit_remote_input(RemoteEndpointId(0), CordId(0), sequence, &input.bytes)
                .unwrap();
            let project = request(&mut kernel);
            route.dispatch(&mut kernel, project).unwrap();
            let custody = request(&mut kernel);
            route.dispatch(&mut kernel, custody).unwrap();
            assert!(route.poll(&mut kernel, &mut capture).unwrap());
            kernel.step().unwrap();
        }
        assert_eq!(route.route.retained_fragments(), 2);
        kernel
            .admit_remote_input(RemoteEndpointId(0), CordId(0), 2, &input.bytes)
            .unwrap();
        let project = request(&mut kernel);
        route.dispatch(&mut kernel, project).unwrap();
        for _ in 0..3 {
            assert!(!route.poll(&mut kernel, &mut capture).unwrap());
            assert!(kernel.host_call_is_pending(project));
            assert_eq!(route.waiting_projection, Some(project));
            assert_eq!(route.route.retained_fragments(), 2);
        }
        if cancel {
            kernel.cancel().unwrap();
            assert!(route.poll(&mut kernel, &mut capture).is_err());
        } else {
            drop(capture.frames.remove(0));
            assert!(route.poll(&mut kernel, &mut capture).unwrap());
            assert!(!kernel.host_call_is_pending(project));
            let custody = request(&mut kernel);
            route.dispatch(&mut kernel, custody).unwrap();
            assert!(route.poll(&mut kernel, &mut capture).unwrap());
            kernel.step().unwrap();
            assert_eq!(capture.frames.len(), 2);
            kernel.cancel().unwrap();
        }
        assert!(capture
            .frames
            .iter()
            .all(|frame| frame.scope().is_retired()));
        assert!(!kernel.host_call_is_pending(project));
        assert_eq!(kernel.pending_host_call_count(), 0);
        assert_eq!(kernel.cord_usage(CordId(0)).unwrap(), (0, 0));
        assert!(kernel.host_value(project.input.value).is_err());
        drop(kernel);
        drop(route);
        drop(lease);
        assert!(memory.load(Ordering::Acquire) > baseline);
        drop(capture);
        assert_eq!(memory.load(Ordering::Acquire), baseline);
    }
}
