//! Browser/WASM sink half of the S4 toggle-demo distributed proof.
//!
//! Split by stable responsibility:
//! - `operation`: `ToggleShowOperation` kernel state machine.
//! - `sink`: `ToggleDistributedSink` session and kernel orchestration.
//! - `abi`: thread-local state and `#[no_mangle]` WASM ABI exports.

mod abi;
mod operation;
mod sink;

// Error/status codes shared between sink and abi.
pub(self) const OUTPUT_NONE: i32 = 0;
pub(self) const OUTPUT_SESSION: i32 = 1;
pub(self) const OUTPUT_PRESENT: i32 = 2;
pub(self) const STATUS_RUNNING: i32 = 0;
pub(self) const STATUS_COMPLETE: i32 = 1;
pub(self) const ERROR_NOT_STARTED: i32 = -201;
pub(self) const ERROR_PREPARE: i32 = -202;
pub(self) const ERROR_SESSION: i32 = -203;
pub(self) const ERROR_KERNEL: i32 = -204;
pub(self) const ERROR_PRESENTATION: i32 = -205;
pub(self) const ERROR_CANCELLED: i32 = -206;
pub(self) const ERROR_EVIDENCE: i32 = -207;
pub(self) const ERROR_CAPACITY: i32 = -208;
