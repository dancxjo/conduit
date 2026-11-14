//! Gate and scope proof only; this fixture is not a new installed Face Gear.
use super::*;
use conduit_kernel::{
    scheduler::{CordCapacity, CordSpec, NodeSpec, SchedulerError, StepInputBytes, StepOutcome},
    BoundedValueRef, CordId, FixedHostCallBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostCallBinding, HostCallOutcome, RemoteEndpointId,
};
use std::sync::atomic::AtomicBool;

struct Back {
    gate: Option<FaceCommitGate>,
    pending: bool,
    done: bool,
    candidate: u8,
    reject: Arc<AtomicBool>,
    input_bytes: u32,
}
impl StepBack<1> for Back {
    fn step(&mut self, io: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        self.candidate = 0;
        let Some(gate) = self.gate.as_mut() else {
            return StepOutcome::Await;
        };
        gate.begin_step();
        if io.host_completion().is_some() {
            gate.consume_completion(io, RequestId(0)).unwrap();
            self.candidate = 2;
            return if self.reject.swap(false, Ordering::Relaxed) {
                StepOutcome::Await
            } else {
                StepOutcome::Progress
            };
        }
        if !self.pending && !self.done {
            if let Some(value) = io.input(conduit_kernel::PortId(0)) {
                io.consume(conduit_kernel::PortId(0)).unwrap();
                io.request_host_call(
                    RequestId(0),
                    gate.state.call,
                    BoundedValueRef::new(value, self.input_bytes).unwrap(),
                )
                .unwrap();
                self.candidate = 1;
                return StepOutcome::Progress;
            }
        }
        StepOutcome::Await
    }
    fn step_committed(&mut self) {
        match self.candidate {
            1 => self.pending = true,
            2 => {
                self.gate.as_mut().unwrap().step_committed();
                self.pending = false;
                self.done = true;
            }
            _ => {}
        }
    }
    fn cancel(&mut self) {
        if let Some(gate) = &mut self.gate {
            gate.cancel();
        }
    }
}
type Kernel =
    FixedScheduler<Back, FixedValueStore<4, 1024>, FixedSignLog<128>, 16, 1, 1, 1, 0, 0, 16, 1>;
