//! Exercise the actual Pico adapter; timed output must not reconstruct a Face.
#![allow(unexpected_cfgs)]
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
mod config {
    pub fn display_profile() -> conduit_ht16k33::Profile {
        conduit_ht16k33::Profile::adafruit_alpha4()
    }
}
#[path = "../../../../targets/rp2040/proof/environment-usb/src/environment_show.rs"]
mod environment_show;
use conduit_bosch_env::{Humidity, Reading};
use conduit_environment_face::EnvironmentalReading;

#[test]
fn prepared_pico_pages_preserve_values_expiry_and_allocate_nothing_per_frame() {
    let (show, preparation) = allocation_probe::observe(|| {
        environment_show::prepare(
            Reading {
                temperature_centi_celsius: 2222,
                pressure_centi_pascal: 10_132_500,
                humidity: Humidity::PercentQ10(45 * 1024),
            },
            7,
        )
        .unwrap()
    });
    assert!(preparation.allocations > 0);
    println!(
        "Pico sample preparation: {} allocations, {} peak bytes; retained Show: {} bytes",
        preparation.allocations,
        preparation.peak_bytes,
        core::mem::size_of_val(&show)
    );
    let expected = [
        [0x07, 0x5b, 0x63, 0x71], // 72°F
        [0x76, 0, 0x66, 0x6d],    // H 45
        [0x06, 0x3f, 0x06, 0x4f], // 1013 hPa
    ];
    let (_, allocations) = allocation_probe::observe(|| {
        let _refuse = allocation_probe::refuse_from_nth_allocation(0);
        for elapsed in 0..30_000 {
            assert_eq!(
                show.render(elapsed),
                expected[((elapsed / 2000) % 3) as usize]
            );
            let frame = environment_show::frame(&show, elapsed).unwrap();
            assert_ne!(frame, [0; 16]);
        }
        assert_eq!(show.render(30_000), [0x40; 4]);
        assert_eq!(show.render(u64::MAX), [0x40; 4]);
    });
    assert_eq!(allocations.allocations, 0);
    assert_eq!(allocations.reallocations, 0);
    assert_eq!(allocations.deallocations, 0);
    let dht = environment_show::prepare_common(
        EnvironmentalReading::temperature_humidity(
            conduit_core::Quantity::parse_plot_literal("22.22°C").unwrap(),
            conduit_core::Quantity::parse_plot_literal("45%").unwrap(),
        )
        .unwrap(),
        8,
    )
    .unwrap();
    assert_eq!(dht.render(0), expected[0]);
    assert_eq!(dht.render(2000), expected[1]);
    assert_eq!(dht.render(4000), expected[0]);
    assert_eq!(show.render(4000), expected[2]); // preparing a new revision does not alter the old pages
}

#[test]
fn bmp_pages_skip_absent_humidity_and_invalid_readings_refuse_preparation() {
    let reading = Reading {
        temperature_centi_celsius: 2222,
        pressure_centi_pascal: 10_132_500,
        humidity: Humidity::Unavailable(conduit_bosch_env::HumidityUnavailable::UnsupportedChip),
    };
    let show = environment_show::prepare(reading, 9).unwrap();
    assert_eq!(show.render(0), [0x07, 0x5b, 0x63, 0x71]);
    assert_eq!(show.render(2000), [0x06, 0x3f, 0x06, 0x4f]);
    assert_eq!(show.render(4000), show.render(0));
    assert!(environment_show::prepare(
        Reading {
            pressure_centi_pascal: 0,
            ..reading
        },
        10
    )
    .is_err());
    assert!(environment_show::prepare(
        Reading {
            humidity: Humidity::PercentQ10(101 * 1024),
            ..reading
        },
        10
    )
    .is_err());
}
