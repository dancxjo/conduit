//! Exact identity specialization for the accepted distributed Signal profile.

use conduit_core::{BootId, HostAdvertisement, HostId, LinkBinding};

/// Rebinds the ordinary std source offer to one exact host process identity.
pub fn distributed_source_advertisement_for(host_id: HostId, boot_id: BootId) -> HostAdvertisement {
    let mut advertisement = crate::distributed_std_source_advertisement();
    advertisement.host_id = host_id;
    advertisement.boot_id = boot_id;
    advertisement
}

/// Rebinds only the source endpoint of the accepted std/browser link. The
/// provider instance, authority, peer endpoint, and finite limits stay exact.
pub fn distributed_websocket_link_binding_for(
    source_host_id: HostId,
    source_boot_id: BootId,
) -> LinkBinding {
    let mut binding = crate::distributed_websocket_link_binding();
    binding.source.host_id = source_host_id;
    binding.source.boot_id = source_boot_id;
    binding
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specialization_changes_only_exact_source_identity() {
        let baseline = crate::distributed_websocket_link_binding();
        let specialized = distributed_websocket_link_binding_for(
            HostId::from("patchbay/native"),
            BootId::from("patchbay/boot"),
        );
        assert_eq!(specialized.source.host_id.as_str(), "patchbay/native");
        assert_eq!(specialized.source.boot_id.as_str(), "patchbay/boot");
        assert_eq!(specialized.sink, baseline.sink);
        assert_eq!(specialized.provider, baseline.provider);
        assert_eq!(
            specialized.provider_instance_id,
            baseline.provider_instance_id
        );
        assert_eq!(specialized.limits, baseline.limits);
    }
}
