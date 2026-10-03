//! Class-neutral USB transaction data. Native ownership and admitted Host Call
//! binding remain necessary for effects; constructing a request grants neither.

pub mod control_contract;
pub mod control_decode;
pub mod control_payload;
pub mod control_request;
pub mod control_result;
