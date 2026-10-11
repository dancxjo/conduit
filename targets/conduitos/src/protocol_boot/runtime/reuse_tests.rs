//! Scheduler/Source conformance below USB physics: no controller or possession.
use super::*;
use crate::{protocol_source::*, pure_protocol_owner::PureProtocolOwner};
use alloc::{boxed::Box, collections::BTreeMap, string::String};
use conduit_composite::*;
use conduit_kernel::{
    HostCallDisposition, HostCallOutcome, HostedValueStore,
    scheduler::{HostCallBack, StepBack},
};
use conduit_plan_lowering::lowering::{FIXED_KERNEL_STORAGE_PORTS_PER_NODE, lower_plan_fragment};

const FIXTURE: &str = "fixture/usb-control-observation@1";
struct ObservationFactory(ImplementationId);
impl KernelOperationFactory for ObservationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.0
    }
    fn budget(&self, gear: &PlannedGear) -> Result<KernelOperationBudget, String> {
        assert_eq!(gear.implementation_id.as_str(), FIXTURE);
        assert!(gear.resources.is_empty() && gear.authority.is_empty() && gear.base.is_none());
        Ok(KernelOperationBudget {
            value_items: 4,
            value_bytes: 4096 * 4,
            maximum_value_bytes: 4096,
            host_requests: 1,
            sign_items: 64,
        })
    }
    fn prepare(
        &self,
        gear: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<FIXED_KERNEL_STORAGE_PORTS_PER_NODE> + Send>, String> {
        self.budget(gear)?;
        Ok(Box::new(HostCallBack::new(4096)))
    }
}
fn zero(ty: &StructuredInfoType) -> StructuredInfoValue {
    match ty.shape() {
        StructuredInfoTypeShape::Leaf(kind) => StructuredInfoValue::leaf(
            ty.clone(),
            vec![
                0;
                match kind.as_str() {
                    "value/u64" => 8,
                    "value/u16" => 2,
                    EMPTY_INFO_ID => 0,
                    _ => 1,
                }
            ],
        )
        .unwrap(),
        StructuredInfoTypeShape::Sequence { .. } => {
            StructuredInfoValue::sequence(ty.clone(), vec![]).unwrap()
        }
        StructuredInfoTypeShape::Record { fields, .. } => StructuredInfoValue::record(
            ty.clone(),
            fields
                .iter()
                .map(|field| {
                    StructuredFieldValue::new(field.name(), zero(field.value_type())).unwrap()
                })
                .collect(),
        )
        .unwrap(),
        _ => panic!("unexpected fixture schema"),
    }
}
fn run(
    storage: KernelCompositeSignStorage,
) -> Result<(usize, usize, bool), (usize, Box<KernelCompositeError>)> {
    let reference = |name: &str| ProtocolValueReference {
        type_name: name.into(),
        maximum_bytes: 4096,
    };
    let package = ProtocolSourcePackage::compile(
        include_str!("../../../plots/usb/control-reuse.conduit").into(),
        &[
            ProtocolSpecializationRequest::SeededUntil {
                value: reference("UsbControlReuseState"),
            },
            ProtocolSpecializationRequest::FeedbackZip {
                left: reference("UsbControlReuseState"),
                right: reference("UsbControlResult"),
            },
        ],
    )
    .unwrap();
    let entry =
        PreparedProtocolEntry::prepare(&serde_json::to_vec(&package).unwrap(), "usb-control-reuse")
            .unwrap();
    let seed_schema = entry.input_schema(&port_id("seed")).unwrap().clone();
    let mut host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "fixture/reuse".into(),
        boot_id: "fixture/reuse-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "conduitos/native@1".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    entry.publish_pure_backs(&mut host).unwrap();
    let contract = crate::usb_base::control_contract::ControlContract::prepare().unwrap();
    let mut offer = crate::usb_base::control_offer::offer(&contract, FIXTURE.into());
    // This synthetic observation Back claims no physical resource or authority.
    offer.capability_id = FIXTURE.into();
    offer.implementation.implementation_id = FIXTURE.into();
    offer.implementation.execution_profile_id = FIXTURE.into();
    offer.resource_requirements.clear();
    offer.authority_requirements.clear();
    host.capabilities.push(offer);
    let hosts = [host];
    let placements =
        conduit_planner::default_expanded_placements(&entry.expanded().expanded, &hosts).unwrap();
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    let options = conduit_planner::PlanningOptions {
        connection_bases: &empty,
        line_candidates: &lines,
        connection_item_capacity: 1,
        connection_byte_capacity: 4096,
        authority_grants: &[],
        protected_resource_grants: &[],
        line_offers: &[],
    };
    let prepared = entry
        .plan(
            &hosts,
            &placements,
            &["conduit.base/local@1".into()],
            options,
        )
        .unwrap();
    let (definition, operations) = prepared.into_preparation_parts();
    let fragment = &definition.internal_plan.fragments[0];
    let lowered = lower_plan_fragment(fragment).unwrap();
    let active = bind_active_play(
        &definition.internal_plan.plan_id,
        &fragment.host_id,
        &fragment.boot_id,
        0,
    );
    let identity = PreparationHostIdentity {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        offer_generation: fragment.offer_generation,
    };
    let mut owners = Vec::new();
    let mut physical_node = None;
    for gear in &fragment.placements {
        let node = lowered
            .identity
            .node_for_placement(&gear.placement_id)
            .unwrap();
        if gear.implementation_id.as_str() == FIXTURE {
            physical_node = Some(node);
        }
        if let Some(owner) = PureProtocolOwner::prepare(fragment, &lowered, &active, gear).unwrap()
        {
            owners.push((node, owner));
        }
    }
    let physical_node = physical_node.unwrap();
    let mut registry = KernelOperationRegistry::new();
    registry
        .install(ObservationFactory(FIXTURE.into()))
        .unwrap();
    registry
        .install(crate::expression_host_call::ExpressionOperationFactory::default())
        .unwrap();
    registry
        .install(crate::structured_selector_host_call::SelectorOperationFactory::default())
        .unwrap();
    registry.install(operations.joins).unwrap();
    registry.install(operations.states).unwrap();
    registry.install(operations.concats).unwrap();
    registry.install(operations.merges).unwrap();
    let mut outputs: Vec<_> = definition
        .external_capability
        .outputs
        .iter()
        .map(|port| {
            (
                port.port_id.clone(),
                ValuePayload {
                    value_kind: port.value_kind.clone(),
                    encoded: Vec::with_capacity(16384),
                },
            )
        })
        .collect();
    let seed_port = port_id("seed");
    let seed = ValuePayload {
        value_kind: seed_schema.profile().unwrap().value_kind().clone(),
        encoded: zero(&seed_schema).canonical_bytes().unwrap(),
    };
    let StructuredInfoTypeShape::Variant { cases, .. } = contract.result_type().shape() else {
        panic!()
    };
    let completed = cases.iter().find(|case| case.tag() == "completed").unwrap();
    let result = StructuredInfoValue::variant(
        contract.result_type().clone(),
        "completed",
        zero(completed.payload_type()),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let mut kernel = KernelCompositeHost::prepare_with_sign_storage(definition, &registry, storage)
        .map_err(|error| (0, Box::new(error)))?;
    let mut results = 0;
    let mut ended = 0;
    let mut complete = false;
    let mut outcome = None;
    let mut play_error = None;
    let allocations = crate::test_allocations::allocations(|| {
        outcome = Some((|| {
            kernel.start().unwrap();
            kernel.admit_input(&seed_port, 0, &seed).unwrap();
            kernel.close_input(&seed_port).unwrap();
            for _ in 0..10000 {
                let status = kernel.step().map_err(|error| {
                    play_error = Some(error);
                    results
                })?;
                if let Some(request) = kernel.next_host_request() {
                    let admitted = kernel
                        .admit_host_request(&request, &identity, &[], &[])
                        .map_err(|error| {
                            play_error = Some(error);
                            results
                        })?;
                    let call = *kernel
                        .admitted_host_request_view(&admitted)
                        .unwrap()
                        .request;
                    if call.node == physical_node {
                        kernel
                            .complete_host_call_bytes(&admitted, &result)
                            .map_err(|error| {
                                play_error = Some(error);
                                results
                            })?;
                    } else {
                        let (_, owner) = owners
                            .iter_mut()
                            .find(|(node, _)| *node == call.node)
                            .unwrap();
                        let input = kernel.host_request_input(&admitted).unwrap();
                        match owner
                            .invoke(call.node, call.call, call.request, input)
                            .unwrap()
                        {
                            Some(bytes) => kernel
                                .complete_host_call_bytes(&admitted, bytes)
                                .map_err(|error| {
                                    play_error = Some(error);
                                    results
                                })?,
                            None => kernel
                                .complete_host_call(
                                    &admitted,
                                    HostCallOutcome {
                                        disposition: HostCallDisposition::Completed,
                                        output: None,
                                        failure: None,
                                    },
                                )
                                .map_err(|error| {
                                    play_error = Some(error);
                                    results
                                })?,
                        }
                    }
                }
                for (port, output) in &mut outputs {
                    if let Some(sequence) = kernel.output_into(port, output).map_err(|error| {
                        play_error = Some(error);
                        results
                    })? {
                        if port.as_str() == "result" {
                            results += 1;
                        } else {
                            ended += 1;
                        }
                        kernel.complete_output(port, sequence).map_err(|error| {
                            play_error = Some(error);
                            results
                        })?;
                    }
                }
                if status == KernelCompositeStatus::Complete {
                    complete = true;
                    break;
                }
            }
            Ok((results, ended, complete))
        })());
    });
    assert_eq!(allocations, 0, "all Play storage is reserved before start");
    let outcome = outcome.unwrap().map_err(|results| {
        (
            results,
            Box::new(play_error.expect("retained exact Play refusal")),
        )
    });
    if outcome.as_ref().is_ok_and(|(_, _, complete)| *complete) {
        use conduit_kernel::KernelEventKind as Event;
        let remote_items = kernel
            .signs()
            .values()
            .flatten()
            .filter(|event| {
                matches!(
                    event.kind,
                    Event::RemoteValueOffered
                        | Event::RemoteValueAccepted
                        | Event::RemoteValueDelivered
                        | Event::RemoteOutputClosed
                        | Event::RemoteInputAdmitted
                        | Event::RemoteInputClosed
                )
            })
            .count();
        assert_eq!(remote_items, 3 * (64 + 1) + 2 + 2);

        let (_, ended) = outputs
            .iter()
            .find(|(port, _)| port.as_str() == "ended")
            .unwrap();
        let value = StructuredInfoValue::from_canonical_bytes(&ended.encoded).unwrap();
        let StructuredInfoValueShape::Record(fields) = value.shape() else {
            panic!()
        };
        let leaf = |name: &str| {
            let value = fields
                .iter()
                .find(|field| field.name() == name)
                .unwrap()
                .value();
            let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
                panic!()
            };
            bytes
        };
        assert_eq!(leaf("count"), 64u64.to_le_bytes());
        assert_eq!(leaf("terminal"), [1]);
    }
    outcome
}

