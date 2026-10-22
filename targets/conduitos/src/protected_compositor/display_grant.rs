//! Root's binding of one validated display provider and concrete scanout format.
//! This is private possession, not a guest claim or a physical-display receipt.
use crate::display::{DisplayError, DisplayFormat};
use alloc::rc::Rc;
use conduit_core::{BootId, HostBaseId, HostId, OfferGeneration};

/// Shared immutable Root custody of one checked display binding. Cloning this
/// handle allocates no identity strings or Rc metadata. It does not refresh the
/// provider, extend a Boot/offer lifetime, or authorize an old Mask/Show/input.
pub(crate) type SharedDisplayGrant = Rc<DisplayGrant>;

#[derive(Clone)]
pub(crate) struct DisplayGrant {
    host: HostId,
    boot: BootId,
    generation: OfferGeneration,
    base: HostBaseId,
    format: DisplayFormat,
}

impl DisplayGrant {
    /// Root must first match the discovered provider to its framebuffer Basis.
    /// Equal formats alone do not identify a device; the boot call site binds
    /// this grant to the concrete scanout target it continues to own.
    pub(crate) fn new(
        host: HostId,
        boot: BootId,
        generation: OfferGeneration,
        base: HostBaseId,
        format: DisplayFormat,
    ) -> Result<Self, DisplayError> {
        let format = format.validate()?;
        if format.width > u32::from(u16::MAX) || format.height > u32::from(u16::MAX) {
            return Err(DisplayError::InvalidExtent);
        }
        Ok(Self {
            host,
            boot,
            generation,
            base,
            format,
        })
    }

    pub(crate) fn matches(
        &self,
        host: &HostId,
        boot: &BootId,
        generation: OfferGeneration,
        base: &HostBaseId,
    ) -> bool {
        self.host == *host
            && self.boot == *boot
            && self.generation == generation
            && self.base == *base
    }

    pub(crate) fn format_matches(&self, format: DisplayFormat) -> bool {
        self.format == format
    }

    pub(crate) fn extent(&self) -> (u16, u16) {
        // Only the checked constructor can create this private immutable value.
        (self.format.width as u16, self.format.height as u16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn format() -> DisplayFormat {
        DisplayFormat {
            width: 3840,
            height: 2160,
            pitch: 3840 * 4,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }
    #[test]
    fn exact_host_boot_generation_and_display_are_required() {
        let host = HostId::from("host");
        let boot = BootId::from("boot");
        let base = HostBaseId::from("display");
        let grant = DisplayGrant::new(
            host.clone(),
            boot.clone(),
            OfferGeneration(3),
            base.clone(),
            format(),
        )
        .unwrap();
        assert!(grant.matches(&host, &boot, OfferGeneration(3), &base));
        assert!(!grant.matches(&HostId::from("other"), &boot, OfferGeneration(3), &base));
        assert!(!grant.matches(&host, &BootId::from("old"), OfferGeneration(3), &base));
        assert!(!grant.matches(&host, &boot, OfferGeneration(2), &base));
        assert!(!grant.matches(&host, &boot, OfferGeneration(3), &HostBaseId::from("other")));
        // A 4K viewport is valid even though a single retained surface is bounded
        // independently by its smaller pixel budget.
        assert_eq!(grant.extent(), (3840, 2160));
    }
    #[test]
    fn scanout_format_changes_require_new_admission() {
        let grant = DisplayGrant::new(
            HostId::from("host"),
            BootId::from("boot"),
            OfferGeneration(3),
            HostBaseId::from("display"),
            format(),
        )
        .unwrap();
        assert!(grant.format_matches(format()));
        for changed in [
            DisplayFormat {
                width: 3839,
                ..format()
            },
            DisplayFormat {
                pitch: 3840 * 4 + 4,
                ..format()
            },
            DisplayFormat {
                red_shift: 0,
                blue_shift: 16,
                ..format()
            },
        ] {
            assert!(!grant.format_matches(changed));
        }
        assert!(
            DisplayGrant::new(
                HostId::from("host"),
                BootId::from("boot"),
                OfferGeneration(3),
                HostBaseId::from("display"),
                DisplayFormat {
                    width: 0,
                    ..format()
                }
            )
            .is_err()
        );
    }
}

#[cfg(test)]
#[path = "display_grant/recovery_tests.rs"]
mod recovery_tests;
