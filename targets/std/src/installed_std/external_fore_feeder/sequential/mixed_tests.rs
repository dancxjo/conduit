use super::*;
fn port(name: &str, temporal: PortTemporal, id: u16) -> LoweredForePort {
    LoweredForePort {
        front_port_id: name.into(),
        direction: PortDirection::Input,
        track: ConnectionTrack::Payload,
        endpoint: RemoteEndpointId(id),
        cord: CordId(id),
        value_kind: "test/byte".into(),
        quantity_profile: conduit_core::QuantityValueProfile::Decimal,
        value_contract: Some(
            conduit_core::CheckedValueContract::new("test/byte".into(), 1, vec![]).unwrap(),
        ),
        abnormal_kind: None,
        temporal,
        item_capacity: 1,
        byte_capacity: 1,
        selected_line: None,
    }
}
fn input(name: &str, byte: u8) -> ExternalForeInput {
    ExternalForeInput {
        front_port_id: name.into(),
        track: ConnectionTrack::Payload,
        bytes: vec![byte],
    }
}
#[derive(Default)]
struct Ingress {
    full: bool,
    ready: bool,
    accepted: Vec<(u64, u8)>,
    closed: Vec<(RemoteEndpointId, CordId)>,
}
impl ForeIngress for Ingress {
    fn has_ready_work(&self) -> bool {
        self.ready
    }
    fn admit(
        &mut self,
        _: &[(RemoteEndpointId, CordId)],
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, String> {
        if self.full {
            self.full = false;
            return Ok(RemoteIngressOutcome::Full { sequence });
        }
        self.accepted.push((sequence, bytes[0]));
        Ok(RemoteIngressOutcome::Accepted { sequence })
    }
    fn close(&mut self, endpoint: RemoteEndpointId, cord: CordId) -> Result<(), String> {
        self.closed.push((endpoint, cord));
        Ok(())
    }
}
#[test]
fn value_and_flow_keep_exact_pressure_sequences_and_close_after_drain() {
    let value = port("profile", PortTemporal::Value, 0);
    let flow = port("sample", PortTemporal::Flow { closes: true }, 1);
    let inputs = [
        input("profile", 7),
        input("sample", 1),
        input("sample", 2),
        input("sample", 3),
    ];
    let mut feeder = SequentialForeFeeder::prepare(&[&value, &flow], &inputs).unwrap();
    let mut ingress = Ingress {
        full: true,
        ..Default::default()
    };
    feeder.feed_next(&mut ingress).unwrap();
    assert!(ingress.accepted.is_empty());
    for _ in 0..4 {
        feeder.feed_next(&mut ingress).unwrap();
    }
    assert_eq!(
        ingress
            .accepted
            .iter()
            .map(|(s, b)| (*s, *b))
            .collect::<Vec<_>>(),
        vec![(0, 7), (0, 1), (1, 2), (2, 3)]
    );
    ingress.ready = true;
    feeder.feed_next(&mut ingress).unwrap();
    assert!(ingress.closed.is_empty());
    ingress.ready = false;
    feeder.feed_next(&mut ingress).unwrap();
    feeder.feed_next(&mut ingress).unwrap();
    assert_eq!(
        ingress.closed,
        vec![
            (RemoteEndpointId(0), CordId(0)),
            (RemoteEndpointId(1), CordId(1))
        ]
    );
}
#[test]
fn missing_duplicate_value_and_inconsistent_fanout_refuse_before_ingress() {
    let value = port("profile", PortTemporal::Value, 0);
    let flow = port("sample", PortTemporal::Flow { closes: true }, 1);
    assert!(SequentialForeFeeder::prepare(&[&value, &flow], &[input("sample", 1)]).is_err());
    assert!(SequentialForeFeeder::prepare(
        &[&value, &flow],
        &[input("profile", 1), input("profile", 2)]
    )
    .is_err());
    let wrong = port("profile", PortTemporal::Flow { closes: true }, 2);
    assert!(SequentialForeFeeder::prepare(&[&value, &wrong], &[input("profile", 1)]).is_err());
}
#[test]
fn retiring_pressured_mixed_feeder_does_not_submit_or_close_owed_input() {
    let value = port("profile", PortTemporal::Value, 0);
    let inputs = [input("profile", 7)];
    let mut feeder = SequentialForeFeeder::prepare(&[&value], &inputs).unwrap();
    let mut ingress = Ingress {
        full: true,
        ..Default::default()
    };
    feeder.feed_next(&mut ingress).unwrap();
    drop(feeder); // The owning execution cancels/retire; Drop performs no ingress.
    assert!(ingress.accepted.is_empty());
    assert!(ingress.closed.is_empty());
}
#[test]
fn initial_receipt_charges_actual_nested_spare_capacity_and_refuses_started_or_stream() {
    let value = port("profile", PortTemporal::Value, 0);
    let inputs = [input("profile", 7)];
    let mut feeder = SequentialForeFeeder::prepare(&[&value], &inputs).unwrap();
    feeder.flows.reserve(5);
    feeder.input_flows.reserve(7);
    feeder.flows[0].targets.reserve(9);
    feeder.flows[0].front.0.reserve(23);
    let exact = feeder.flows.capacity() * core::mem::size_of::<SequenceFlow>()
        + feeder.input_flows.capacity() * core::mem::size_of::<usize>()
        + feeder
            .flows
            .iter()
            .map(|f| {
                f.front.0.capacity()
                    + f.targets.capacity() * core::mem::size_of::<(RemoteEndpointId, CordId)>()
            })
            .sum::<usize>();
    assert_eq!(feeder.prepared_backing_bytes(), Some(exact));
    let mut ingress = Ingress::default();
    feeder.feed_next(&mut ingress).unwrap();
    assert!(feeder.prepared_backing_bytes().is_none());
    let flow = port("sample", PortTemporal::Flow { closes: true }, 1);
    let inputs = [input("sample", 7)];
    let stream = SequentialForeFeeder::prepare_stream(
        &[&flow],
        &inputs,
        crate::ExternalForeStreamConfig { admission_batch: 1 },
    )
    .unwrap();
    assert!(stream.prepared_backing_bytes().is_none());
}
