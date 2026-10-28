//! Ordinary boot admission of one administratively approved protocol workload.
#[cfg(all(target_arch = "aarch64", not(feature = "aarch64-board")))]
mod absent_providers;
#[cfg(any(
    target_arch = "x86_64",
    target_arch = "arm",
    all(target_arch = "aarch64", not(feature = "aarch64-board"))
))]
mod authority;
mod request;
pub use request::{
    MAXIMUM_REQUEST_BYTES, ProtocolBootInput, ProtocolBootRequest, ProtocolBootRequestRefusal,
    ROOT_MODULE_COMMAND,
};

#[cfg(target_arch = "x86_64")]
mod owners;

#[cfg(any(
    target_arch = "x86_64",
    target_arch = "arm",
    all(target_arch = "aarch64", not(feature = "aarch64-board"))
))]
mod body;
#[cfg(target_arch = "arm")]
pub(crate) mod minipitft_owners;
#[cfg(any(
    target_arch = "x86_64",
    all(target_arch = "aarch64", not(feature = "aarch64-board"))
))]
mod runtime;
mod step_failure;

#[cfg(target_arch = "x86_64")]
mod bootstrap;
#[cfg(target_arch = "x86_64")]
pub use bootstrap::run_if_selected;

mod usb_request;
pub use usb_request::UsbProtocolBootRequest;
#[cfg(any(
    target_arch = "x86_64",
    all(
        target_arch = "aarch64",
        feature = "aarch64-product",
        not(feature = "aarch64-board")
    )
))]
mod usb_bootstrap;
#[cfg(any(
    target_arch = "x86_64",
    all(target_arch = "aarch64", not(feature = "aarch64-board"))
))]
mod usb_owners;

#[cfg(all(
    target_arch = "aarch64",
    feature = "aarch64-product",
    not(feature = "aarch64-board")
))]
pub use usb_bootstrap::run_if_selected;

#[cfg(any(
    target_arch = "x86_64",
    all(target_arch = "aarch64", not(feature = "aarch64-board"))
))]
#[path = "protocol_boot/usb_preflight.rs"]
mod usb_preflight;
