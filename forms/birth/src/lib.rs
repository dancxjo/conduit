#![no_std]

//! The Birth form's bounded draft and friendly-name derivation.
//! Selections cross the owning body lifecycle boundary; this crate neither
//! creates a body nor owns a renderer, event protocol, admission, or execution.

extern crate alloc;

mod draft;
pub mod names;

pub use draft::{BirthDraft, BirthDraftRefusal, BirthFormChoice, BirthSelection};
