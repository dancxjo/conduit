//! Browser/WASM sink half of the distributed toggle proof.
//!
//! Split by stable responsibility:
//! - `operation`: `ToggleShowOperation` kernel state machine.
//! - `plan`: exact two-Host plan reconstruction.
//! - `sink`: `ToggleDistributedSink` session and kernel orchestration.
//! - `abi`: thread-local state and `#[no_mangle]` WASM ABI exports.

mod abi;
mod operation;
mod plan;
mod sink;

// Error/status codes shared between sink and abi.
const OUTPUT_NONE: i32 = 0;
const OUTPUT_SESSION: i32 = 1;
const OUTPUT_PRESENT: i32 = 2;
const STATUS_RUNNING: i32 = 0;
const STATUS_COMPLETE: i32 = 1;
const ERROR_NOT_STARTED: i32 = -201;
const ERROR_PREPARE: i32 = -202;
const ERROR_SESSION: i32 = -203;
const ERROR_KERNEL: i32 = -204;
const ERROR_PRESENTATION: i32 = -205;
const ERROR_CANCELLED: i32 = -206;
const ERROR_SIGN: i32 = -207;
const ERROR_CAPACITY: i32 = -208;
