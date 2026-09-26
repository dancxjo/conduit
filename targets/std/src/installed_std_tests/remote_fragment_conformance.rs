use super::{host, installed_std};
use conduit_core::{
    process_owned_line_offer_with_limits, BaseImplementationId, GearId, LinkLimits,
};
use conduit_form::parse;
use conduit_planner::{plan_with_options, PlacementChoice, PlacementChoices, PlanningOptions};
use std::collections::BTreeMap;

#[test]
fn generic_remote_fragment_routes_through_latest_and_atomic_tee() {
    let form = parse(
        "form remote_typed_flow {\n source: conduit-test/scalar-source\n latest: state/latest\n split: flow/tee\n left: conduit-test/scalar-sink\n right: conduit-test/scalar-sink\n source.value >> latest.in\n latest.out >> split.in\n split.left >> left.in\n split.right >> right.in\n}\n",
        &installed_std::test_catalog(),
    )
    .unwrap();
    let mut source_host = host("remote-flow-source");
    let source = source_host.advertisement().clone();
    let middle = host("remote-flow-middle").advertisement().clone();
    let sink = host("remote-flow-sink").advertisement().clone();
    let hosts = [source.clone(), middle.clone(), sink.clone()];
    let capability = |host: &conduit_core::HostAdvertisement, kind: &str| {
        host.capabilities
            .iter()
            .find(|offer| offer.kind_id.as_str() == kind)
            .unwrap()
            .capability_id
            .clone()
    };
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("remote_typed_flow/source"),
                PlacementChoice {
                    host_id: source.host_id.clone(),
                    capability_id: capability(&source, "conduit-test/scalar-source"),
                },
            ),
            (
                GearId::from("remote_typed_flow/latest"),
                PlacementChoice {
                    host_id: middle.host_id.clone(),
                    capability_id: capability(&middle, conduit_semantic_catalog::LATEST_KIND),
                },
            ),
            (
                GearId::from("remote_typed_flow/split"),
                PlacementChoice {
                    host_id: middle.host_id.clone(),
                    capability_id: capability(&middle, conduit_semantic_catalog::TEE_KIND),
                },
            ),
            (
                GearId::from("remote_typed_flow/left"),
                PlacementChoice {
                    host_id: sink.host_id.clone(),
                    capability_id: capability(&sink, "conduit-test/scalar-sink"),
                },
            ),
            (
                GearId::from("remote_typed_flow/right"),
                PlacementChoice {
                    host_id: sink.host_id.clone(),
                    capability_id: capability(&sink, "conduit-test/scalar-sink"),
                },
            ),
        ]),
    };
    let limits = LinkLimits {
        maximum_in_flight_items: 1,
        maximum_payload_bytes: conduit_core::SCALAR_ENCODED_LEN as u32,
        maximum_buffered_bytes: conduit_core::SCALAR_ENCODED_LEN as u32,
        maximum_frame_bytes: 8_192,
    };
    let line =
        |id: &str, from: &conduit_core::HostAdvertisement, to: &conduit_core::HostAdvertisement| {
            process_owned_line_offer_with_limits(
                id,
                &format!("{id}-binding"),
                BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
                &format!("{id}-instance"),
                from,
                to,
                limits,
            )
        };
    let mut incoming = line("remote-flow-in", &source, &middle);
    let mut left = line("remote-flow-left", &middle, &sink);
    let mut right = line("remote-flow-right", &middle, &sink);
    for offered in [&mut incoming, &mut left, &mut right] {
        offered.contract.scope = conduit_core::LineScope::LocalNetwork;
        offered.contract.security = conduit_core::LineSecurity::PlaintextNetwork;
    }
    let line_candidates = BTreeMap::from([
        (
            (
                GearId::from("remote_typed_flow/source"),
                GearId::from("remote_typed_flow/latest"),
            ),
            vec![incoming.line_id.clone()],
        ),
        (
            (
                GearId::from("remote_typed_flow/split"),
                GearId::from("remote_typed_flow/left"),
            ),
            vec![left.line_id.clone()],
        ),
        (
            (
                GearId::from("remote_typed_flow/split"),
                GearId::from("remote_typed_flow/right"),
            ),
            vec![right.line_id.clone()],
        ),
    ]);
    let plan = plan_with_options(
        &form,
        &hosts,
        &placements,
        &[
            BaseImplementationId::from("conduit.base/local@1"),
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        ],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_core::SCALAR_ENCODED_LEN as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[incoming, left, right],
        },
    )
    .unwrap();
    let fragment = |host: &conduit_core::HostAdvertisement| {
        plan.fragments
            .iter()
            .find(|fragment| fragment.host_id == host.host_id)
            .unwrap()
    };
    let mut source_runtime = source_host
        .prepare_remote_fragment(fragment(&source))
        .unwrap();
    assert_eq!(
        source_runtime.identity().active_play_id,
        source_runtime
            .sessions()
            .iter()
            .next()
            .unwrap()
            .binding()
            .source_active_play_id
    );
    assert!(source_host
        .prepare_remote_fragment(fragment(&source))
        .err()
        .unwrap()
        .contains("combined active-instance limit exceeded"));
    let mut middle_runtime =
        crate::InstalledRemoteFragment::prepare(&middle, fragment(&middle), 1).unwrap();
    let mut sink_runtime =
        crate::InstalledRemoteFragment::prepare(&sink, fragment(&sink), 1).unwrap();
    assert_eq!(fragment(&middle).placements.len(), 2);
    let source_endpoint = source_runtime.sessions().iter().next().unwrap().endpoint;
    let middle_ingress = middle_runtime
        .sessions()
        .iter()
        .find(|session| {
            session.direction == conduit_plan_lowering::lowering::RemoteCordDirection::Ingress
        })
        .unwrap()
        .endpoint;
    crate::remote_cord_sessions::activate_in_process(
        source_runtime
            .sessions_mut()
            .get_mut(source_endpoint)
            .unwrap(),
        middle_runtime
            .sessions_mut()
            .get_mut(middle_ingress)
            .unwrap(),
    )
    .unwrap();
    let transfer = (0..8)
        .find_map(|_| {
            if let Some(transfer) = source_runtime.next_egress(source_endpoint).unwrap() {
                return Some(transfer);
            }
            if let Some(request) = source_runtime.next_host_request() {
                let work = source_runtime.describe_host_request(request).unwrap();
                assert_eq!(work.request, request);
                assert_eq!(
                    work.contract_id,
                    conduit_core::wait_host_call_requirement().contract_id
                );
                assert_eq!(work.input.len(), request.input.value.byte_len as usize);
                assert_eq!(work.maximum_output_bytes, 0);
                source_runtime
                    .complete_host_call(
                        request,
                        conduit_kernel::HostCallOutcome {
                            disposition: conduit_kernel::HostCallDisposition::Completed,
                            output: None,
                            failure: None,
                        },
                    )
                    .unwrap();
            }
            let _ = source_runtime.step().unwrap();
            None
        })
        .expect("literal reaches remote Cord");
    source_runtime.accept_egress(&transfer).unwrap();
    middle_runtime
        .admit_ingress(middle_ingress, transfer.sequence, &transfer.bytes)
        .unwrap();
    source_runtime.deliver_egress(&transfer).unwrap();
    assert_eq!(
        source_runtime
            .fail_remote_line(source_endpoint, 0)
            .unwrap_err(),
        "record remote Line failure: InvalidState"
    );
    source_runtime
        .fail_remote_line(source_endpoint, 73)
        .unwrap();
    assert!(source_runtime
        .next_egress(source_endpoint)
        .unwrap_err()
        .contains("Cancelled"));
    middle_runtime.close_ingress(middle_ingress).unwrap();
    let egress = middle_runtime
        .sessions()
        .iter()
        .filter(|session| {
            session.direction == conduit_plan_lowering::lowering::RemoteCordDirection::Egress
        })
        .map(|session| session.endpoint)
        .collect::<Vec<_>>();
    assert_eq!(egress.len(), 2);
    let mut sink_pairs = Vec::new();
    for endpoint in &egress {
        let binding = middle_runtime
            .sessions()
            .get(*endpoint)
            .unwrap()
            .binding()
            .clone();
        let sink_endpoint = sink_runtime
            .sessions()
            .iter()
            .find(|session| session.binding() == &binding)
            .unwrap()
            .endpoint;
        crate::remote_cord_sessions::activate_in_process(
            middle_runtime.sessions_mut().get_mut(*endpoint).unwrap(),
            sink_runtime.sessions_mut().get_mut(sink_endpoint).unwrap(),
        )
        .unwrap();
        sink_pairs.push((*endpoint, sink_endpoint));
    }
    let offers = (0..16)
        .find_map(|_| {
            let offers = egress
                .iter()
                .filter_map(|endpoint| middle_runtime.next_egress(*endpoint).unwrap())
                .collect::<Vec<_>>();
            if offers.len() == 2 {
                return Some(offers);
            }
            let _ = middle_runtime.step().unwrap();
            None
        })
        .expect("latest and tee atomically commit both remote branches");
    assert!(offers.iter().all(|offer| offer.bytes == transfer.bytes));
    for offer in &offers {
        let sink_endpoint = sink_pairs
            .iter()
            .find(|(source_endpoint, _)| *source_endpoint == offer.endpoint)
            .unwrap()
            .1;
        middle_runtime.accept_egress(offer).unwrap();
        assert_eq!(
            sink_runtime
                .admit_ingress(sink_endpoint, offer.sequence, &offer.bytes)
                .unwrap(),
            conduit_kernel::scheduler::RemoteIngressOutcome::Accepted {
                sequence: offer.sequence
            },
        );
        middle_runtime.deliver_egress(offer).unwrap();
        sink_runtime.close_ingress(sink_endpoint).unwrap();
    }
    source_host.release_remote_fragment(source_runtime).unwrap();
    let replacement = source_host
        .prepare_remote_fragment(fragment(&source))
        .expect("released remote capability and resource capacity is reusable");
    source_host.release_remote_fragment(replacement).unwrap();
}
