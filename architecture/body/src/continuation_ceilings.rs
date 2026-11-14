//! Installed finite continuation intent; no Boot approval or storage grant.
use serde::{Deserialize, Serialize};

/// Finite instantaneous ceilings, not resource grants or operation possession.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContinuationCeilings {
    pub assignment_bytes: u32,
    pub runtime_bytes: u32,
    pub durable_bytes: u32,
    pub operation_slots: u32,
    pub active_worksets: u32,
}
impl ContinuationCeilings {
    pub(crate) fn words(self) -> [u32; 5] {
        [
            self.assignment_bytes,
            self.runtime_bytes,
            self.durable_bytes,
            self.operation_slots,
            self.active_worksets,
        ]
    }
    pub fn has_finite_bounds(self) -> bool {
        self.words().into_iter().all(|n| n != 0)
    }
    #[cfg(feature = "authenticated-admission")]
    pub(crate) fn within(self, maximum: Self) -> bool {
        self.words()
            .into_iter()
            .zip(maximum.words())
            .all(|(n, max)| n <= max)
    }
}
