//! Current Crèche widget projection and event adapter for the Birth form.
//! Semantic draft and selection types are owned by `conduit_birth_form`.
//! These extensions retain the live browser/native protocol during extraction.

mod actions;
mod presentation;

pub use actions::{BirthActionOutcome, BirthActions};
pub use presentation::BirthPresentation;
