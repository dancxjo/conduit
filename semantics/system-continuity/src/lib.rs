#![no_std]

extern crate alloc;

#[allow(dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));
}

pub use generated::{LineLossDisposition, RebootDenial, RebootPendingState, RebootProgressError};

mod model;
mod reboot;
mod record;
mod transition;

pub use model::*;
pub use reboot::*;
pub use transition::*;
