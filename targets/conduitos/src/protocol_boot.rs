//! Ordinary boot admission of one administratively approved protocol workload.
mod request;
pub use request::{
    MAXIMUM_REQUEST_BYTES, ProtocolBootInput, ProtocolBootRequest, ProtocolBootRequestRefusal,
    ROOT_MODULE_COMMAND,
};

#[cfg(target_arch = "x86_64")]
mod owners;

#[cfg(target_arch = "x86_64")]
mod body;
#[cfg(target_arch = "x86_64")]
mod runtime;

#[cfg(target_arch = "x86_64")]
mod bootstrap;
#[cfg(target_arch = "x86_64")]
pub use bootstrap::run_if_selected;
