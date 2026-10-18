//! Only byte sources and observation sinks are fixtures. Editing, dispatch and
//! terminal propagation use the installed Back and the ordinary fixed kernel.
use super::{prepare, InstalledBack, TextStateHost};
use conduit_core::{ConfigurationEntry, ConfigurationValue, PlannedGear};
use conduit_kernel::scheduler::{
    CordCapacity, CordSpec, FixedScheduler, NodeSpec, StepBack, StepInputBytes, StepIo, StepOutcome,
};
use conduit_kernel::{
    BoundedValueRef, CordEndpoint, CordId, Failure, FailureCode, FixedHostCallBindings,
    FixedRoutes, FixedSignLog, HostCallBinding, HostCallDisposition, HostCallId, HostCallOutcome,
    HostedValueStore, NodeId, PortId, RouteRange, RouteTarget, ValueRef, ValueStorage,
};
use conduit_semantic_catalog::{TextDocument, TextEdit};

#[allow(clippy::large_enum_variant)]
enum Driver {
    Source { values: Vec<ValueRef>, next: usize },
    Editor(InstalledBack),
    Sink(Vec<(u64, Vec<u8>)>),
}
impl StepBack<1> for Driver {
    fn step(&mut self, io: &mut StepIo<1>, bytes: &StepInputBytes<'_, 1>) -> StepOutcome {
        match self {
            Self::Editor(back) => back.step(io, bytes),
            Self::Source { values, next } => {
                let Some(value) = values.get(*next).copied() else {
                    return StepOutcome::Complete;
                };
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(PortId(0), value).unwrap();
                *next += 1;
                StepOutcome::Progress
            }
            Self::Sink(documents) => {
                if io.input(PortId(0)).is_some() {
                    let document = TextDocument::decode(bytes.input(PortId(0)).unwrap()).unwrap();
                    documents.push((document.revision, document.text.to_vec()));
                    io.consume(PortId(0)).unwrap();
                    StepOutcome::Progress
                } else if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0)).unwrap();
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
        }
    }
}
fn placement(maximum: u64) -> PlannedGear {
    let offer = conduit_std_offers::text_edit_document_std_offer();
    conduit_core::planned_gear_from_parts! {
        semantic_contract: conduit_semantic_catalog::text_edit_document_semantic_contract().semantic_contract(),
        placement_id: "editor-placement".into(), gear_id: "editor".into(),
        kind_id: offer.kind_id, kind_contract_revision: offer.kind_contract_revision,
        execution_profile_id: offer.implementation.execution_profile_id,
        configuration: vec![ConfigurationEntry { key: "maximum-bytes".into(), value: ConfigurationValue::U64(maximum) }],
        host_id: "editor-host".into(), boot_id: "editor-boot".into(),
        offer_generation: conduit_core::OfferGeneration(1), capability_id: offer.capability_id,
        implementation_id: offer.implementation.implementation_id, artifact_id: offer.implementation.artifact_id,
        base: None, realization_characteristics: vec![], limits: offer.limits,
        inputs: offer.inputs, outputs: offer.outputs, terminal_transductions: vec![], host_calls: offer.host_calls,
        resources: vec![], authority: vec![], pool_references: vec![],
    }
}
fn edit(base_revision: u64, start: u32, end: u32, text: &[u8]) -> Vec<u8> {
    let mut encoded = vec![0; conduit_semantic_catalog::MAXIMUM_TEXT_EDIT_BYTES as usize];
    let size = TextEdit {
        base_revision,
        start,
        end,
        insert_text: text,
    }
    .encode_into(&mut encoded)
    .unwrap();
    encoded.truncate(size);
    encoded
}
fn play(edits: Vec<Vec<u8>>, maximum: u64) -> (Vec<(u64, Vec<u8>)>, Option<Failure>) {
    let placement = placement(maximum);
    let mut values = HostedValueStore::new(64, 272, 16384).unwrap();
    let editor = prepare(&placement, &mut values).unwrap();
    let mut host = TextStateHost::from_placement(&placement).unwrap().unwrap();
    let inputs = edits
        .iter()
        .map(|bytes| values.store(bytes).unwrap())
        .collect();
    let before = values.allocation_capacities();
    let mut routes = FixedRoutes::<3, 2>::new(1);
    for index in 0..2 {
        routes
            .install(
                NodeId(index),
                PortId(0),
                RouteRange {
                    start: index,
                    len: 1,
                },
                &[RouteTarget {
                    cord: CordId(index),
                    sink: CordEndpoint::local(NodeId(index + 1), PortId(0)),
                }],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let mut bindings = FixedHostCallBindings::<3>::new(1);
    bindings
        .install(
            NodeId(1),
            HostCallBinding {
                call: HostCallId(0),
                maximum_input_bytes: 272,
                maximum_output_bytes: 264,
            },
        )
        .unwrap();
    bindings.seal().unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, 3, 2, 1, 2, 3, 2, 3, 1>::new_with_host_calls(
        [None, Some(CordId(0)), Some(CordId(1))].map(|input| NodeSpec {
            input_cords: [input],
            maximum_step_fuel: 3,
        }),
        [0, 1].map(|index| {
            CordSpec::local(
                CordId(index),
                (NodeId(index), PortId(0)),
                (NodeId(index + 1), PortId(0)),
                CordCapacity {
                    slot_start: index,
                    item_capacity: 1,
                    byte_capacity: 272,
                    pressure_policy: Default::default(),
                },
            )
        }),
        routes,
        bindings,
        [
            Driver::Source {
                values: inputs,
                next: 0,
            },
            Driver::Editor(editor),
            Driver::Sink(Vec::new()),
        ],
        values,
        FixedSignLog::<256>::new(
            (core::mem::size_of::<conduit_kernel::KernelEvent>() * 256) as u32,
        )
        .unwrap(),
    )
    .unwrap();
    let mut failure = None;
    for _ in 0..256 {
        if let Err(error) = scheduler.step() {
            assert_eq!(
                error,
                conduit_kernel::scheduler::SchedulerError::BackFailed(
                    failure.expect("only a refused Host Call may fail the installed Back")
                )
            );
            break;
        }
        if let Some(request) = scheduler.next_host_request() {
            let input = scheduler
                .values()
                .get(request.input.value)
                .unwrap()
                .to_vec();
            let (output, refused) = match host.execute(&input) {
                Ok(Some(bytes)) => (Some(bytes.to_vec()), None),
                Ok(None) => (None, None),
                Err(refusal) => (None, Some(refusal)),
            };
            failure = refused.or(failure);
            let output = output.map(|bytes| {
                BoundedValueRef::new(scheduler.store_host_value(&bytes).unwrap(), 264).unwrap()
            });
            scheduler
                .complete_host_call(
                    request.node,
                    request.request,
                    HostCallOutcome {
                        disposition: if refused.is_some() {
                            HostCallDisposition::Failed
                        } else {
                            HostCallDisposition::Completed
                        },
                        output,
                        failure: refused,
                    },
                )
                .unwrap();
        }
    }
    assert_eq!(before, scheduler.values().allocation_capacities());
    assert_eq!(
        scheduler
            .signs()
            .events()
            .any(|event| event.node == NodeId(1)
                && event.kind == conduit_kernel::KernelEventKind::BackFailed),
        failure.is_some()
    );
    let Driver::Sink(documents) = &scheduler.drivers()[2] else {
        panic!("sink")
    };
    (documents.clone(), failure)
}
#[test]
fn text_state_installed_document_runs_cafe_replacements_and_terminal_refusals() {
    let initial = edit(0, 0, 0, "café".as_bytes());
    let (documents, failure) = play(vec![initial.clone(), edit(1, 3, 4, b"e")], 8);
    assert_eq!(
        documents,
        vec![
            (0, vec![]),
            (1, "café".as_bytes().to_vec()),
            (2, b"cafe".to_vec())
        ]
    );
    assert_eq!(failure, None);
    let mut invalid_utf8 = edit(1, 0, 0, b"x");
    *invalid_utf8.last_mut().unwrap() = 0xff;
    for (bad, code, detail) in [
        (invalid_utf8, FailureCode::InvalidInput, 4),
        (edit(0, 0, 0, b"x"), FailureCode::InvalidInput, 2),
        (edit(1, 5, 5, b""), FailureCode::InvalidInput, 3),
        (
            edit(1, 4, 4, b"four"),
            FailureCode::StateCapacityExhausted,
            6,
        ),
    ] {
        let (documents, failure) = play(vec![initial.clone(), bad], 8);
        assert_eq!(
            documents,
            vec![(0, vec![]), (1, "café".as_bytes().to_vec())]
        );
        assert_eq!(failure, Some(Failure { code, detail }));
    }
}
