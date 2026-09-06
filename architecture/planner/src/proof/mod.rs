//! Executable acceptance proofs built from the reusable planner API.
//!
//! These modules compose ordinary planning mechanisms to establish named,
//! bounded claims. They are not planner extension points and production code
//! must not depend on them.

pub mod heterogeneous;
pub mod voyager;

pub mod resource_frame;
