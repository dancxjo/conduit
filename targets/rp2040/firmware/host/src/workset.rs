//! Exact checked Source closure emitted by the ordinary fabrication input.
//! These immutable definitions describe implementations; they confer no Ready
//! possession, original resource reservation or permission to start a Play.
#[allow(dead_code)]
pub mod generated {
    include!(concat!(env!("OUT_DIR"), "/workset.rs"));
}
#[allow(dead_code)]
pub mod renderer {
    include!(concat!(env!("OUT_DIR"), "/renderer.rs"));
}

#[allow(dead_code)]
pub mod native_images {
    include!(concat!(env!("OUT_DIR"), "/native_images.rs"));
}

pub use conduit_mcu_host_runtime::native_values::prepare as values;
