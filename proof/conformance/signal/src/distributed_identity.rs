//! Exact identity specialization for the accepted distributed Signal profile.

use conduit_core::{BootId, HostAdvertisement, HostId, LineOffer};

/// Rebinds the ordinary std source offer to one exact host process identity.
pub fn distributed_source_advertisement_for(host_id: HostId, boot_id: BootId) -> HostAdvertisement {
    let mut advertisement = crate::distributed_std_source_advertisement();
    advertisement.host_id = host_id;
    advertisement.boot_id = boot_id;
    advertisement
}

/// Rebinds the ordinary browser sink offer to one exact launched page identity.
pub fn distributed_browser_advertisement_for(
    host_id: HostId,
    boot_id: BootId,
) -> HostAdvertisement {
    let mut advertisement = crate::distributed_browser_sink_advertisement();
    advertisement.host_id = host_id;
    advertisement.boot_id = boot_id;
    advertisement
}

/// Rebinds only the source endpoint of the accepted std/browser link. The
/// base instance, authority, peer endpoint, and finite limits stay exact.
pub fn distributed_websocket_line_offer_for(
    source_host_id: HostId,
    source_boot_id: BootId,
) -> LineOffer {
    let mut line = crate::distributed_websocket_line_offer();
    line.binding.source.host_id = source_host_id;
    line.binding.source.boot_id = source_boot_id;
    line
}

pub fn distributed_websocket_line_offer_for_endpoints(
    source_host_id: HostId,
    source_boot_id: BootId,
    sink_host_id: HostId,
    sink_boot_id: BootId,
) -> LineOffer {
    let mut line = distributed_websocket_line_offer_for(source_host_id, source_boot_id);
    line.binding.sink.host_id = sink_host_id;
    line.binding.sink.boot_id = sink_boot_id;
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specialization_changes_only_exact_source_identity() {
        let baseline = crate::distributed_websocket_line_offer();
        let specialized = distributed_websocket_line_offer_for(
            HostId::from("patchbay/native"),
            BootId::from("patchbay/boot"),
        );
        assert_eq!(
            specialized.binding.source.host_id.as_str(),
            "patchbay/native"
        );
        assert_eq!(specialized.binding.source.boot_id.as_str(), "patchbay/boot");
        assert_eq!(specialized.binding.sink, baseline.binding.sink);
        assert_eq!(specialized.binding.base, baseline.binding.base);
        assert_eq!(
            specialized.binding.base_instance_id,
            baseline.binding.base_instance_id
        );
        assert_eq!(specialized.binding.limits, baseline.binding.limits);
        assert_eq!(specialized.line_id, baseline.line_id);
        assert_eq!(specialized.contract, baseline.contract);
    }
}
