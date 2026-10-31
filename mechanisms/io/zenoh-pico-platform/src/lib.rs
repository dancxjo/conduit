#![no_std]
//! Finite socket handoff for a blocking Zenoh-Pico worker and an asynchronous
//! network owner. This is platform backing, not a Conduit Line or admission.
//!
//! The actual network owner retains its socket and executor-local references.
//! Callers synchronize access to this value; no socket or borrowed buffer crosses
//! cores. All payload storage is supplied at construction. Accepted bytes mean
//! local buffer custody, never transport delivery or durable recording.
mod bridge;
mod ring;
pub use bridge::*;
