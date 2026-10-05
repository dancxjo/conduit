//! Class-neutral USB transaction data. Native ownership and admitted Host Call
//! binding remain necessary for effects; constructing a request grants neither.

pub mod control_contract;
pub mod control_decode;
pub mod control_factory;
pub mod control_owner;
pub mod control_payload;
pub mod control_proof_plan;
pub mod control_request;
pub mod control_result;
pub(crate) mod control_ring;

pub mod device_probe_proof_plan;

pub mod device_probe_proof_kernel;

pub mod configuration_probe_proof_plan;

pub mod endpoint_read_contract;
pub mod endpoint_read_request;
pub mod endpoint_read_result;

pub mod endpoint_read_factory;
pub mod endpoint_read_owner;

pub(crate) mod endpoint_ring;

pub mod endpoint_read_proof_plan;