#[test]
fn default_sign_storage_reports_exact_refusal_and_finite_extra_storage_closes_64_cycles() {
    let (results, error) = run(KernelCompositeSignStorage::default()).unwrap_err();
    let wrapped = crate::protocol_source::ProtocolBodyRefusal::Call(
        crate::protocol_host_calls::ProtocolCallRefusal::Kernel(*error),
    );
    assert_eq!(results, 37);
    assert_eq!(
        output_close_refusal(&wrapped),
        "protocol-output-sign-remote-items-exhausted"
    );
    assert_eq!(
        run(KernelCompositeSignStorage {
            additional_local_items: 0,
            // Three remote Signs per result/ended frame, two per seed,
            // and one closure per output; reserve the entire finite trace.
            additional_remote_items: 3 * (64 + 1) + 2 + 2
        })
        .unwrap(),
        (64, 1, true)
    );
}

#[test]
fn output_close_diagnostic_preserves_each_finite_sign_refusal() {
    use conduit_kernel::{SignError, scheduler::SchedulerError};
    for (reason, expected) in [
        (
            SignError::InvalidBudget,
            "protocol-output-sign-invalid-budget",
        ),
        (
            SignError::ItemCapacityExceeded,
            "protocol-output-sign-items-exhausted",
        ),
        (
            SignError::ByteCapacityExceeded,
            "protocol-output-sign-bytes-exhausted",
        ),
        (
            SignError::RemoteItemCapacityExceeded,
            "protocol-output-sign-remote-items-exhausted",
        ),
        (
            SignError::RemoteByteCapacityExceeded,
            "protocol-output-sign-remote-bytes-exhausted",
        ),
        (
            SignError::SequenceOverflow,
            "protocol-output-sign-sequence-overflow",
        ),
    ] {
        let error = crate::protocol_source::ProtocolBodyRefusal::Call(
            crate::protocol_host_calls::ProtocolCallRefusal::Kernel(
                KernelCompositeError::Execution {
                    child: 0,
                    reason: ChildExecutionError::Scheduler(SchedulerError::Sign(reason)),
                },
            ),
        );
        assert_eq!(output_close_refusal(&error), expected);
    }
    assert_eq!(
        output_close_refusal(&crate::protocol_source::ProtocolBodyRefusal::NotPlaying),
        "protocol-output-close-refused"
    );
}

#[test]
fn remote_sign_admission_overflow_refuses_before_play() {
    let (results, error) = run(KernelCompositeSignStorage {
        additional_local_items: 0,
        additional_remote_items: u16::MAX,
    })
    .unwrap_err();
    assert_eq!(results, 0);
    assert!(
        matches!(*error, KernelCompositeError::ChildRefused { reason, .. }
        if reason == "kernel composite remote Sign bound overflow")
    );
}
