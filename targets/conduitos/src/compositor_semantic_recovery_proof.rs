//! Separate semantic recovery appliance using the normal native graphical Make.
//! Its entry feature changes the executable; it is not the uninstrumented image.
use crate::{arch, boot, make};
#[path = "ordinary_domain_proof/compositor_semantic_recovery.rs"]
mod fixture;

pub fn run(record: &boot::BootRecord) -> ! {
    arch::initialize_machine(record, boot::executable_physical_address);
    let admitted = &make::EMBEDDED_MAKE;
    if record.runtime_arena.length != admitted.runtime_arena_ceiling {
        refuse("semantic-recovery-runtime-admission");
    }
    arch::early_write(alloc::format!(
        "CONDUIT_COMPOSITOR_SEMANTIC_RECOVERY_PROFILE {{\"schema\":\"conduit.conduitos/compositor-semantic-recovery-profile@1\",\"proof_feature\":\"compositor-semantic-recovery-proof\",\"profile_id\":{:?},\"build_id\":{:?},\"image_binding\":{:?},\"runtime_arena_bytes\":{},\"runtime_arena_ceiling\":{}}}\n",
        admitted.profile_id, admitted.build_id, admitted.image_binding,
        record.runtime_arena.length, admitted.runtime_arena_ceiling,
    ).as_bytes());
    fixture::run();
    arch::deterministic_exit(true)
}
fn refuse(reason: &str) -> ! {
    arch::early_write(b"CONDUIT_COMPOSITOR_SEMANTIC_RECOVERY_REFUSAL ");
    arch::early_write(reason.as_bytes());
    arch::early_write(b"\n");
    arch::deterministic_exit(false)
}
