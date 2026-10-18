//! Binary32 entrance; never rounds through binary64 or host float arithmetic.
use super::{ExactRational, Work, R};
use crate::{Binary64Rounding, IeeeF32};
impl ExactRational {
    /// Exact denotation of finite IEEE bits; either signed zero denotes zero.
    pub fn from_binary32(value: IeeeF32, work: &mut Work) -> Result<Self, R> {
        Self::from_ieee(u64::from(value.bits()), super::ieee::BINARY32, work)
    }
    pub(in crate::exact_numeric) fn round_binary32(
        self,
        rounding: Binary64Rounding,
        work: &mut Work,
    ) -> Result<(IeeeF32, bool), R> {
        let (bits, inexact) = self.round_ieee(super::ieee::BINARY32, rounding, work)?;
        Ok((IeeeF32::from_bits(bits as u32), inexact))
    }
}
