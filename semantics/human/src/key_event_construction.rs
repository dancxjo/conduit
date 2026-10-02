// Included beside the generated private fields. The public constructor keeps
// the exact keyboard usage and modifier laws, without allocating a structured
// invariant graph on every live key transition. new_native remains the general
// generated validator; conformance tests compare both construction routes.
impl KeyEvent {
    pub fn new(
        usage: u8,
        transition: KeyTransition,
        modifiers_after: KeyModifiers,
    ) -> Result<Self, conduit_core::InfoDecodeError> {
        if !crate::key_event::is_canonical_keyboard_usage(usage) {
            return Err(conduit_core::InfoDecodeError::ReservedValue {
                field: "keyboard-usage",
                actual: usage,
            });
        }
        if crate::key_event::is_modifier_usage(usage)
            && modifiers_after.contains_usage(usage) != matches!(transition, KeyTransition::Pressed)
        {
            return Err(conduit_core::InfoDecodeError::InconsistentValue(
                "modifier-after-transition",
            ));
        }
        let contains = |modifier: KeyModifiers| modifiers_after.bits() & modifier.bits() != 0;
        Ok(Self {
            usage,
            transition,
            left_control_after: contains(KeyModifiers::LEFT_CONTROL),
            left_shift_after: contains(KeyModifiers::LEFT_SHIFT),
            left_alt_after: contains(KeyModifiers::LEFT_ALT),
            left_gui_after: contains(KeyModifiers::LEFT_GUI),
            right_control_after: contains(KeyModifiers::RIGHT_CONTROL),
            right_shift_after: contains(KeyModifiers::RIGHT_SHIFT),
            right_alt_after: contains(KeyModifiers::RIGHT_ALT),
            right_gui_after: contains(KeyModifiers::RIGHT_GUI),
        })
    }
}
