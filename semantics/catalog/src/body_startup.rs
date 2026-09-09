//! Startup events are Body semantics. They contain no Host or playback policy.
use crate::{StandardKindContract, TerminalBehavior};
#[cfg(feature = "form-catalog")]
use alloc::string::{String, ToString};
use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, PortDescriptor, PortDirection, PortTemporal, BOOL_INFO_ID,
};

pub const BODY_WAKE_KIND: &str = "body/wake";
pub const BODY_FIRST_WAKE_KIND: &str = "body/first-wake";
pub const BODY_WAKE_REVISION: &str = "conduit.body/wake@1";
pub const BODY_FIRST_WAKE_REVISION: &str = "conduit.body/first-wake@1";
pub const STARTUP_CHIME_KIND: &str = "sound/startup-chime";
pub const STARTUP_CHIME_REVISION: &str = "conduit.sound/startup-chime@1";
pub const STARTUP_PULSE_MAXIMUM_BYTES: u32 = conduit_core::BOOL_ENCODED_LEN as u32;

pub fn body_wake_contract(first_only: bool) -> StandardKindContract {
    let kind = if first_only {
        BODY_FIRST_WAKE_KIND
    } else {
        BODY_WAKE_KIND
    };
    StandardKindContract {
        kind_id: kind_id(kind),
        plain_name: if first_only {
            "First Body wake"
        } else {
            "Body wake"
        }
        .into(),
        summary: if first_only {
            "Emit true once in the first admitted Play of this Body's first Wake."
        } else {
            "Emit true once in the first admitted Play of each Body Wake."
        }
        .into(),
        inputs: Vec::new(),
        outputs: vec![pulse_port(PortDirection::Output)],
        configuration: Vec::new(),
        limits: limits(),
        terminal_behavior: TerminalBehavior::EmitsOnceWhenScopeIsEligible,
        hosted_implementation_required: true,
        browser_manifestation_honest: true,
        pico_manifestation_honest: false,
        example: alloc::format!("wake: {kind}"),
    }
}

/// Optional audible confirmation: denial is retained as a failed/denied Host
/// operation, but this sink consumes the event and permits unrelated work to run.
/// No graphics, input device, or particular audio implementation is required.
pub fn startup_chime_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(STARTUP_CHIME_KIND), plain_name: "Conduit startup chime".into(),
        summary: "Announce a true event with the original short Conduit cue; retain inaudible outcomes without failing unrelated work.".into(),
        inputs: vec![pulse_port(PortDirection::Input)], outputs: Vec::new(),
        configuration: Vec::new(), limits: limits(),
        terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: true, pico_manifestation_honest: false,
        example: "chime: sound/startup-chime".into(),
    }
}

fn pulse_port(direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id("pulse"),
        value_kind: kind_id(BOOL_INFO_ID),
        direction,
        temporal: PortTemporal::Flow { closes: true },
    }
}
fn limits() -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 16,
        max_queue_items: 1,
        max_queue_bytes: STARTUP_PULSE_MAXIMUM_BYTES,
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_body_startup_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (contract, revision) in [
        (body_wake_contract(false), BODY_WAKE_REVISION),
        (body_wake_contract(true), BODY_FIRST_WAKE_REVISION),
        (startup_chime_contract(), STARTUP_CHIME_REVISION),
    ] {
        startup.insert(conduit_form::KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: vec![],
        })?;
        profile
            .insert(conduit_form::KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: revision.into(),
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: vec![],
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
