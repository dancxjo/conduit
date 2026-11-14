//! Shared enumeration meaning; native Root retains topology and possession.
use super::{ProtocolSourcePackage, ProtocolSourceRefusal};

pub fn usb_enumeration_package() -> Result<ProtocolSourcePackage, ProtocolSourceRefusal> {
    ProtocolSourcePackage::compile(
        [
            include_str!("../../plots/usb/control.conduit"),
            include_str!("../../plots/usb/enumeration-requests.conduit"),
            include_str!("../../plots/usb/enumeration-descriptors.conduit"),
            include_str!("../../plots/usb/hub-port-power.conduit"),
            include_str!("../../plots/usb/hub-port-status.conduit"),
            include_str!("../../plots/usb/hub-port-reset.conduit"),
        ]
        .join("\n"),
        &[],
    )
}
