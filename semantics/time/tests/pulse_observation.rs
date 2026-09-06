use conduit_core::{ConfigurationEntry, ConfigurationValue};
use conduit_time::*;
fn entries(period: u64, count: u64) -> Vec<ConfigurationEntry> {
    vec![
        ConfigurationEntry {
            key: "period-ms".into(),
            value: ConfigurationValue::U64(period),
        },
        ConfigurationEntry {
            key: "maximum-pulses".into(),
            value: ConfigurationValue::U64(count),
        },
    ]
}
#[test]
fn exact_configuration_refuses_unknown_duplicate_missing_and_out_of_bounds_fields() {
    for (period, count) in [(159, 1), (961, 1), (240, 0), (240, 65), (65536, 1)] {
        assert_eq!(
            PulseObservationConfiguration::parse(&entries(period, count)),
            Err(PulseObservationRefusal::Configuration)
        );
    }
    for invalid in [
        vec![],
        entries(240, 1)[..1].to_vec(),
        vec![entries(240, 1)[0].clone(); 2],
    ] {
        assert_eq!(
            PulseObservationConfiguration::parse(&invalid),
            Err(PulseObservationRefusal::Configuration)
        );
    }
    let mut unknown = entries(240, 1);
    unknown[1].key = "host-clock".into();
    assert_eq!(
        PulseObservationConfiguration::parse(&unknown),
        Err(PulseObservationRefusal::Configuration)
    );
    for period in [160, 960] {
        assert!(PulseObservationConfiguration::parse(&entries(period, 64)).is_ok());
    }
}
#[test]
fn nominal_period_and_order_are_exact_without_sampling_an_ambient_clock() {
    let configuration = PulseObservationConfiguration::parse(&entries(320, 2)).unwrap();
    assert_eq!(
        configuration.observe(0, 0),
        Ok(PulseObservation {
            sequence: 0,
            period_ms: 320
        })
    );
    assert_eq!(
        configuration.observe(0, 1),
        Err(PulseObservationRefusal::UnexpectedSequence {
            expected: 0,
            actual: 1
        })
    );
    assert_eq!(
        configuration.observe(1, 0),
        Err(PulseObservationRefusal::UnexpectedSequence {
            expected: 1,
            actual: 0
        })
    );
    assert!(matches!(
        configuration.observe(0, u64::MAX),
        Err(PulseObservationRefusal::UnexpectedSequence { .. })
    ));
    assert_eq!(
        configuration.observe(2, 2),
        Err(PulseObservationRefusal::Exhausted)
    );
}
