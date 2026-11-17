//! Exact linked Source representation and kernel Back for pure compensation.
//! These definitions supply no Kind offer, equipment, Plan or admission grant.
extern crate alloc;
mod back;
mod codec;
pub use back::ObservationBack;
pub use codec::{CodecError, PreparedObservationCodec};
use conduit_core::{BorrowedStructuredValueValidator, StructuredInfoRefusal};
include!(concat!(env!("OUT_DIR"), "/contract.rs"));
pub const INPUT_TYPE_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/input.type"));
pub const RESULT_TYPE_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/result.type"));

/// Prepared identity of the immutable build-checked input/result representation.
/// No owned schema tree, parser, capability, or provider is retained.
pub struct ObservationContract {
    _checked: (),
}
impl ObservationContract {
    pub fn prepare() -> Result<Self, StructuredInfoRefusal> {
        BorrowedStructuredValueValidator::new(INPUT_TYPE_BYTES, INPUT_MAXIMUM_BYTES)?;
        BorrowedStructuredValueValidator::new(RESULT_TYPE_BYTES, RESULT_MAXIMUM_BYTES)?;
        Ok(Self { _checked: () })
    }
}
