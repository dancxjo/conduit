//! Source protocol execution through separate retained bus and clock owners.
use super::*;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};

struct WrongIdentityBus(Arc<AtomicUsize>);
impl crate::i2c_base::I2cProvider for WrongIdentityBus {
    fn transact(
        &mut self,
        transaction: &crate::i2c_base::I2cTransaction<'_>,
        input: &mut [u8],
    ) -> Result<usize, crate::i2c_base::I2cDisposition> {
        assert_eq!(transaction.address(), 0x76);
        assert_eq!(transaction.write(), &[0xd0]);
        assert_eq!(input.len(), 1);
        assert_eq!(
            self.0.fetch_add(1, Ordering::SeqCst),
            0,
            "no retry after identity refusal"
        );
        input[0] = 0;
        Ok(1)
    }
    fn revoke(&mut self) {}
}
struct ObservedClock(Arc<AtomicUsize>);
impl crate::monotonic_clock::owner::MonotonicDeadlineProvider for ObservedClock {
    fn poll_until(
        &mut self,
        deadline: u64,
    ) -> Result<Option<u64>, crate::monotonic_clock::codec::ClockDisposition> {
        assert_eq!(
            deadline, 0,
            "bus completion requests an immediate observed timestamp"
        );
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(Some(17))
    }
    fn revoke(&mut self) {}
}

#[test]
fn wrong_identity_traverses_actual_bus_clock_and_source_feedback_without_retry() {
    let bus_effects = Arc::new(AtomicUsize::new(0));
    let clock_effects = Arc::new(AtomicUsize::new(0));
    let (mut play, begin, failure) = super::automatic_events::prepare(
        WrongIdentityBus(bus_effects.clone()),
        ObservedClock(clock_effects.clone()),
    );
    assert_eq!(bus_effects.load(Ordering::SeqCst), 0);
    assert_eq!(clock_effects.load(Ordering::SeqCst), 0);
    let StructuredInfoTypeShape::Record { fields, .. } = begin.shape() else {
        panic!("begin record")
    };
    let value = StructuredInfoValue::record(
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
        encoded: value.canonical_bytes().unwrap(),
    };
    let StructuredInfoTypeShape::Variant { cases, .. } = failure.shape() else {
        panic!("failure variant")
    };
    let wrong = cases
        .iter()
        .find(|case| case.tag() == "wrong-identity")
        .unwrap();
    let expected = StructuredInfoValue::variant(
        failure.clone(),
        "wrong-identity",
        StructuredInfoValue::leaf(wrong.payload_type().clone(), vec![0]).unwrap(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let mut output = ValuePayload {
        value_kind: failure.profile().unwrap().value_kind().clone(),
        encoded: alloc::vec::Vec::with_capacity(4096),
    };
    play.start().unwrap();
    assert_eq!(
        play.admit_input(&port_id("begin"), 0, &input).unwrap(),
        conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { sequence: 0 }
    );
    play.close_input(&port_id("begin")).unwrap();
    let mut observed = false;
    for _ in 0..2000 {
        play.step().unwrap();
        if let Some(sequence) = play.output_into(&port_id("refused"), &mut output).unwrap() {
            assert_eq!(output.encoded, expected);
            play.complete_output(&port_id("refused"), sequence).unwrap();
            observed = true;
            break;
        }
    }
    assert!(
        observed,
        "the Source transition must publish the exact identity refusal"
    );
    for _ in 0..100 {
        play.step().unwrap();
    }
    assert_eq!(bus_effects.load(Ordering::SeqCst), 1);
    assert_eq!(clock_effects.load(Ordering::SeqCst), 1);
    // The Source terminal state must close the whole admitted graph normally.
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
