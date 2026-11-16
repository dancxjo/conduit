//! Checked observation Source contracts; no acquisition or runtime installation.
#![no_std]
extern crate alloc;
mod types;
pub use types::{
    install_ambient_display_types, prepare_ambient_display_types, AMBIENT_DISPLAY_TYPES_SOURCE,
};
