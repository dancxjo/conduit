//! Clock failure is an exact Source refusal and a normally closing lifecycle.
use super::*;
use crate::monotonic_clock::codec::ClockDisposition;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};

struct ProbeBus(Arc<AtomicUsize>);
impl crate::i2c_base::I2cProvider for ProbeBus {
    fn transact(
        &mut self,
        request: &crate::i2c_base::I2cTransaction<'_>,
        input: &mut [u8],
    ) -> Result<usize, crate::i2c_base::I2cDisposition> {
        assert_eq!(request.write(), &[0xd0]);
        assert_eq!(
            self.0.fetch_add(1, Ordering::SeqCst),
            0,
            "no transaction after clock failure"
        );
        input[0] = 0x60;
        Ok(1)
    }
    fn revoke(&mut self) {}
}
struct FailedClock(ClockDisposition, Arc<AtomicUsize>);
impl crate::monotonic_clock::owner::MonotonicDeadlineProvider for FailedClock {
    fn poll_until(&mut self, deadline: u64) -> Result<Option<u64>, ClockDisposition> {
        assert_eq!(deadline, 0);
        assert_eq!(
            self.1.fetch_add(1, Ordering::SeqCst),
            0,
            "no retry after clock failure"
        );
        Err(self.0)
    }
    fn revoke(&mut self) {}
}
fn case<'a>(ty: &'a StructuredInfoType, tag: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!("variant")
    };
    cases
        .iter()
        .find(|case| case.tag() == tag)
        .unwrap()
        .payload_type()
}

#[test]
fn unavailable_and_lost_clock_publish_exact_refusals_and_close_without_retry() {
    for (disposition, tag) in [
        (ClockDisposition::Unavailable, "unavailable"),
        (ClockDisposition::ProviderLost, "provider-lost"),
    ] {
        let bus_calls = Arc::new(AtomicUsize::new(0));
        let clock_calls = Arc::new(AtomicUsize::new(0));
        let (mut play, begin, failure) = super::automatic_events::prepare(
            ProbeBus(bus_calls.clone()),
            FailedClock(disposition, clock_calls.clone()),
        );
        let StructuredInfoTypeShape::Record { fields, .. } = begin.shape() else {
            panic!("begin")
        };
        let input = ValuePayload {
            value_kind: begin.profile().unwrap().value_kind().clone(),
            encoded: StructuredInfoValue::record(
                begin.clone(),
                vec![
                    StructuredFieldValue::new(
                        "address",
                        StructuredInfoValue::leaf(fields[0].value_type().clone(), vec![0x76])
                            .unwrap(),
                    )
                    .unwrap(),
                ],
            )
            .unwrap()
            .canonical_bytes()
            .unwrap(),
        };
        let clock_type = case(&failure, "clock");
        let clock_value = StructuredInfoValue::variant(
            clock_type.clone(),
            tag,
            StructuredInfoValue::leaf(case(clock_type, tag).clone(), vec![]).unwrap(),
        )
        .unwrap();
        let clock_bytes = clock_value.canonical_bytes().unwrap();
        let failure_bytes = StructuredInfoValue::variant(failure.clone(), "clock", clock_value)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        let mut refused = ValuePayload {
            value_kind: failure.profile().unwrap().value_kind().clone(),
            encoded: alloc::vec::Vec::with_capacity(4096),
        };
        let mut clock_failed = ValuePayload {
            value_kind: clock_type.profile().unwrap().value_kind().clone(),
            encoded: alloc::vec::Vec::with_capacity(512),
        };
        play.start().unwrap();
        play.admit_input(&port_id("begin"), 0, &input).unwrap();
        play.close_input(&port_id("begin")).unwrap();
        let mut refusals = 0;
        let mut failures = 0;
        let mut complete = false;
        for _ in 0..2000 {
            let status = play.step().unwrap();
            for (port, output, expected, count) in [
                ("refused", &mut refused, &failure_bytes, &mut refusals),
                (
                    "clock_failed",
                    &mut clock_failed,
                    &clock_bytes,
                    &mut failures,
                ),
            ] {
                if let Some(sequence) = play.output_into(&port_id(port), output).unwrap() {
                    assert_eq!(&output.encoded, expected);
                    *count += 1;
                    play.complete_output(&port_id(port), sequence).unwrap();
                }
            }
            if status == conduit_composite::KernelCompositeStatus::Complete {
                complete = true;
                break;
            }
        }
        assert!(complete, "clock failure must close the entire Source graph");
        assert_eq!((refusals, failures), (1, 1));
        assert_eq!(bus_calls.load(Ordering::SeqCst), 1);
        assert_eq!(clock_calls.load(Ordering::SeqCst), 1);
    }
}
