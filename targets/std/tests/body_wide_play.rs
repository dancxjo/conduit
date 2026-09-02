use conduit_body::{Body, BodyFormPlan, BodyPlan, BodyPlayIdentity, ResidentForm};
use conduit_core::{
    seal_plan, CheckedFormId, ExpandedFormId, FormIdentity, SignId, SourceDocumentId,
};
use conduit_kernel::scheduler::{
    CordCapacity, CordSpec, FixedScheduler, NodeSpec, SchedulerStatus, StepInputBytes, StepIo,
    StepOperation, StepOutcome,
};
use conduit_kernel::{
    CordId, FixedRoutes, FixedSignLog, FixedValueStore, KernelEvent, NodeId, PortId, RouteRange,
    RouteTarget, ValueRef, ValueStorage,
};

const PORTS: usize = 1;
const FORMS: usize = 2;
const NODES: usize = FORMS * 2;
const CORDS: usize = FORMS;
const SIGNS: usize = 64;

#[derive(Clone, Copy)]
enum Work {
    Source { value: ValueRef, emitted: bool },
    Sink { received: bool },
}

#[derive(Clone, Copy)]
struct FormDriver {
    form: usize,
    work: Work,
}

impl StepOperation<PORTS> for FormDriver {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match &mut self.work {
            Work::Source { value, emitted } if !*emitted => {
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(PortId(0), *value).unwrap();
                *emitted = true;
                StepOutcome::Progress
            }
            Work::Source { .. } => StepOutcome::Complete,
            Work::Sink { received } if !*received => {
                if io.input(PortId(0)).is_some() {
                    io.consume(PortId(0)).unwrap();
                    *received = true;
                    StepOutcome::Progress
                } else {
                    StepOutcome::Await
                }
            }
            Work::Sink { .. } if io.input_closed(PortId(0)) => {
                io.consume_closed(PortId(0)).unwrap();
                StepOutcome::Complete
            }
            Work::Sink { .. } => StepOutcome::Await,
        }
    }

    fn cancel(&mut self) {}
}

fn resident(name: &str) -> ResidentForm {
    ResidentForm::new(
        SourceDocumentId::from(format!("source/{name}")),
        CheckedFormId::from(format!("checked/{name}")),
    )
}

fn constituent(form: &ResidentForm) -> conduit_core::Plan {
    seal_plan(
        FormIdentity {
            source_document_id: form.source_document_id.clone(),
            checked_form_id: form.checked_form_id.clone(),
            expanded_form_id: ExpandedFormId::from(format!(
                "expanded/{}",
                form.checked_form_id.as_str()
            )),
        },
        vec![],
    )
}

#[test]
fn two_forms_progress_in_one_body_play_through_one_production_kernel_scheduler() {
    let first = resident("dashboard");
    let second = resident("counter");
    let body = Body::born(
        first.source_document_id.clone(),
        first.checked_form_id.clone(),
        1,
        SignId::from("sign/born"),
    )
    .unwrap()
    .admit_form(second.clone(), SignId::from("sign/counter-admitted"))
    .unwrap();
    let (_awake, wake) = body.wake(1, SignId::from("sign/woke")).unwrap();
    let plan = BodyPlan::seal(
        &wake,
        vec![
            BodyFormPlan {
                form: first.clone(),
                plan: constituent(&first),
            },
            BodyFormPlan {
                form: second.clone(),
                plan: constituent(&second),
            },
        ],
    )
    .unwrap();
    let play = BodyPlayIdentity::bind(&plan, 1);
    let playing = wake
        .body_plan_ready(&plan, SignId::from("sign/planned"))
        .unwrap()
        .body_play_started(&plan, &play, SignId::from("sign/playing"))
        .unwrap();

    let mut values = FixedValueStore::<FORMS, 1>::new(FORMS as u32).unwrap();
    let first_value = values.store(&[1]).unwrap();
    let second_value = values.store(&[2]).unwrap();
    let node = |input| NodeSpec {
        input_cords: [input],
        maximum_step_work: 2,
    };
    let cord = |index: u16, source: u16, sink: u16| {
        CordSpec::local(
            CordId(index),
            (NodeId(source), PortId(0)),
            (NodeId(sink), PortId(0)),
            CordCapacity {
                slot_start: index,
                item_capacity: 1,
                byte_capacity: 1,
            },
        )
    };
    let mut routes = FixedRoutes::<NODES, CORDS>::new(PORTS as u16);
    for (index, source, sink) in [(0, 0, 1), (1, 2, 3)] {
        routes
            .install(
                NodeId(source),
                PortId(0),
                RouteRange {
                    start: index,
                    len: 1,
                },
                &[RouteTarget {
                    cord: CordId(index),
                    sink: conduit_kernel::CordEndpoint::local(NodeId(sink), PortId(0)),
                }],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let sign_bytes = u32::try_from(SIGNS * core::mem::size_of::<KernelEvent>()).unwrap();
    let signs = FixedSignLog::<SIGNS>::new(sign_bytes).unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, NODES, CORDS, PORTS, FORMS, NODES, CORDS>::new(
        [
            node(None),
            node(Some(CordId(0))),
            node(None),
            node(Some(CordId(1))),
        ],
        [cord(0, 0, 1), cord(1, 2, 3)],
        routes,
        [
            FormDriver {
                form: 0,
                work: Work::Source {
                    value: first_value,
                    emitted: false,
                },
            },
            FormDriver {
                form: 0,
                work: Work::Sink { received: false },
            },
            FormDriver {
                form: 1,
                work: Work::Source {
                    value: second_value,
                    emitted: false,
                },
            },
            FormDriver {
                form: 1,
                work: Work::Sink { received: false },
            },
        ],
        values,
        signs,
    )
    .unwrap();

    scheduler.run(16).unwrap();
    assert_eq!(scheduler.step(), Ok(SchedulerStatus::Complete));
    assert!(matches!(
        scheduler.drivers()[1].work,
        Work::Sink { received: true }
    ));
    assert!(matches!(
        scheduler.drivers()[3].work,
        Work::Sink { received: true }
    ));
    assert_eq!(scheduler.drivers()[0].form, 0);
    assert_eq!(scheduler.drivers()[2].form, 1);
    assert_eq!(playing.plans.len(), 1);
    assert_eq!(playing.plans[0].active_play_id, Some(play.active_play_id));
}
