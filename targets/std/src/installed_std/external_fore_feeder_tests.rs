use super::*;

#[test]
fn sixty_four_values_fit_the_finite_fore_envelope_and_sixty_five_refuse() {
    let port = LoweredForePort {
        front_port_id: conduit_core::port_id("commands"),
        direction: PortDirection::Input,
        track: conduit_core::ConnectionTrack::Payload,
        endpoint: RemoteEndpointId(0),
        cord: CordId(0),
        value_kind: conduit_core::kind_id("test/command"),
        quantity_profile: conduit_core::QuantityValueProfile::Decimal,
        value_contract: Some(
            conduit_core::CheckedValueContract::new(
                conduit_core::kind_id("test/command"),
                1,
                vec![],
            )
            .unwrap(),
        ),
        abnormal_kind: None,
        temporal: PortTemporal::Flow { closes: true },
        item_capacity: 1,
        byte_capacity: 1,
        selected_line: None,
    };
    let values = (0..64)
        .map(|_| ExternalForeInput {
            front_port_id: conduit_core::port_id("commands"),
            track: conduit_core::ConnectionTrack::Payload,
            bytes: vec![1],
        })
        .collect::<Vec<_>>();
    let mut feeder = SequentialForeFeeder::prepare(&[&port], &values).unwrap();
    let mut ingress = PressuredIngress::default();
    for _ in 0..66 {
        feeder.feed_next(&mut ingress).unwrap();
    }
    assert_eq!(ingress.accepted, (0..64).collect::<Vec<_>>());
    assert_eq!(ingress.closed, vec![(RemoteEndpointId(0), CordId(0))]);
    let mut over = values.clone();
    over.push(values[0].clone());
    assert!(SequentialForeFeeder::prepare(&[&port], &over).is_err());
}

#[derive(Default)]
struct PressuredIngress {
    busy: bool,
    attempts: Vec<(u64, Vec<u8>)>,
    accepted: Vec<u64>,
    closed: Vec<(RemoteEndpointId, CordId)>,
}

