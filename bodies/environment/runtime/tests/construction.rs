#![cfg(feature = "construction")]
use conduit_environment_runtime::{construction::compile, publication::PublicationChannel};
#[test]
fn authored_appliance_uses_real_typed_publication_kinds_and_a_finite_clock() {
    for (count, period) in [(0, 60000), (4, 3000), (960, 60000), (4096, 15000)] {
        let source = compile(count, period).unwrap();
        assert_eq!((source.count, source.period_ms), (count, period));
        assert_eq!(source.expanded.gears.len(), 15);
        assert_eq!(source.expanded.connections.len(), 14);
        assert!(source
            .expanded
            .gears
            .iter()
            .all(|g| !g.kind_id.as_str().contains("fixture")));
        for channel in PublicationChannel::ALL {
            let gear = source
                .expanded
                .gears
                .iter()
                .find(|g| g.kind_id.as_str() == channel.kind_id())
                .unwrap();
            assert_eq!(gear.inputs.len(), 1);
            assert!(gear.outputs.is_empty());
            assert_eq!(
                gear.inputs[0].temporal,
                conduit_core::PortTemporal::Flow { closes: true }
            );
        }
    }
    assert!(compile(4097, 60000).is_err());
}
