//! Exact retained native owners and their admitted node bindings.
use crate::{i2c_base::owner::I2cHostCall, pure_protocol_owner::PureProtocolOwner};
use alloc::{boxed::Box, vec::Vec};
use conduit_core::{AuthorityBinding, ResourceBinding};
use conduit_kernel::NodeId;

pub(super) enum Owner<P> {
    Pure(PureProtocolOwner),
    Spi(Box<crate::spi_base::owner::SpiHostCall<Box<dyn crate::spi_base::SpiProvider>>>),
    Digital(
        Box<
            crate::digital_base::owner::DigitalHostCall<
                Box<dyn crate::digital_base::DigitalProvider>,
            >,
        >,
    ),
    I2c(Box<I2cHostCall<P>>),
    #[cfg(all(target_arch = "x86_64", feature = "native-owner-network"))]
    EthernetFrame(Box<crate::ethernet_frame::owned_installation::OwnedNativeEthernetFrameHost>),
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-board"))
    ))]
    UsbControl(Box<crate::arch::NativeUsbControlCall>),
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-board"))
    ))]
    UsbProtocolControl,
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-board"))
    ))]
    UsbProtocolEndpoint,
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-board"))
    ))]
    UsbDuplexControl,
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-board"))
    ))]
    UsbDuplexRead,
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-board"))
    ))]
    UsbDuplexWrite,
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-board"))
    ))]
    StorageAuthorize,
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-board"))
    ))]
    StorageSettle,
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-board"))
    ))]
    UsbIsochronousControl,
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-board"))
    ))]
    UsbIsochronousOut,
}
#[cfg(any(
    target_arch = "x86_64",
    all(target_arch = "aarch64", not(feature = "aarch64-board"))
))]
#[derive(Clone, Copy)]
pub(super) enum PendingUsbKind {
    ProtocolRead,
    DuplexRead,
    DuplexWrite,
    IsochronousOut,
}
pub(super) struct Binding<P> {
    pub(super) node: NodeId,
    pub(super) resources: Vec<ResourceBinding>,
    pub(super) authorities: Vec<AuthorityBinding>,
    pub(super) owner: Owner<P>,
}
