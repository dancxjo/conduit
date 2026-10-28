//! Public Source package path checks peripheral Fores without creating resources.
use super::*;
#[test]
fn spi_and_digital_sources_package_and_expand_without_native_possession() {
    for (source, entry) in [
        (
            include_str!("../../../../../../plots/device-protocols/spi.conduit"),
            "spi-exchange",
        ),
        (
            include_str!("../../../../../../plots/device-protocols/digital.conduit"),
            "digital-access",
        ),
    ] {
        let package = ProtocolSourcePackage::compile(source.into(), &[]).unwrap();
        let encoded = serde_json::to_vec(&package).unwrap();
        let prepared = PreparedProtocolEntry::prepare(&encoded, entry).unwrap();
        assert_eq!(prepared.expanded().expanded.gears.len(), 1);
        assert_ne!(prepared.expanded().expanded.source_document_id.as_str(), "");
    }
}
