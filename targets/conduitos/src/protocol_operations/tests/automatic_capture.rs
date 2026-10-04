//! Literal device transcript fixture, never linked into native production paths.
use super::*;
use alloc::{sync::Arc, vec::Vec};
use core::sync::atomic::{AtomicUsize, Ordering};

const HUMIDITY: [u8; 7] = [0x6a, 1, 0, 0x13, 0x2b, 3, 30];
const SAMPLE: [u8; 8] = [0x65, 0x5a, 0xc0, 0x7e, 0xed, 0, 0x75, 0x30];
struct Exchange {
    write: Vec<u8>,
    response: Vec<u8>,
}
struct Transcript {
    exchanges: Vec<Exchange>,
    progress: Arc<AtomicUsize>,
}
impl crate::i2c_base::I2cProvider for Transcript {
    fn transact(
        &mut self,
        transaction: &crate::i2c_base::I2cTransaction<'_>,
        input: &mut [u8],
    ) -> Result<usize, crate::i2c_base::I2cDisposition> {
        let index = self.progress.fetch_add(1, Ordering::SeqCst);
        let exchange = self
            .exchanges
            .get(index)
            .expect("no extra device transactions");
        assert_eq!(transaction.address(), 0x76);
        assert_eq!(transaction.write(), exchange.write);
        assert_eq!(input.len(), exchange.response.len());
        input.copy_from_slice(&exchange.response);
        Ok(input.len())
    }
    fn revoke(&mut self) {}
}
struct ScriptedTime {
    now: u64,
    pending: bool,
    waits: Arc<AtomicUsize>,
}
impl crate::monotonic_clock::owner::MonotonicDeadlineProvider for ScriptedTime {
    fn poll_until(
        &mut self,
        deadline: u64,
    ) -> Result<Option<u64>, crate::monotonic_clock::codec::ClockDisposition> {
        match deadline {
            0 => Ok(Some(self.now)),
            19 | 40 => {
                if !self.pending {
                    self.pending = true;
                    self.waits.fetch_add(1, Ordering::SeqCst);
                    self.now = if deadline == 19 { 18 } else { 39 };
                    Ok(None)
                } else {
                    self.pending = false;
                    self.now = if deadline == 19 { 20 } else { 41 };
                    Ok(Some(self.now))
                }
            }
            _ => panic!("unexpected Source wait deadline {deadline}"),
        }
    }
    fn revoke(&mut self) {}
}
fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
}
fn bytes(value: &StructuredInfoValue) -> Vec<u8> {
    match value.shape() {
        StructuredInfoValueShape::Leaf(bytes) => bytes.to_vec(),
        StructuredInfoValueShape::Collection(values) => values.iter().flat_map(bytes).collect(),
        _ => panic!("byte value"),
    }
}
#[test]
fn complete_source_protocol_captures_calibration_and_sample_through_native_bus_and_waits() {
    let mut calibration = Vec::new();
    for coefficient in [
        27504_i32, 26435, -1000, 36477, -10685, 3024, 2855, 140, -7, 15500, -14600, 6000,
    ] {
        calibration.extend_from_slice(&(coefficient as u16).to_le_bytes());
    }
    let mut exchanges = Vec::new();
    let mut exchange = |write: &[u8], response: &[u8]| {
        exchanges.push(Exchange {
            write: write.to_vec(),
            response: response.to_vec(),
        })
    };
    exchange(&[0xd0], &[0x60]);
    exchange(&[0xe0, 0xb6], &[]);
    exchange(&[0xf3], &[0]);
    for write in [[0xf4, 0], [0xf2, 1], [0xf5, 0]] {
        exchange(&write, &[]);
    }
    for (index, value) in calibration.iter().enumerate() {
        exchange(&[0x88 + index as u8], &[*value]);
    }
    exchange(&[0xa1], &[75]);
    for (index, value) in HUMIDITY.iter().enumerate() {
        exchange(&[0xe1 + index as u8], &[*value]);
    }
    exchange(&[0xf4, 0x25], &[]);
    exchange(&[0xf3], &[0]);
    for (index, value) in SAMPLE.iter().enumerate() {
        exchange(&[0xf7 + index as u8], &[*value]);
    }
    let count = exchanges.len();
    let progress = Arc::new(AtomicUsize::new(0));
    let waits = Arc::new(AtomicUsize::new(0));
    let (mut play, begin, _) = super::automatic_events::prepare(
        Transcript {
            exchanges,
            progress: progress.clone(),
        },
        ScriptedTime {
            now: 17,
            pending: false,
            waits: waits.clone(),
        },
    );
    let StructuredInfoTypeShape::Record { fields, .. } = begin.shape() else {
        panic!("begin")
    };
    let input = StructuredInfoValue::record(
        begin.clone(),
        vec![
            StructuredFieldValue::new(
                "address",
                StructuredInfoValue::leaf(fields[0].value_type().clone(), vec![0x76]).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let input = ValuePayload {
        value_kind: begin.profile().unwrap().value_kind().clone(),
        encoded: input.canonical_bytes().unwrap(),
    };
    // The output buffer is allocated before execution; its exact kind comes from the sealed Fore.
    let captured = play
        .kernel()
        .definition()
        .external_capability
        .outputs
        .iter()
        .find(|port| port.port_id == port_id("captured"))
        .unwrap();
    let mut output = ValuePayload {
        value_kind: captured.value_kind.clone(),
        encoded: Vec::with_capacity(4096),
    };
    let observation_port = play
        .kernel()
        .definition()
        .external_capability
        .outputs
        .iter()
        .find(|port| port.port_id == port_id("observation"))
        .unwrap();
    let mut observation = ValuePayload {
        value_kind: observation_port.value_kind.clone(),
        encoded: Vec::with_capacity(4096),
    };
    play.start().unwrap();
    play.admit_input(&port_id("begin"), 0, &input).unwrap();
    play.close_input(&port_id("begin")).unwrap();
    let mut compensated = false;
    let mut observed = false;
    for _ in 0..20000 {
        play.step().unwrap();
        if let Some(sequence) = play.output_into(&port_id("captured"), &mut output).unwrap() {
            let state = StructuredInfoValue::from_canonical_bytes(&output.encoded).unwrap();
            assert_eq!(bytes(field(&state, "phase")), &[14]);
            assert_eq!(bytes(field(&state, "first")), calibration);
            assert_eq!(bytes(field(&state, "humidity_first")), &[75]);
            assert_eq!(bytes(field(&state, "humidity")), HUMIDITY);
            assert_eq!(bytes(field(&state, "sample")), SAMPLE);
            assert_eq!(bytes(field(&state, "clock")), 41_u64.to_le_bytes());
            play.complete_output(&port_id("captured"), sequence)
                .unwrap();
            observed = true;
        }
        if let Some(sequence) = play
            .output_into(&port_id("observation"), &mut observation)
            .unwrap()
        {
            let value = StructuredInfoValue::from_canonical_bytes(&observation.encoded).unwrap();
            let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
                panic!("observation result")
            };
            assert_eq!(tag, "observation");
            for (name, expected) in [
                ("temperature_centidegrees", 2508_i128),
                ("pressure_q24_8_pa", 25767233),
                ("humidity_q22_10_percent", 55588),
            ] {
                assert_eq!(
                    i128::from_le_bytes(bytes(field(payload, name)).try_into().unwrap()),
                    expected,
                    "{name}"
                );
            }
            play.complete_output(&port_id("observation"), sequence)
                .unwrap();
            compensated = true;
        }
        if observed && compensated {
            break;
        }
    }
    assert!(
        compensated,
        "the native Source graph must emit the exact semantic observation"
    );
    assert!(observed, "complete protocol must capture one sample");
    assert_eq!(progress.load(Ordering::SeqCst), count);
    assert_eq!(waits.load(Ordering::SeqCst), 2);
    let mut complete = false;
    for _ in 0..2000 {
        if play.step().unwrap() == conduit_composite::KernelCompositeStatus::Complete {
            complete = true;
            break;
        }
    }
    assert!(
        complete,
        "terminal Source state must drain and complete normally"
    );
}
