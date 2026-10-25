//! CPU proofs of the actual owner-control and evidence-custody components.
extern crate alloc;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_display_evidence.rs"]
#[allow(dead_code)]
mod native_display_evidence;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_owner_control.rs"]
#[allow(dead_code)]
mod native_owner_control;
#[path = "support/owner_evidence.rs"]
mod proofs;
