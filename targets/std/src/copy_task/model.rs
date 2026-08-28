use conduit_core::{ActivePlayId, PlanId, ResourceHandleId};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CopyRequestId(String);

impl CopyRequestId {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if value.is_empty() || value.len() > 128 {
            return Err("copy request identity must contain 1..=128 bytes".to_string());
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyResult {
    Success {
        bytes_copied: u64,
    },
    DestinationExists,
    Denied,
    StaleHandle,
    Oversized {
        source_bytes: u64,
        maximum_bytes: u64,
    },
    Partial {
        bytes_copied: u64,
    },
    Cancelled {
        bytes_copied: u64,
    },
    CleanupFailed {
        bytes_copied: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyRunReceipt {
    pub request_id: CopyRequestId,
    pub run_id: ActivePlayId,
    pub plan_id: PlanId,
    pub source_binding_id: ResourceHandleId,
    pub destination_binding_id: ResourceHandleId,
    pub result: CopyResult,
    pub kernel_events: usize,
    pub presented_result: Option<conduit_core::StructuredInfoValue>,
}

#[derive(Debug, Clone, Default)]
pub struct CopyStopToken {
    requested: Arc<AtomicBool>,
}

impl CopyStopToken {
    pub fn request_stop(&self) {
        self.requested.store(true, Ordering::Release);
    }

    pub(crate) fn is_requested(&self) -> bool {
        self.requested.load(Ordering::Acquire)
    }
}
