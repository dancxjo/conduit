//! Binary64 entrance to the shared integer-only, single-round converter.
use super::{ExactRational, Work, R};
use crate::{Binary64Rounding, IeeeF64};
impl ExactRational {
    /// Exact denotation of finite IEEE bits; either signed zero denotes zero.
    pub fn from_binary64(value: IeeeF64, work: &mut Work) -> Result<Self, R> {
        Self::from_ieee(value.bits(), super::ieee::BINARY64, work)
    }
    pub(in crate::exact_numeric) fn round_binary64(
        self,
        rounding: Binary64Rounding,
        work: &mut Work,
    ) -> Result<(IeeeF64, bool), R> {
        let (bits, inexact) = self.round_ieee(super::ieee::BINARY64, rounding, work)?;
        Ok((IeeeF64::from_bits(bits), inexact))
    }
}
