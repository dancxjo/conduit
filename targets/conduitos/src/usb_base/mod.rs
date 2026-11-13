//! Class-neutral USB transaction data. Native ownership and admitted Host Call
//! binding remain necessary for effects; constructing a request grants neither.

pub mod control_contract;
pub mod control_decode;
pub mod control_factory;
pub mod control_offer;
pub mod control_owner;
pub mod control_payload;
pub mod control_proof_plan;
pub mod control_request;
pub mod control_result;
pub(crate) mod control_ring;

pub mod device_probe_proof_plan;
pub mod hid_boot_control_proof_plan;

pub mod device_probe_proof_kernel;

pub mod configuration_probe_proof_plan;

pub mod endpoint_read_contract;
pub mod endpoint_read_request;
pub mod endpoint_read_result;

pub mod endpoint_read_factory;
pub mod endpoint_read_offer;
pub mod endpoint_read_owner;

pub(crate) mod endpoint_ring;

pub mod endpoint_read_proof_plan;

pub mod hid_endpoint_proof_plan;
pub mod hid_source_kernel;
pub mod hid_source_plan;

pub mod hid_endpoint_proof_kernel;

pub mod installation;

pub mod endpoint_write_contract;
pub mod endpoint_write_factory;
pub mod endpoint_write_offer;
pub mod endpoint_write_owner;
pub mod endpoint_write_request;
pub mod endpoint_write_result;

pub mod isochronous_schedule;

pub mod isochronous;

#[cfg(any(
    test,
    any(
        all(target_arch = "arm", feature = "armv6-product"),
        all(target_arch = "aarch64", feature = "aarch64-rpi-3b-plus")
    )
))]
pub(crate) mod dwc2_control;

#[cfg(any(
    test,
    any(
        all(target_arch = "arm", feature = "armv6-product"),
        all(target_arch = "aarch64", feature = "aarch64-rpi-3b-plus")
    )
))]
pub(crate) mod dwc2_endpoint_context;
#[cfg(any(
    test,
    any(
        all(target_arch = "arm", feature = "armv6-product"),
        all(target_arch = "aarch64", feature = "aarch64-rpi-3b-plus")
    )
))]
pub(crate) mod dwc2_endpoint_read;
#[cfg(any(
    test,
    any(
        all(target_arch = "arm", feature = "armv6-product"),
        all(target_arch = "aarch64", feature = "aarch64-rpi-3b-plus")
    )
))]
pub(crate) mod dwc2_hid_play;
#[cfg(any(
    test,
    any(
        all(target_arch = "arm", feature = "armv6-product"),
        all(target_arch = "aarch64", feature = "aarch64-rpi-3b-plus")
    )
))]
pub(crate) mod dwc2_keyboard_session;

pub mod control_source_plan;

#[cfg(any(
    test,
    any(
        all(target_arch = "arm", feature = "armv6-product"),
        all(target_arch = "aarch64", feature = "aarch64-rpi-3b-plus")
    )
))]
pub(crate) mod dwc2_bus_route;

#[cfg(any(
    test,
    any(
        all(target_arch = "arm", feature = "armv6-product"),
        all(target_arch = "aarch64", feature = "aarch64-rpi-3b-plus")
    )
))]
pub(crate) mod dwc2_control_play;

#[cfg_attr(test, allow(dead_code))]
#[cfg(any(
    test,
    all(target_arch = "arm", feature = "armv6-product"),
    all(target_arch = "aarch64", feature = "aarch64-rpi-3b-plus")
))]
pub(crate) mod dwc2_hub_play;

#[cfg(any(
    test,
    any(
        all(target_arch = "arm", feature = "armv6-product"),
        all(target_arch = "aarch64", feature = "aarch64-rpi-3b-plus")
    )
))]
pub(crate) mod dwc2_pointer_play;

#[cfg(any(
    test,
    any(
        all(target_arch = "arm", feature = "armv6-product"),
        all(target_arch = "aarch64", feature = "aarch64-rpi-3b-plus")
    )
))]
pub(crate) mod dwc2_pointer_session;
