#![no_std]

extern crate alloc;

#[allow(dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));
}

pub use generated::LinguisticOffsetBasis;

mod catalog;
mod info;
mod reference;

pub use catalog::*;
pub use info::*;
pub use reference::*;
