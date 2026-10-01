#![no_std]

//! The Birth form's bounded draft and friendly-name derivation.
//! Selections cross the owning body lifecycle boundary; this crate neither
//! creates a body nor owns a renderer, admission, or execution. Its Face
//! projection and bounded action protocol preserve the exact Birth encounter
//! shared by browser and native consumers.

extern crate alloc;

mod actions;
mod draft;
pub mod names;
mod presentation;

pub use actions::{BirthActionOutcome, BirthActions};
pub use draft::{BirthDraft, BirthDraftRefusal, BirthFormChoice, BirthSelection};
pub use presentation::BirthPresentation;
