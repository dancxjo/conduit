//! Library-owned driver representation and finite-loop capabilities. The sealed
//! numerical profile binds the original model Types; full Source executions and
//! fresh Native admission establish policy and ancestry, never these constants.
use crate::{
    parser_session_numeric_profile::{FixedParserNumericProfile, PinnedFourSlotNumericProfile},
    LanguageParserLegalMask,
};
use conduit_plot::rust_binding::PreparedNativeRustBinding;

pub(crate) trait FixedParserDriverProfile: FixedParserNumericProfile {
    type Mask: PreparedNativeRustBinding;
    const MAXIMUM_TOKENS: usize;
    const MAXIMUM_ROUNDS: usize;
}
impl FixedParserDriverProfile for PinnedFourSlotNumericProfile {
    type Mask = LanguageParserLegalMask;
    const MAXIMUM_TOKENS: usize = 4;
    const MAXIMUM_ROUNDS: usize = 12;
}
