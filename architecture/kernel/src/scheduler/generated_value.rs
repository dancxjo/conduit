//! A staged generated output retains its physical representation until commit.
#[derive(Clone, Copy)]
pub(super) enum PreparedStepValue {
    Bytes(u32),
    #[cfg(feature = "alloc")]
    NativeU64(u64),
}
impl PreparedStepValue {
    pub(super) fn logical_bytes(self) -> u32 {
        match self {
            Self::Bytes(n) => n,
            #[cfg(feature = "alloc")]
            Self::NativeU64(_) => 8,
        }
    }
}
