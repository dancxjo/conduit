//! Validated physical keyboard transitions, independent of the controller.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HidKeyTransition {
    usage: u8,
    pressed: bool,
    modifiers: u8,
}

impl HidKeyTransition {
    #[cfg(test)]
    pub(crate) const fn new(usage: u8, pressed: bool, modifiers: u8) -> Self {
        Self {
            usage,
            pressed,
            modifiers,
        }
    }

    /// Constructs a transition after a non-HID physical adapter has validated
    /// and translated its mechanism-specific code into the portable keyboard
    /// usage vocabulary.
    pub(crate) const fn from_validated_physical(usage: u8, pressed: bool, modifiers: u8) -> Self {
        Self {
            usage,
            pressed,
            modifiers,
        }
    }
    /// Carry an already decoded Source transition. This descriptive value does
    /// not establish physical origin; Root retains the admitted USB provenance.
    pub(crate) const fn from_source(usage: u8, pressed: bool, modifiers: u8) -> Self {
        Self {
            usage,
            pressed,
            modifiers,
        }
    }
    pub const fn usage(self) -> u8 {
        self.usage
    }

    pub const fn pressed(self) -> bool {
        self.pressed
    }

    pub const fn modifiers(self) -> u8 {
        self.modifiers
    }

    /// Crosses the local adapter boundary only after HID report validation.
    pub const fn into_local_rescue(self) -> crate::local_rescue::ValidatedLocalTransition {
        crate::local_rescue::ValidatedLocalTransition::from_validated_hid(
            self.usage,
            self.pressed,
            self.modifiers,
        )
    }
}
