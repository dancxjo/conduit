//! Production bounded-kernel proof, not Body admission or physical sensor proof.
#![cfg(feature = "kernel-step")]
use conduit_bosch_env::{
    bme280_observation::{Calibration, Error, Sample, Unavailable},
    typed::{CodecError, ObservationContract, PreparedObservationCodec},
};
use conduit_kernel::{
    scheduler::{SchedulerError, SchedulerStatus, StepBack},
    CordId, KernelEventKind, PortId, ValueStorage,
};
use std::cell::Cell;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "typed_observation/pipeline.rs"]
mod pipeline;
#[path = "source_observation/fixture.rs"]
mod source;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn calibration() -> Calibration {
    Calibration {
        t1: 27504,
        t2: 26435,
        t3: -1000,
        p1: 36477,
        p2: -10685,
        p3: 3024,
        p4: 2855,
        p5: 140,
        p6: -7,
        p7: 15500,
        p8: -14600,
        p9: 6000,
        h1: 75,
        h2: 362,
        h3: 0,
        h4: 315,
        h5: 50,
        h6: 30,
    }
}
fn sample() -> Sample {
    Sample {
        pressure: 415148,
        temperature: 519888,
        humidity: 30000,
    }
}
fn fill_both_queues(play: &mut pipeline::Play<'_>) {
    for _ in 0..24 {
        play.step().unwrap();
        if play.cord_usage(CordId(0)).unwrap().0 == 1 && play.cord_usage(CordId(1)).unwrap().0 == 1
        {
            return;
        }
    }
    panic!("bounded pipeline did not reach output pressure")
}
#[test]
fn typed_back_preserves_source_meaning_under_real_kernel_pressure_and_retirement() {
    let contract = ObservationContract::prepare().unwrap();
    let mut source = source::Prepared::new();
    let input = source.input(calibration(), sample());
    source.evaluate(calibration(), sample()).unwrap();
    let expected = source.canonical_result().to_vec();
    let mut codec = PreparedObservationCodec::prepare(&contract).unwrap();
    let backing = codec.retained_backing_bytes();
    let ((), cost) = allocation_probe::observe(|| {
        assert_eq!(codec.convert(&input).unwrap(), expected);
        assert_eq!(codec.output(), expected);
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert_eq!(codec.retained_backing_bytes(), backing);
    let gate = Cell::new(false);
    let mut play = pipeline::prepare(
        &contract,
        vec![input.clone(), input.clone()],
        vec![expected.clone(), expected.clone()],
        &gate,
    );
    let ((), cost) = allocation_probe::observe(|| {
        fill_both_queues(&mut play);
        assert_eq!(play.drivers()[1].commits(), 1);
        assert_eq!(play.drivers()[2].seen(), 0);
        for _ in 0..8 {
            play.step().unwrap();
            assert_eq!(play.cord_usage(CordId(0)).unwrap().0, 1);
            assert_eq!(play.cord_usage(CordId(1)).unwrap().0, 1);
            assert_eq!(play.drivers()[1].commits(), 1);
            assert!(play.drivers()[1].prepared_output(PortId(0)).is_none());
        }
        gate.set(true);
        play.run(48).unwrap();
        assert_eq!(play.drivers()[1].commits(), 2);
        assert_eq!(play.drivers()[2].seen(), 2);
        assert_eq!(play.step().unwrap(), SchedulerStatus::Drained);
        assert_eq!(play.values().used_items(), 0);
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    gate.set(false);
    let mut play = pipeline::prepare(
        &contract,
        vec![input.clone(), input.clone()],
        vec![expected.clone(), expected.clone()],
        &gate,
    );
    let ((), cost) = allocation_probe::observe(|| {
        fill_both_queues(&mut play);
        play.cancel().unwrap();
        assert_eq!(play.step().unwrap(), SchedulerStatus::Cancelled);
        assert_eq!(play.values().used_items(), 0);
        assert_eq!(play.values().used_bytes(), 0);
        assert_eq!(play.cord_usage(CordId(0)).unwrap(), (0, 0));
        assert_eq!(play.cord_usage(CordId(1)).unwrap(), (0, 0));
        assert!(play.drivers()[1].prepared_output(PortId(0)).is_none());
        gate.set(true);
        assert_eq!(play.step().unwrap(), SchedulerStatus::Cancelled);
        assert_eq!(play.drivers()[1].commits(), 1);
        assert_eq!(play.drivers()[2].seen(), 0);
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    let disabled = Sample {
        pressure: 524288,
        ..sample()
    };
    let c = Calibration {
        p1: 0,
        ..calibration()
    };
    let disabled_input = source.input(c, disabled);
    assert_eq!(
        source.evaluate(c, disabled),
        Err(Error::Unavailable(Unavailable::SampleDisabled))
    );
    let disabled_result = source.canonical_result().to_vec();
    let mut play = pipeline::prepare(
        &contract,
        vec![disabled_input],
        vec![disabled_result],
        &gate,
    );
    let ((), cost) = allocation_probe::observe(|| {
        play.run(32).unwrap();
        assert_eq!(play.drivers()[1].commits(), 1);
        assert_eq!(play.drivers()[2].seen(), 1);
        assert_eq!(play.values().used_items(), 0);
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    // Valid canonical bytes with the wrong schema must not pass as an input.
    for invalid in [vec![0], expected] {
        assert!(matches!(
            codec.convert(&invalid),
            Err(CodecError::Canonical(_))
        ));
        assert!(codec.output().is_empty());
        let mut play = pipeline::prepare(&contract, vec![invalid], vec![], &gate);
        let ((), cost) = allocation_probe::observe(|| {
            let error = (0..12)
                .find_map(|_| play.step().err())
                .expect("invalid input refused");
            assert!(matches!(error, SchedulerError::BackFailed(_)));
            assert_eq!(play.cord_usage(CordId(0)).unwrap().0, 1);
            assert_eq!(play.cord_usage(CordId(1)).unwrap(), (0, 0));
            assert_eq!(play.drivers()[1].commits(), 0);
            assert_eq!(play.drivers()[2].seen(), 0);
            assert!(play
                .signs()
                .events()
                .any(|event| event.kind == KernelEventKind::BackFailed));
            assert!(play.drivers()[1].prepared_output(PortId(0)).is_none());
            play.cancel().unwrap();
            assert_eq!(play.values().used_items(), 0);
        });
        assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    }
}
