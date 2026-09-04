use conduit_body::{Body, BodyFormPlan, BodyPlan, BodyPlayIdentity, ResidentForm};
use conduit_core::{
    seal_plan, CheckedFormId, ExpandedFormId, FormIdentity, Plan, SignId, SourceDocumentId,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_kernel::scheduler::{
    CordCapacity, CordSpec, FixedScheduler, NodeSpec, SchedulerStatus, StepInputBytes, StepIo,
    StepOperation, StepOutcome,
};
use conduit_kernel::{
    CordId, FixedRoutes, FixedSignLog, FixedValueStore, KernelEvent, NodeId, PortId, RouteRange,
    RouteTarget, ValueRef, ValueStorage,
};
use std::collections::BTreeSet;

const PORTS: usize = 1;
const FORMS: usize = 3;
const NODES: usize = FORMS * 2;
const CORDS: usize = FORMS;
const SIGNS: usize = 64;
const EVIDENCE_MARKER: &str = "CONDUIT_FORM_EVIDENCE=";

const MORSE_NETWORK: &str = include_str!("../../../forms/morse-network/main.conduit");
const MEMORY_LANTERN: &str = include_str!("../../../forms/memory-lantern/main.conduit");
const DESK_TELEGRAPH: &str = include_str!("../../../forms/desk-telegraph/main.conduit");

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

fn canonical_constituent(
    source: &str,
    root: &str,
    host: &conduit_std_host::StdHost,
) -> (ResidentForm, Plan) {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profiles)
        .expect("the installed text pipeline catalogs are disjoint");
    let syntax = parse_syntax_document(source);
    assert_eq!(syntax.round_trip(), source);
    let checked = check_syntax_document(&syntax, &startup).expect("reviewed Form checks");
    let expanded = expand_canonical_form(&checked, root, &profiles).expect("reviewed Form expands");
    let resident = ResidentForm::new(
        expanded.source_document_id.clone(),
        expanded.checked_form_id.clone(),
    );
    let plan = host
        .plan_expanded_local(&expanded)
        .expect("reviewed Form plans onto the exact std Host offers");
    (resident, plan)
}

#[test]
fn three_reviewed_forms_progress_in_one_body_play_through_one_production_kernel_scheduler() {
    let host = conduit_std_host::StdHost::new();
    let (morse, morse_plan) = canonical_constituent(MORSE_NETWORK, "morse_network", &host);
    let (lantern, lantern_plan) = canonical_constituent(MEMORY_LANTERN, "memory_lantern", &host);
    let (telegraph, telegraph_plan) =
        canonical_constituent(DESK_TELEGRAPH, "desk_telegraph", &host);
    let body = Body::born(
        morse.source_document_id.clone(),
        morse.checked_form_id.clone(),
        1,
        SignId::from("sign/born"),
    )
    .unwrap()
    .admit_form(lantern.clone(), SignId::from("sign/lantern-admitted"))
    .unwrap()
    .admit_form(telegraph.clone(), SignId::from("sign/telegraph-admitted"))
    .unwrap();
    let (_awake, wake) = body.wake(1, SignId::from("sign/woke")).unwrap();
    let plan = BodyPlan::seal(
        &wake,
        vec![
            BodyFormPlan {
                form: morse,
                plan: morse_plan,
            },
            BodyFormPlan {
                form: lantern,
                plan: lantern_plan,
            },
            BodyFormPlan {
                form: telegraph,
                plan: telegraph_plan,
            },
        ],
    )
    .unwrap();
    let play = BodyPlayIdentity::bind(&plan, 1);
    let body_plan_id = plan.plan_id.clone();
    let active_play_id = play.active_play_id.clone();
    let workload_revision = plan.workload_revision;
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
    for (index, source, sink) in [(0, 0, 1), (1, 2, 3), (2, 4, 5)] {
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
            node(None),
            node(Some(CordId(2))),
        ],
        [cord(0, 0, 1), cord(1, 2, 3), cord(2, 4, 5)],
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
            FormDriver {
                form: 2,
                work: Work::Source {
                    value: values.store(&[3]).unwrap(),
                    emitted: false,
                },
            },
            FormDriver {
                form: 2,
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
    assert!(matches!(
        scheduler.drivers()[5].work,
        Work::Sink { received: true }
    ));
    assert_eq!(scheduler.drivers()[0].form, 0);
    assert_eq!(scheduler.drivers()[2].form, 1);
    assert_eq!(scheduler.drivers()[4].form, 2);
    assert_eq!(playing.plans.len(), 1);
    assert_eq!(
        playing.plans[0].active_play_id,
        Some(active_play_id.clone())
    );
    println!(
        "{EVIDENCE_MARKER}{{\"plan_id\":\"{}\",\"play_id\":\"{}\",\"workload_revision\":{workload_revision}}}",
        body_plan_id.as_str(),
        active_play_id.as_str()
    );
}

#[test]
fn the_same_body_plan_model_covers_local_and_distributed_form_partitions() {
    let distributed = conduit_semantic_catalog::exact_body_coordination_plan(
        conduit_core::BootId::from("forebrain/boot"),
        conduit_core::BootId::from("motherbrain/boot"),
        "line/interbrain",
    )
    .unwrap();
    let distributed_form = ResidentForm::new(
        distributed.plan.source_document_id.clone(),
        distributed.plan.checked_form_id.clone(),
    );
    let local_form = resident("local-dashboard");
    let body = Body::born(
        distributed_form.source_document_id.clone(),
        distributed_form.checked_form_id.clone(),
        9,
        SignId::from("sign/distributed-born"),
    )
    .unwrap()
    .admit_form(local_form.clone(), SignId::from("sign/local-admitted"))
    .unwrap();
    let wake = body
        .wake(1, SignId::from("sign/distributed-woke"))
        .unwrap()
        .1;
    let body_plan = BodyPlan::seal(
        &wake,
        vec![
            BodyFormPlan {
                form: distributed_form,
                plan: distributed.plan,
            },
            BodyFormPlan {
                form: local_form.clone(),
                plan: constituent(&local_form),
            },
        ],
    )
    .unwrap();
    let hosts = body_plan
        .forms
        .iter()
        .find(|partition| partition.form != local_form)
        .unwrap()
        .plan
        .fragments
        .iter()
        .map(|fragment| fragment.host_id.clone())
        .collect::<BTreeSet<_>>();

    assert_eq!(body_plan.forms.len(), 2);
    assert_eq!(hosts.len(), 2);
    assert_eq!(body_plan.workset, wake.workset);
}
