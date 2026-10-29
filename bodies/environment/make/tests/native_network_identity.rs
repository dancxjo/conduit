//! Actual native identity construction under allocation refusal. No provider,
//! radio effects, Root-wide memory admission or target execution is simulated.
extern crate alloc;
#[allow(dead_code)]
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_network_identity.rs"]
mod native_network_identity;
