// Per-thread read faults affect only explicit receipt-proof minting tests.
std::thread_local! {
    static CHECKPOINT_DECODE_FAULT: std::cell::Cell<(u8,usize,usize)> = const { std::cell::Cell::new((0,0,0)) };
}
fn checkpoint_decode_fault(mode: u8) {
    CHECKPOINT_DECODE_FAULT.with(|f| f.set((mode, 0, 0)));
}

// Actual cooperative mailbox/window backing needed by included checkpoint proofs.
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_flash_jobs.rs"]
mod native_flash_jobs;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_flash_window.rs"]
mod native_flash_window;

// Actual Root/task/kernel/Face/history modules; adapters are in adapters.rs.
use conduit_core as c;
include!("adapters.rs");
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_authority.rs"]
mod native_authority;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_kernel_storage.rs"]
mod native_kernel_storage;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_kernel.rs"]
mod native_kernel;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_service.rs"]
mod native_service;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_worker.rs"]
mod native_worker;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_retirement.rs"]
mod native_retirement;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_task.rs"]
mod native_task;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_face.rs"]
// Preserve the production affine errors and pinned-toolchain byte iteration.
#[allow(clippy::result_large_err, clippy::chunks_exact_to_as_chunks)]
mod native_face;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_mask_evidence.rs"]
mod native_mask_evidence;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_software.rs"]
mod native_software;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_owner_control.rs"]
mod native_owner_control;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_owner_reservation.rs"]
mod native_owner_reservation;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_owner_receipt.rs"]
mod native_owner_receipt;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_execution_gate.rs"]
mod native_execution_gate;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_body_checkpoint.rs"]
mod native_body_checkpoint;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_body_join.rs"]
mod native_body_join;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_joined_host_wire.rs"]
mod native_joined_host_wire;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_restart.rs"]
mod native_restart;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_admission.rs"]
mod native_admission;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_display_evidence.rs"]
mod native_display_evidence;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_observed_archive.rs"]
// Existing target archive APIs preserve boxed custody and pinned byte loops.
#[allow(clippy::borrowed_box, clippy::collapsible_if, clippy::chunks_exact_to_as_chunks)]
mod native_observed_archive;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_observed_http.rs"]
mod native_observed_http;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_observed_wire.rs"]
mod native_observed_wire;
#[path = "../../../../../targets/esp32/proof/tm1637/src/appliance_layout.rs"]
mod appliance_layout;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_mask_display.rs"]
mod native_display;
#[path = "root.rs"]
mod native_root;
#[test]
#[ignore = "requires explicit production native profile cfg and generated Source fixture"]
fn actual_root_two_complete_finite_handoffs_with_checkpoint_refusal_custody() {
    native_root::prove_two_handoffs();
}
#[test]
#[ignore = "requires explicit production native profile cfg and generated Source fixture; separate process"]
fn actual_store_retries_storage_but_latches_corruption_without_publishing_pending_state() {
    native_root::prove_storage_policy();
}
#[test]
#[ignore = "requires explicit production native profile cfg and generated Source fixture; separate process"]
fn actual_store_fresh_journal_failure_keeps_initial_seed_unpublished_and_latched() {
    native_root::prove_fresh_journal_failure_latches();
}
