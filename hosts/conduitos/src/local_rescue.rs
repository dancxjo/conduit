//! Finite local physical-input rescue policy below ordinary semantic routing.

pub const LOCAL_RESCUE_POLICY: &str = "conduitos/local-physical-rescue@1";
pub const LOCAL_REBOOT_OPERATION: &str = "conduitos.machine/reboot@1";
pub const DELETE_USAGE: u8 = 0x4c;
const LEFT_CONTROL: u8 = 1 << 0;
const LEFT_ALT: u8 = 1 << 2;
const RIGHT_CONTROL: u8 = 1 << 4;
const RIGHT_ALT: u8 = 1 << 6;

/// A transition whose provenance is the validated local HID path.
///
/// Construction remains inside ConduitOS so portable values received from a
/// Cord or Line cannot be passed to the local rescue policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedLocalTransition {
    usage: u8,
    pressed: bool,
    modifiers: u8,
}

impl ValidatedLocalTransition {
    /// Wrap fields only after the local adapter has validated the physical HID
    /// report. Portable semantic adapters must not invoke this constructor.
    #[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
    pub(crate) const fn from_validated_hid(usage: u8, pressed: bool, modifiers: u8) -> Self {
        Self {
            usage,
            pressed,
            modifiers,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalRescuePolicy {
    pub enabled: bool,
    pub reboot_base_available: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RescueDecision {
    NoRequest,
    RequestAccepted {
        policy: &'static str,
        operation: &'static str,
    },
    RebootBaseUnavailable {
        policy: &'static str,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LocalRescueMatcher {
    delete_held: bool,
}

impl LocalRescueMatcher {
    pub const fn new() -> Self {
        Self { delete_held: false }
    }

    pub fn observe(
        &mut self,
        policy: LocalRescuePolicy,
        local: ValidatedLocalTransition,
    ) -> RescueDecision {
        let transition = local;
        if transition.usage == DELETE_USAGE && !transition.pressed {
            self.delete_held = false;
            return RescueDecision::NoRequest;
        }
        if !policy.enabled
            || transition.usage != DELETE_USAGE
            || !transition.pressed
            || self.delete_held
            || !control_held(transition.modifiers)
            || !alt_held(transition.modifiers)
        {
            return RescueDecision::NoRequest;
        }
        self.delete_held = true;
        if policy.reboot_base_available {
            RescueDecision::RequestAccepted {
                policy: LOCAL_RESCUE_POLICY,
                operation: LOCAL_REBOOT_OPERATION,
            }
        } else {
            RescueDecision::RebootBaseUnavailable {
                policy: LOCAL_RESCUE_POLICY,
            }
        }
    }
}

const fn control_held(modifiers: u8) -> bool {
    modifiers & (LEFT_CONTROL | RIGHT_CONTROL) != 0
}

const fn alt_held(modifiers: u8) -> bool {
    modifiers & (LEFT_ALT | RIGHT_ALT) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    const INTERACTIVE: LocalRescuePolicy = LocalRescuePolicy {
        enabled: true,
        reboot_base_available: true,
    };

    fn local(usage: u8, pressed: bool, modifiers: u8) -> ValidatedLocalTransition {
        ValidatedLocalTransition::from_validated_hid(usage, pressed, modifiers)
    }

    #[test]
    fn either_control_and_either_alt_accept_exact_delete_press() {
        for modifiers in [
            LEFT_CONTROL | LEFT_ALT,
            LEFT_CONTROL | RIGHT_ALT,
            RIGHT_CONTROL | LEFT_ALT,
            RIGHT_CONTROL | RIGHT_ALT,
        ] {
            let mut matcher = LocalRescueMatcher::new();
            assert_eq!(
                matcher.observe(INTERACTIVE, local(DELETE_USAGE, true, modifiers)),
                RescueDecision::RequestAccepted {
                    policy: LOCAL_RESCUE_POLICY,
                    operation: LOCAL_REBOOT_OPERATION,
                }
            );
        }
    }

    #[test]
    fn near_misses_and_non_physical_meaning_make_no_request() {
        let mut matcher = LocalRescueMatcher::new();
        for transition in [
            local(DELETE_USAGE, true, LEFT_CONTROL),
            local(DELETE_USAGE, true, LEFT_ALT),
            local(0x2a, true, LEFT_CONTROL | LEFT_ALT),
            local(DELETE_USAGE, false, LEFT_CONTROL | LEFT_ALT),
        ] {
            assert_eq!(
                matcher.observe(INTERACTIVE, transition),
                RescueDecision::NoRequest
            );
        }
    }

    #[test]
    fn held_delete_triggers_once_and_rearms_only_after_release() {
        let chord = local(DELETE_USAGE, true, LEFT_CONTROL | LEFT_ALT);
        let mut matcher = LocalRescueMatcher::new();
        assert!(matches!(
            matcher.observe(INTERACTIVE, chord),
            RescueDecision::RequestAccepted { .. }
        ));
        assert_eq!(
            matcher.observe(INTERACTIVE, chord),
            RescueDecision::NoRequest
        );
        assert_eq!(
            matcher.observe(
                INTERACTIVE,
                local(DELETE_USAGE, false, LEFT_CONTROL | LEFT_ALT)
            ),
            RescueDecision::NoRequest
        );
        assert!(matches!(
            matcher.observe(INTERACTIVE, chord),
            RescueDecision::RequestAccepted { .. }
        ));
    }

    #[test]
    fn disabled_policy_and_unavailable_reboot_base_are_distinct() {
        let chord = local(DELETE_USAGE, true, RIGHT_CONTROL | RIGHT_ALT);
        let mut disabled = LocalRescueMatcher::new();
        assert_eq!(
            disabled.observe(
                LocalRescuePolicy {
                    enabled: false,
                    reboot_base_available: true,
                },
                chord,
            ),
            RescueDecision::NoRequest
        );
        let mut unavailable = LocalRescueMatcher::new();
        assert_eq!(
            unavailable.observe(
                LocalRescuePolicy {
                    enabled: true,
                    reboot_base_available: false,
                },
                chord,
            ),
            RescueDecision::RebootBaseUnavailable {
                policy: LOCAL_RESCUE_POLICY,
            }
        );
    }
}
