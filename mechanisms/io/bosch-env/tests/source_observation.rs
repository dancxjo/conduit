//! Differential proof against checked authored stages, not physical/admission proof.
use conduit_bosch_env::bme280_observation::{
    compensate, Calibration, Error, Observation, Sample, Unavailable,
};
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "source_observation/fixture.rs"]
mod fixture;
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
fn compare(
    source: &mut fixture::Prepared,
    c: Calibration,
    s: Sample,
) -> Result<Observation, Error> {
    let expected = source.evaluate(c, s);
    let (actual, cost) = allocation_probe::observe(|| compensate(c, s));
    assert_eq!(actual, expected, "calibration={c:?}, sample={s:?}");
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    actual
}
fn coefficient(mut c: Calibration, index: usize, value: i128) -> Calibration {
    let fields = [
        &mut c.t1, &mut c.t2, &mut c.t3, &mut c.p1, &mut c.p2, &mut c.p3, &mut c.p4, &mut c.p5,
        &mut c.p6, &mut c.p7, &mut c.p8, &mut c.p9, &mut c.h1, &mut c.h2, &mut c.h3, &mut c.h4,
        &mut c.h5, &mut c.h6,
    ];
    *fields.into_iter().nth(index).unwrap() = value;
    c
}
#[test]
fn native_candidate_matches_original_checked_source_coordinates_and_refusals() {
    // Parse/check/prepare once for the entire bounded case set.
    let mut source = fixture::Prepared::new();
    assert_eq!(
        compare(&mut source, calibration(), sample()),
        Ok(Observation {
            temperature_centidegrees: 2508,
            pressure_q24_8_pa: 25767233,
            humidity_q22_10_percent: 55588,
        })
    );
    // Source intentionally accepts coefficients outside the driver's separate
    // factory-block and physical-range policies. No policy is silently imported.
    let mut zero_humidity = calibration();
    zero_humidity.h1 = 0;
    zero_humidity.h2 = 0;
    zero_humidity.h3 = 0;
    zero_humidity.h4 = 0;
    zero_humidity.h5 = 0;
    zero_humidity.h6 = 0;
    assert_eq!(
        compare(&mut source, zero_humidity, sample())
            .unwrap()
            .humidity_q22_10_percent,
        0
    );
    for index in 0..18 {
        let (minimum, maximum) = match index {
            0 | 3 => (0, 65535),
            12 | 14 => (0, 255),
            15 | 16 => (-2048, 2047),
            17 => (-128, 127),
            _ => (-32768, 32767),
        };
        for value in [minimum, maximum] {
            let _ = compare(
                &mut source,
                coefficient(calibration(), index, value),
                sample(),
            );
        }
        for value in [minimum - 1, maximum + 1, i128::MIN, i128::MAX] {
            // Malformed dominates a disabled sample AND zero pressure divisor.
            let mut c = calibration();
            c.p1 = 0;
            c = coefficient(c, index, value);
            let s = Sample {
                pressure: 524288,
                ..sample()
            };
            assert_eq!(
                compare(&mut source, c, s),
                Err(Error::Unavailable(Unavailable::Malformed))
            );
        }
    }
    for s in [
        Sample {
            pressure: 524288,
            ..sample()
        },
        Sample {
            temperature: 524288,
            ..sample()
        },
        Sample {
            humidity: 32768,
            ..sample()
        },
    ] {
        let c = Calibration {
            p1: 0,
            ..calibration()
        };
        assert_eq!(
            compare(&mut source, c, s),
            Err(Error::Unavailable(Unavailable::SampleDisabled))
        );
    }
    assert_eq!(
        compare(
            &mut source,
            Calibration {
                p1: 0,
                ..calibration()
            },
            sample()
        ),
        Err(Error::Unavailable(Unavailable::InvalidCalibration))
    );
    for s in [
        Sample {
            pressure: -1,
            ..sample()
        },
        Sample {
            pressure: 1048576,
            ..sample()
        },
        Sample {
            temperature: -1,
            ..sample()
        },
        Sample {
            temperature: 1048576,
            ..sample()
        },
        Sample {
            humidity: -1,
            ..sample()
        },
        Sample {
            humidity: 65536,
            ..sample()
        },
    ] {
        assert_eq!(
            compare(&mut source, calibration(), s),
            Err(Error::Unavailable(Unavailable::Malformed))
        );
    }
    let mut seed = 0x15b8_7a34_u64;
    for _ in 0..64 {
        let mut next = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            seed >> 32
        };
        let c = Calibration {
            t2: 26435 + i128::from(next() % 1001) - 500,
            p2: -10685 + i128::from(next() % 1001) - 500,
            h2: 362 + i128::from(next() % 21) - 10,
            h4: if next() & 1 == 0 { -7 } else { 315 },
            h5: if next() & 1 == 0 { -50 } else { 50 },
            h6: if next() & 1 == 0 { -30 } else { 30 },
            ..calibration()
        };
        let s = Sample {
            pressure: i128::from(next() % 1048576),
            temperature: i128::from(next() % 1048576),
            humidity: i128::from(next() % 65536),
        };
        let _ = compare(&mut source, c, s);
    }
    // In-range coefficients can still overflow a checked pressure-correction
    // product. This must stay an arithmetic failure, not sensor-unavailable.
    let c = Calibration {
        t1: 0,
        t2: -32768,
        t3: -32768,
        p1: 1,
        p2: -32768,
        p3: -9878,
        p4: -32768,
        p5: -32768,
        p6: -32768,
        p9: 32767,
        ..calibration()
    };
    let s = Sample {
        pressure: 0,
        temperature: 1048575,
        ..sample()
    };
    assert_eq!(compare(&mut source, c, s), Err(Error::Arithmetic));
}