impl ForeIngress for PressuredIngress {
    fn has_ready_work(&self) -> bool {
        self.busy
    }
    fn admit(
        &mut self,
        _: &[(RemoteEndpointId, CordId)],
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, String> {
        self.attempts.push((sequence, bytes.to_vec()));
        if self.attempts.len() == 2 {
            return Ok(RemoteIngressOutcome::Full { sequence });
        }
        self.accepted.push(sequence);
        Ok(RemoteIngressOutcome::Accepted { sequence })
    }

    fn close(&mut self, endpoint: RemoteEndpointId, cord: CordId) -> Result<(), String> {
        self.closed.push((endpoint, cord));
        Ok(())
    }
}

#[test]
fn full_retries_same_value_and_sequence_then_closes_once() {
    let inputs = [b"first".as_slice(), b"second".as_slice()]
        .into_iter()
        .map(|bytes| ExternalForeInput {
            front_port_id: conduit_core::port_id("values"),
            track: conduit_core::ConnectionTrack::Payload,
            bytes: bytes.to_vec(),
        })
        .collect::<Vec<_>>();
    let target = (RemoteEndpointId(0), CordId(0));
    let mut feeder = SequentialForeFeeder {
        inputs: &inputs,
        flows: vec![SequenceFlow {
            front: conduit_core::port_id("values"),
            track: conduit_core::ConnectionTrack::Payload,
            targets: vec![target],
            sequence: 0,
        }],
        input_flows: vec![0; inputs.len()],
        next: 0,
        closed: false,
        stream: None,
    };
    let mut ingress = PressuredIngress::default();
    feeder.feed_next(&mut ingress).unwrap();
    feeder.feed_next(&mut ingress).unwrap();
    assert!(feeder.is_pending());
    assert_eq!(ingress.accepted, vec![0]);
    feeder.feed_next(&mut ingress).unwrap();
    assert!(feeder.is_pending());
    assert!(ingress.closed.is_empty());
    feeder.feed_next(&mut ingress).unwrap();
    assert!(!feeder.is_pending());
    feeder.feed_next(&mut ingress).unwrap();
    assert_eq!(ingress.accepted, vec![0, 1]);
    assert_eq!(ingress.closed, vec![target]);
    assert_eq!(
        ingress.attempts,
        vec![
            (0, b"first".to_vec()),
            (1, b"second".to_vec()),
            (1, b"second".to_vec())
        ]
    );
}

#[test]
fn live_queue_retains_full_value_then_closes_once_after_drain() {
    let plan = crate::flow_activation::authored_todo_plan();
    let control = crate::RunControl::default();
    let queue = crate::BodyLiveForeQueue::for_todo_plan(&plan, control, 1).unwrap();
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment_for_profile_from_plan(
        &plan,
        &plan.fragments[0].fragment_id,
        crate::installed_std::state_storage_profile(),
    )
    .unwrap();
    let planned = lowered
        .fore_ports
        .iter()
        .filter(|port| port.direction == PortDirection::Input)
        .collect::<Vec<_>>();
    let mut feeder = LiveForeFeeder::prepare(&planned, &queue).unwrap();
    let first = conduit_todo_plot::TodoCommand::Add {
        text: "Milk".into(),
    }
    .encode_info()
    .unwrap();
    let second = conduit_todo_plot::TodoCommand::SetComplete {
        id: "task-1".into(),
        complete: true,
    }
    .encode_info()
    .unwrap();
    assert_eq!(
        queue.submit(&first).unwrap(),
        crate::BodyLiveForeAdmission::Accepted { sequence: 0 }
    );
    assert_eq!(
        queue.submit(&second).unwrap(),
        crate::BodyLiveForeAdmission::Full
    );
    let mut ingress = PressuredIngress::default();
    feeder.feed_next(&mut ingress).unwrap();
    assert_eq!(
        queue.submit(&second).unwrap(),
        crate::BodyLiveForeAdmission::Accepted { sequence: 1 }
    );
    queue.close().unwrap();
    feeder.feed_next(&mut ingress).unwrap();
    assert!(feeder.full);
    feeder.feed_next(&mut ingress).unwrap();
    feeder.feed_next(&mut ingress).unwrap();
    feeder.feed_next(&mut ingress).unwrap();
    assert!(feeder.closed);
    assert_eq!(ingress.accepted, vec![0, 1]);
    assert_eq!(ingress.attempts[1].1, ingress.attempts[2].1);
    assert_eq!(ingress.closed, feeder.targets);
    assert!(queue.submit(&first).is_err());
    assert!(queue.close().is_err());
}

#[test]
fn ready_kernel_work_defers_payload_admission_and_final_close() {
    let input = [ExternalForeInput {
        front_port_id: conduit_core::port_id("values"),
        track: conduit_core::ConnectionTrack::Payload,
        bytes: vec![1],
    }];
    let target = (RemoteEndpointId(0), CordId(0));
    let mut feeder = SequentialForeFeeder {
        inputs: &input,
        flows: vec![SequenceFlow {
            front: input[0].front_port_id.clone(),
            track: input[0].track,
            targets: vec![target],
            sequence: 0,
        }],
        input_flows: vec![0],
        next: 0,
        closed: false,
        stream: None,
    };
    let mut ingress = PressuredIngress {
        busy: true,
        ..Default::default()
    };
    feeder.feed_next(&mut ingress).unwrap();
    assert!(ingress.attempts.is_empty());
    ingress.busy = false;
    feeder.feed_next(&mut ingress).unwrap();
    assert_eq!(ingress.accepted, vec![0]);
    assert!(ingress.closed.is_empty());
    ingress.busy = true;
    feeder.feed_next(&mut ingress).unwrap();
    assert!(ingress.closed.is_empty());
    assert!(feeder.is_pending());
    ingress.busy = false;
    feeder.feed_next(&mut ingress).unwrap();
    assert_eq!(ingress.closed, vec![target]);
    assert!(!feeder.is_pending());
}
