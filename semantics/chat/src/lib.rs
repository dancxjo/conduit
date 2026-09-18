//! Generic, transport-neutral chat contracts and bounded conversation state.
//!
//! Speech recognition and synthesis compose around these contracts in Tongues;
//! they do not own Body prompt, history, or context semantics.
#![no_std]

extern crate alloc;

mod body_chat;
pub use body_chat::*;

mod shared_pool;
pub use shared_pool::*;
mod interactive_state;
pub use interactive_state::*;
mod interactive_catalog;
pub use interactive_catalog::*;
mod messaging;
pub use messaging::*;
mod messaging_fixture;
pub use messaging_fixture::*;
mod messaging_reference;
pub use messaging_reference::*;
mod messaging_view;
pub use messaging_view::*;
#[cfg(feature = "form-catalog")]
mod messaging_catalog;
#[cfg(feature = "form-catalog")]
pub use messaging_catalog::*;
