//! One explicit checkpoint/runtime profile, never an arbitrary ONNX fallback.
use serde::{Deserialize, Serialize};

pub const PROFILE: &str = "std/linux-x86_64-glibc/ort-1.23.2-cpu-f32-identity@1";
pub const MODEL_SHA256: [u8; 32] = [
    0x72, 0x78, 0x55, 0x40, 0x32, 0x3f, 0xee, 0xfa, 0x4d, 0xe4, 0x20, 0xbe, 0x19, 0x8a, 0x40, 0xb7,
    0x16, 0x32, 0x73, 0xa9, 0x09, 0x96, 0x2e, 0xd8, 0x9b, 0x0a, 0x58, 0xe7, 0x81, 0x33, 0xbe, 0x7b,
];
pub const RUNTIME_SHA256: [u8; 32] = [
    156, 143, 97, 18, 233, 220, 240, 158, 212, 230, 33, 161, 208, 167, 182, 186, 73, 87, 92, 125,
    9, 242, 122, 101, 45, 36, 162, 44, 133, 100, 177, 60,
];
pub const MAX_CALLS: u16 = 1024;
pub const STACK_BYTES: usize = 512 * 1024;
pub const HEAP_SEED_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeCpuProfile {
    /// Host configuration, never authored Plot content. No path discovery/download.
    pub runtime_library: String,
    pub runtime_sha256: [u8; 32],
    pub maximum_calls: u16,
}
impl NativeCpuProfile {
    pub fn validate(&self) -> Result<(), NativeBackingRefusal> {
        if !self.runtime_library.starts_with('/')
            || self.runtime_library.len() > 1024
            || self.runtime_library.as_bytes().contains(&0)
            || self.runtime_sha256 != RUNTIME_SHA256
            || self.maximum_calls == 0
            || self.maximum_calls > MAX_CALLS
        {
            return Err(NativeBackingRefusal::UnsupportedProfile);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeBackingGrant {
    pub maximum_private_bytes: u64,
    pub maximum_file_backing_bytes: u64,
}
impl NativeBackingGrant {
    pub fn validate(self) -> Result<(), NativeBackingRefusal> {
        if self.maximum_private_bytes == 0
            || self.maximum_private_bytes > 256 * 1024 * 1024
            || self.maximum_file_backing_bytes == 0
            || self.maximum_file_backing_bytes > 512 * 1024 * 1024
        {
            return Err(NativeBackingRefusal::UnsupportedProfile);
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NativeBackingRefusal {
    UnsupportedProfile,
    IdentityMismatch,
    InsufficientBacking {
        private_required: u64,
        file_required: u64,
    },
    InvalidLedger,
    BackingPreparation,
    ExecutionBoundary,
    ProviderLost,
    InvalidInput,
    InvalidOutput,
    Protocol,
    Capacity,
    Io,
}
