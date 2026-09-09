use conduit_core::{ConfigurationEntry, ConfigurationValue};
use conduit_time::*;
fn entries(period: u64) -> Vec<ConfigurationEntry> {
    vec![ConfigurationEntry {
        key: "period-ms".into(),
        value: ConfigurationValue::U64(period),
    }]
}
#[test]
fn exact_configuration_refuses_unknown_duplicate_missing_and_out_of_bounds_fields() {
    for period in [159, 961, 65536] {
        assert_eq!(
            PulseObservationConfiguration::parse(&entries(period)),
            Err(PulseObservationRefusal::Configuration)
        );
    }
    for invalid in [vec![], vec![entries(240)[0].clone(); 2]] {
        assert_eq!(
            PulseObservationConfiguration::parse(&invalid),
            Err(PulseObservationRefusal::Configuration)
        );
    }
    let mut unknown = entries(240);
    unknown[0].key = "host-clock".into();
    assert_eq!(
        PulseObservationConfiguration::parse(&unknown),
        Err(PulseObservationRefusal::Configuration)
    );
    for period in [160, 960] {
        assert!(PulseObservationConfiguration::parse(&entries(period)).is_ok());
    }
}
#[test]
fn nominal_period_and_order_are_exact_without_sampling_an_ambient_clock() {
    let configuration = PulseObservationConfiguration::parse(&entries(320)).unwrap();
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
        Ok(PulseObservation {
            sequence: 2,
            period_ms: 320
        })
    );
}
