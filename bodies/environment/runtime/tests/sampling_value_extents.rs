#![cfg(feature = "construction")]
use conduit_environment_runtime::{construction::compile, observation::ObservationContract};

#[test]
fn all_authored_payload_ports_have_independent_type_ceilings() {
    let contract = ObservationContract::prepare().unwrap();
    let reading =
        conduit_plot::maximum_prepared_canonical_value_bytes(contract.result_type()).unwrap();
    for (count, period) in [(0, 0), (1, 3000), (960, 60000), (4096, u64::MAX)] {
        let source = compile(count, period).unwrap();
        let bound = source.maximum_value_bytes().unwrap();
        eprintln!("sampling maximum canonical payload: {bound}; observation result: {reading}");
        assert!(bound >= reading);
        assert!(bound < 4096);
    }
}

#[test]
fn changed_payload_and_abnormal_ports_do_not_reuse_the_source_bound() {
    for abnormal in [false, true] {
        let mut source = compile(1, 3000).unwrap();
        let port = source
            .expanded
            .gears
            .iter_mut()
            .flat_map(|gear| gear.outputs.iter_mut())
            .next()
            .unwrap();
        if abnormal {
            port.abnormal_kind = Some("test/unbounded-abnormal".into());
        } else {
            port.value_kind = "test/unbounded-payload".into();
        }
        assert!(source.maximum_value_bytes().is_err());
    }
}
