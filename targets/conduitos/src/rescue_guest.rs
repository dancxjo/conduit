//! Freestanding presentation and execution of the admitted local rescue contract.

use alloc::format;

use crate::{arch, identity, local_rescue};

pub fn observe(
    identities: &identity::BootIdentities,
    matcher: &mut local_rescue::LocalRescueMatcher,
    transition: local_rescue::ValidatedLocalTransition,
    ordinary_keyboard_plan: bool,
) {
    let policy = local_rescue::LocalRescuePolicy {
        enabled: true,
        reboot_base_available: true,
    };
    match matcher.observe(policy, transition) {
        local_rescue::RescueDecision::NoRequest => {}
        local_rescue::RescueDecision::RebootBaseUnavailable { .. } => refuse(),
        local_rescue::RescueDecision::RequestAccepted { policy, operation } => {
            let boot_id = identity::hex(&identities.boot);
            let receipt = format!(
                "CONDUIT_RESCUE_SIGN {{\"schema\":\"conduit.conduitos.local-rescue-request/v1\",\"status\":\"accepted\",\"proof_class\":\"freestanding-emulator\",\"old_boot_id\":\"{}\",\"authority\":\"local-physical-input\",\"policy\":\"{}\",\"operation\":\"{}\",\"request_id\":\"local-rescue/{}/1\",\"ordinary_keyboard_plan\":{}}}\n",
                boot_id, policy, operation, boot_id, ordinary_keyboard_plan,
            );
            arch::early_write(receipt.as_bytes());
            arch::early_write(b"CONDUIT_BOOT_STAGE local-rescue-reset-requested\n");
            match arch::local_reboot_base().request() {
                Ok(never) => match never {},
                Err(_) => refuse(),
            }
        }
    }
}

fn refuse() -> ! {
    arch::early_write(b"CONDUIT_MACHINE_SIGN {\"status\":\"refused\",\"reason\":\"local-rescue-base-unavailable\"}\n");
    arch::deterministic_exit(false)
}
