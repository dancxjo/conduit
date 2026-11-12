//! Startup events are Body semantics. They contain no Host or playback policy.
use crate::{KindTerminalBehavior, StandardKindContract};
#[cfg(feature = "plot-catalog")]
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
pub const BODY_WAKE_NAME: &str = "Body wake";
pub const BODY_FIRST_WAKE_NAME: &str = "First body wake";
pub const BODY_WAKE_SUMMARY: &str = "Emit true once in the first admitted play of each body Wake.";
pub const BODY_FIRST_WAKE_SUMMARY: &str =
    "Emit true once in the first admitted play of this body's first wake.";
pub const BODY_WAKE_EXAMPLE: &str = "wake: body/wake";
pub const BODY_FIRST_WAKE_EXAMPLE: &str = "wake: body/first-wake";
pub const STARTUP_CHIME_NAME: &str = "Conduit startup chime";
pub const STARTUP_CHIME_SUMMARY: &str = "Announce a true event with the original short Conduit cue; retain inaudible outcomes without failing unrelated work.";
pub const STARTUP_CHIME_EXAMPLE: &str = "chime: sound/startup-chime";
pub const STARTUP_PULSE_PORT: &str = "pulse";
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
            BODY_FIRST_WAKE_NAME
        } else {
            BODY_WAKE_NAME
        }
        .into(),
        summary: if first_only {
            BODY_FIRST_WAKE_SUMMARY
        } else {
            BODY_WAKE_SUMMARY
        }
        .into(),
        inputs: Vec::new(),
        outputs: vec![pulse_port(PortDirection::Output)],
        configuration: Default::default(),
        limits: limits(),
        terminal_behavior: KindTerminalBehavior::EmitsOnceWhenScopeIsEligible,
        hosted_implementation_required: true,
        browser_manifestation_honest: true,
        pico_manifestation_honest: false,
        example: if first_only {
            BODY_FIRST_WAKE_EXAMPLE
        } else {
            BODY_WAKE_EXAMPLE
        }
        .into(),
    }
}

/// Optional audible confirmation: denial is retained as a failed/denied Host
/// operation, but this sink consumes the event and permits unrelated work to run.
/// No graphics, input device, or particular audio implementation is required.
pub fn startup_chime_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(STARTUP_CHIME_KIND),
        plain_name: STARTUP_CHIME_NAME.into(),
        summary: STARTUP_CHIME_SUMMARY.into(),
        inputs: vec![pulse_port(PortDirection::Input)],
        outputs: Vec::new(),
        configuration: Default::default(),
        limits: limits(),
        terminal_behavior: KindTerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: true,
        pico_manifestation_honest: false,
        example: STARTUP_CHIME_EXAMPLE.into(),
    }
}

fn pulse_port(direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(STARTUP_PULSE_PORT),
        value_kind: kind_id(BOOL_INFO_ID),
        direction,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    }
}
fn limits() -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 16,
        max_queue_items: 1,
        max_queue_bytes: STARTUP_PULSE_MAXIMUM_BYTES,
    }
}

#[cfg(feature = "plot-catalog")]
pub fn install_body_startup_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    for (contract, revision) in [
        (body_wake_contract(false), BODY_WAKE_REVISION),
        (body_wake_contract(true), BODY_FIRST_WAKE_REVISION),
        (startup_chime_contract(), STARTUP_CHIME_REVISION),
    ] {
        startup.insert(conduit_plot::KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: vec![],
        })?;
        profile
            .insert_kind(contract.into_semantic_contract(revision))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
