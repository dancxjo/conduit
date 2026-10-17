//! Opt-in, bounded revision law. This semantic library neither schedules work
//! nor wraps ordinary flows, records Signs, or grants effect authority.
mod event;
mod journal;

pub use event::*;
pub use journal::*;
