//! Named measurement profiles keep physical choices and retained schemas exact.
use crate::plan::NumericProfile;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Profile {
    Standard,
    CompactLocal,
    MatrixLocal,
}
impl Profile {
    pub(crate) fn from_name(name: &str) -> Result<Self, &'static str> {
        match name {
            "prepared" => Ok(Self::Standard),
            "prepared-compact-local" => Ok(Self::CompactLocal),
            "prepared-matrix-local" => Ok(Self::MatrixLocal),
            _ => Err("unknown prepared owner profile"),
        }
    }
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Standard => "prepared",
            Self::CompactLocal => "prepared-compact-local",
            Self::MatrixLocal => "prepared-matrix-local",
        }
    }
    pub(super) fn local(self) -> bool {
        self != Self::Standard
    }
    pub(super) fn numeric(self) -> Option<NumericProfile> {
        match self {
            Self::Standard => None,
            Self::CompactLocal => Some(NumericProfile::CompactDagLocalExpressions),
            Self::MatrixLocal => Some(NumericProfile::CompactMatrixDagLocalExpressions),
        }
    }
    pub(super) fn native_values(self) -> u64 {
        match self {
            Self::Standard => 0,
            Self::CompactLocal => 140,
            Self::MatrixLocal => 236,
        }
    }
    pub(super) fn schema(self) -> &'static str {
        match self {
            Self::Standard => "conduit.fargan-prepared-performance@1",
            Self::CompactLocal => "conduit.fargan-compact-local-performance@1",
            Self::MatrixLocal => "conduit.fargan-matrix-local-performance@1",
        }
    }
}
