//! Source command synthesis with complete recurring input/output storage prepared.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_environment_runtime::display_packets::{DisplayPacketRefusal, PreparedDisplayPackets};
#[test]
fn source_packets_preserve_registers_and_refuse_invalid_brightness_without_allocating() {
    let mut encoder = PreparedDisplayPackets::prepare().unwrap();
    let ((), measured) = allocation_probe::observe(|| {
        let batch = encoder
            .encode([0x7d, 0xff, 0x66, 0x71, 0, 0], 3, true)
            .unwrap();
        assert_eq!(
            batch.packets(),
            [
                &[0x40][..],
                &[0xc0, 0x7d, 0xff, 0x66, 0x71, 0, 0][..],
                &[0x8b][..]
            ]
        );
        assert_eq!(
            encoder.encode([0; 6], 8, true),
            Err(DisplayPacketRefusal::InvalidBrightness)
        );
        let disabled = encoder.encode([0; 6], 7, false).unwrap();
        assert_eq!(disabled.packets()[2], &[0x87]);
        assert_eq!(
            encoder.encode([0; 6], 255, false),
            Err(DisplayPacketRefusal::InvalidBrightness)
        );
    });
    assert_eq!((measured.allocations, measured.reallocations), (0, 0));
}
