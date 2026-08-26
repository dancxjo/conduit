//! Pete-only architecture specimens.
//!
//! These modules retain deterministic capstone evidence for repository tests.
//! They are not part of the ordinary Pete application API and are compiled
//! only while testing this package. Firmware that shares the fixed kernel
//! specimen includes its two explicitly proof-owned source modules directly.

mod capstone;
mod capstone_kernel;
mod capstone_operations;
mod capstone_play;