struct Fixture {
    kernel: Kernel,
    route: PendingFaceContributionRoute,
    reject: Arc<AtomicBool>,
    request: HostCallRequest,
}
fn fixture() -> Fixture {
    let adapter = crate::hosted_creche::test_support::Adapter::new();
    let (host, plan, play, wake) = crate::hosted_creche::test_support::planned(&adapter);
    let fragment = &plan.plots[0].plan.fragments[0];
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment).unwrap();
    let operation = &lowered.host_calls[0];
    let node = operation.node;
    let placement = lowered.identity.placement_for_node(node).unwrap().clone();
    let gate = FaceCommitGate::prepare(placement, operation.call, 1).unwrap();
    let observation = gate.observation();
    assert!(gate.prepared_backing_bytes().unwrap() > core::mem::size_of::<CommitState>());
    let route = PendingFaceContributionRoute::prepare(
        FaceContributionContext {
            wake: &wake,
            plan: &plan,
            play: &play,
            host: &host,
            partition: &lowered.identity,
            node,
            call: operation.call,
            contract: &operation.contract_id,
        },
        observation,
    )
    .unwrap();
    assert!(
        route.additional_backing_bytes().unwrap() > core::mem::size_of::<FaceContributionScope>()
    );
    let reject = Arc::new(AtomicBool::new(false));
    let mut owned_gate = Some(gate);
    let backs = core::array::from_fn(|i| Back {
        gate: if i == usize::from(node.0) {
            owned_gate.take()
        } else {
            None
        },
        pending: false,
        done: false,
        candidate: 0,
        reject: reject.clone(),
        input_bytes: operation.binding.maximum_input_bytes,
    });
    let nodes = core::array::from_fn(|i| NodeSpec {
        input_cords: [if i == usize::from(node.0) {
            Some(CordId(0))
        } else {
            None
        }],
        maximum_step_fuel: 8,
    });
    let cords = [CordSpec::remote_ingress(
        CordId(0),
        RemoteEndpointId(0),
        (node, conduit_kernel::PortId(0)),
        CordCapacity {
            slot_start: 0,
            item_capacity: 1,
            byte_capacity: 1024,
            pressure_policy: Default::default(),
        },
    )];
    let mut routes = FixedRoutes::<0, 0>::new(1);
    routes.seal().unwrap();
    let mut calls = FixedHostCallBindings::<16>::new(1);
    calls
        .install(
            node,
            HostCallBinding {
                call: operation.call,
                maximum_input_bytes: operation.binding.maximum_input_bytes,
                maximum_output_bytes: 0,
            },
        )
        .unwrap();
    calls.seal().unwrap();
    let signs = FixedSignLog::<128>::new_with_remote_storage(
        (128 * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32,
        32,
        conduit_kernel::remote_sign_storage_bytes(32).unwrap(),
    )
    .unwrap();
    let mut kernel = Kernel::new_with_active_counts_and_host_calls(
        usize::from(node.0) + 1,
        1,
        nodes,
        cords,
        routes,
        calls,
        backs,
        FixedValueStore::new(4096).unwrap(),
        signs,
    )
    .unwrap();
    kernel
        .admit_remote_input(
            RemoteEndpointId(0),
            CordId(0),
            0,
            &vec![0; operation.binding.maximum_input_bytes as usize],
        )
        .unwrap();
    let request = (0..16)
        .find_map(|_| {
            kernel.step().unwrap();
            kernel.next_host_request()
        })
        .unwrap();
    Fixture {
        kernel,
        route,
        reject,
        request,
    }
}
fn fragment(route: &PendingFaceContributionRoute) -> FaceFragment {
    FaceFragment {
        basis: route.basis(),
        subjects: vec![conduit_face::FaceSubject {
            identity: "fixture/commit-inspection".into(),
            role: conduit_face::FaceRole::Semantic(c::kind_id("fixture/commit-inspection")),
            name: "Commit inspection".into(),
        }],
        relationships: vec![],
        composition: vec![],
        properties: vec![],
        text: vec![],
        actions: vec![],
        disclosures: vec![],
        temporal_references: vec![],
        temporal_facts: vec![],
    }
}
struct Capture {
    value: Option<CommittedFaceContribution>,
    pressure: bool,
}
impl BodyFaceContributionObserver for Capture {
    fn contribute(
        &mut self,
        value: CommittedFaceContribution,
    ) -> Result<(), ReturnedFaceContribution> {
        if self.pressure {
            Err(ReturnedFaceContribution {
                contribution: value,
                refusal: FaceContributionRefusal::Pressure,
            })
        } else {
            self.value = Some(value);
            Ok(())
        }
    }
}
fn complete(f: &mut Fixture) {
    f.kernel
        .complete_host_call(
            f.request.node,
            f.request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            },
        )
        .unwrap();
}
#[test]
fn actual_completion_is_not_committed_until_successful_resume_step() {
    let mut f = fixture();
    let face = fragment(&f.route);
    f.route.stage(&f.kernel, f.request, face).unwrap();
    let mut capture = Capture {
        value: None,
        pressure: false,
    };
    assert_eq!(f.route.publish(&mut capture), Ok(false));
    complete(&mut f);
    assert_eq!(f.route.publish(&mut capture), Ok(false));
    f.reject.store(true, Ordering::Relaxed);
    assert_eq!(f.kernel.step(), Err(SchedulerError::FalseProgress));
    assert_eq!(f.route.publish(&mut capture), Ok(false));
    f.kernel.step().unwrap();
    assert_eq!(f.route.publish(&mut capture), Ok(true));
    let committed = capture.value.unwrap();
    assert_eq!(committed.request(), f.request.request);
    assert_eq!(committed.scope().node(), f.request.node);
    assert_eq!(
        committed.fragment().basis.plan_id,
        committed.scope().body_play().plan_id
    );
    assert_eq!(
        f.route.publish(&mut Capture {
            value: None,
            pressure: false
        }),
        Ok(false)
    );
}
#[test]
fn pressure_retains_exact_contribution_and_rejects_overwrite() {
    let mut f = fixture();
    let face = fragment(&f.route);
    f.route.stage(&f.kernel, f.request, face).unwrap();
    complete(&mut f);
    f.kernel.step().unwrap();
    let mut capture = Capture {
        value: None,
        pressure: true,
    };
    assert_eq!(
        f.route.publish(&mut capture),
        Err(FaceContributionRefusal::Pressure)
    );
    assert!(capture.value.is_none());
    let other = fragment(&f.route);
    assert_eq!(
        f.route.stage(&f.kernel, f.request, other).unwrap_err().1,
        FaceContributionRefusal::Pressure
    );
    capture.pressure = false;
    assert_eq!(f.route.publish(&mut capture), Ok(true));
    assert_eq!(capture.value.unwrap().request(), f.request.request);
}
#[test]
fn wrong_request_basis_and_cancelled_execution_never_publish() {
    let mut f = fixture();
    let mut wrong = f.request;
    wrong.request = RequestId(1);
    let face = fragment(&f.route);
    assert_eq!(
        f.route.stage(&f.kernel, wrong, face).unwrap_err().1,
        FaceContributionRefusal::Scope
    );
    let mut face = fragment(&f.route);
    face.basis.active_play_id = "old/play".into();
    assert_eq!(
        f.route.stage(&f.kernel, f.request, face).unwrap_err().1,
        FaceContributionRefusal::Scope
    );
    let face = fragment(&f.route);
    f.route.stage(&f.kernel, f.request, face).unwrap();
    f.kernel.cancel().unwrap();
    assert_eq!(
        f.route.publish(&mut Capture {
            value: None,
            pressure: false
        }),
        Err(FaceContributionRefusal::Revoked)
    );
}
#[test]
fn one_gate_cannot_be_attached_twice_or_reused_for_different_play() {
    let adapter = crate::hosted_creche::test_support::Adapter::new();
    let (host, plan, play, wake) = crate::hosted_creche::test_support::planned(&adapter);
    let lowered =
        conduit_plan_lowering::lowering::lower_plan_fragment(&plan.plots[0].plan.fragments[0])
            .unwrap();
    let operation = &lowered.host_calls[0];
    let gate = FaceCommitGate::prepare(
        lowered
            .identity
            .placement_for_node(operation.node)
            .unwrap()
            .clone(),
        operation.call,
        1,
    )
    .unwrap();
    let context = || FaceContributionContext {
        wake: &wake,
        plan: &plan,
        play: &play,
        host: &host,
        partition: &lowered.identity,
        node: operation.node,
        call: operation.call,
        contract: &operation.contract_id,
    };
    let mut stale = play.clone();
    stale.play_sequence += 1;
    let mut bad = context();
    bad.play = &stale;
    assert!(PendingFaceContributionRoute::prepare(bad, gate.observation()).is_err());
    assert!(PendingFaceContributionRoute::prepare(context(), gate.observation()).is_ok());
    assert!(PendingFaceContributionRoute::prepare(context(), gate.observation()).is_err());
}
