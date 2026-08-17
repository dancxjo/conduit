//! Portable, bounded HTTP protocol-domain semantics.
//!
//! These contracts describe intentional HTTP work. They do not advertise an
//! implementation and contain no Conduit Line, Base, transport, platform, or
//! authority grant. A selected Host implementation must supply and admit those
//! realization facts separately.

mod codec;
mod contracts;
mod model;
mod schema;
mod state;

pub use codec::{decode_request, decode_response, encode_request, encode_response};
pub use contracts::*;
pub use model::*;
pub use schema::{http_request_type, http_response_type};
pub use state::*;
