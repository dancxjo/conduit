use super::*;
use crate::ExternalForeStreamConfig as Config;

fn port() -> LoweredForePort {
    LoweredForePort {
        front_port_id: "input".into(),
        direction: PortDirection::Input,
        track: ConnectionTrack::Payload,
        endpoint: RemoteEndpointId(0),
        cord: CordId(0),
        value_kind: "test/byte".into(),
        quantity_profile: conduit_core::QuantityValueProfile::Decimal,
        value_contract: Some(
            conduit_core::CheckedValueContract::new("test/byte".into(), 1, vec![]).unwrap(),
        ),
        abnormal_kind: None,
        temporal: PortTemporal::Flow { closes: true },
        item_capacity: 2,
        byte_capacity: 2,
        selected_line: None,
    }
}
fn inputs(count: usize) -> Vec<ExternalForeInput> {
    (0..count)
        .map(|i| ExternalForeInput {
            front_port_id: "input".into(),
            track: ConnectionTrack::Payload,
            bytes: vec![i as u8],
        })
        .collect()
}
struct Queue {
    busy: bool,
    occupied: usize,
    accepted: Vec<(u64, u8, usize)>,
    closed: usize,
}
impl ForeIngress for Queue {
    fn has_ready_work(&self) -> bool {
        self.busy
    }
    fn admit(
        &mut self,
        targets: &[(RemoteEndpointId, CordId)],
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, String> {
        if self.occupied == 2 {
            return Ok(RemoteIngressOutcome::Full { sequence });
        }
        self.occupied += 1;
        self.accepted.push((sequence, bytes[0], targets.len()));
        Ok(RemoteIngressOutcome::Accepted { sequence })
    }
    fn close(&mut self, _: RemoteEndpointId, _: CordId) -> Result<(), String> {
        assert!(!self.busy);
        self.closed += 1;
        Ok(())
    }
}
#[test]
fn overlapping_pressure_retries_exact_input_and_drains_before_close_without_growth() {
    let a = port();
    let mut b = port();
    b.endpoint = RemoteEndpointId(1);
    b.cord = CordId(1);
    let inputs = inputs(1024);
    for batch in [1, 4, 64] {
        let mut feeder = SequentialForeFeeder::prepare_stream(
            &[&a, &b],
            &inputs,
            Config {
                admission_batch: batch,
            },
        )
        .unwrap();
        let mut queue = Queue {
            busy: true,
            occupied: 0,
            accepted: Vec::with_capacity(1024),
            closed: 0,
        };
        let probe = crate::allocation_probe::begin();
        for turn in 0..10_000 {
            feeder.feed_next(&mut queue).unwrap();
            if turn % 3 == 0 {
                queue.occupied = queue.occupied.saturating_sub(1);
            }
            if feeder.stream.unwrap().accepted == 1024 {
                break;
            }
        }
        assert_eq!(queue.closed, 0);
        queue.busy = false;
        feeder.feed_next(&mut queue).unwrap();
        feeder.feed_next(&mut queue).unwrap();
        assert_eq!(probe.finish(), 0);
        assert_eq!(queue.closed, 2);
        assert_eq!(queue.accepted.len(), 1024);
        for (i, item) in queue.accepted.iter().enumerate() {
            assert_eq!(*item, (i as u64, i as u8, 2));
        }
        let stats = feeder.stream.unwrap();
        assert_eq!(stats.attempts, u32::from(stats.accepted) + stats.full);
        assert!(stats.full > 0);
        assert!(!feeder.is_pending());
    }
}
#[test]
fn stream_admission_checks_finite_lifetime_batch_and_every_payload_before_play() {
    let port = port();
    let values = inputs(4096);
    let config = Config {
        admission_batch: 64,
    };
    assert!(SequentialForeFeeder::prepare_stream(&[&port], &values, config).is_ok());
    assert!(SequentialForeFeeder::prepare(&[&port], &values).is_err());
    for count in [0, 4097] {
        assert!(SequentialForeFeeder::prepare_stream(&[&port], &inputs(count), config).is_err());
    }
    for admission_batch in [0, 65, u16::MAX] {
        assert!(SequentialForeFeeder::prepare_stream(
            &[&port],
            &values,
            Config { admission_batch }
        )
        .is_err());
    }
    for alteration in 0..3 {
        let mut changed = values.clone();
        match alteration {
            0 => changed[4095].bytes.push(0),
            1 => changed[4095].front_port_id = "other".into(),
            _ => changed[4095].track = ConnectionTrack::NormalClose,
        }
        assert!(SequentialForeFeeder::prepare_stream(&[&port], &changed, config).is_err());
    }
}
#[test]
fn causal_sequence_still_waits_for_ready_work_to_drain() {
    let port = port();
    let inputs = inputs(2);
    let mut feeder = SequentialForeFeeder::prepare(&[&port], &inputs).unwrap();
    let mut queue = Queue {
        busy: true,
        occupied: 0,
        accepted: Vec::with_capacity(2),
        closed: 0,
    };
    feeder.feed_next(&mut queue).unwrap();
    assert!(queue.accepted.is_empty());
    queue.busy = false;
    feeder.feed_next(&mut queue).unwrap();
    assert_eq!(queue.accepted.len(), 1);
    assert!(feeder.stream.is_none());
}
