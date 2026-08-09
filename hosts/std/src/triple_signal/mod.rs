//! Kernel and session surfaces for the final actual three-host Signal proof.

mod clue;
mod operation;
#[cfg(unix)]
mod runner;
mod source;

pub use clue::{PicoClue, PicoRuntimeIdentity};
#[cfg(unix)]
pub use runner::{default_pico_ports, TriplePhysicalRunner};
pub use source::{RemoteKind, StdoutReceipt, TripleOffer, TripleSource};
