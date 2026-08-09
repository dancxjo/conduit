//! Kernel and session surfaces for the final actual three-host Signal proof.

mod operation;
#[cfg(unix)]
mod runner;
mod sign;
mod source;

#[cfg(unix)]
pub use runner::{default_pico_ports, TriplePhysicalRunner};
pub use sign::{PicoRuntimeIdentity, PicoSign};
pub use source::{RemoteKind, StdoutReceipt, TripleOffer, TripleSource};
