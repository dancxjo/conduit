//! Root-only physical plumbing for the product controller.
//!
//! The controller receives portable input and connectivity observations, plus
//! a checked write-only display sink. Hardware handles stay in this module.
//! This is preparation for a service message boundary, not CPU isolation.

use crate::{
    arch::{HidKeyboardSession, HidPointerSession, UsbDevice, XhciReady},
    display::{DisplayError, DisplayFormat, PixelTarget},
    identity::BootIdentities,
    keyboard_input::{self, ProductInputControl, ProductInputEvent},
    local_rescue::LocalRescueMatcher,
    make::MakeRecord,
    offer_make::ImageBoundHostOffer,
    product_bases::NativeProductBases,
};

#[allow(clippy::too_many_arguments)]
pub fn run(
    identities: &BootIdentities,
    offer: &ImageBoundHostOffer<'_>,
    make: &MakeRecord,
    framebuffer_basis: &conduit_observatory::FramebufferBasis,
    display: &mut impl crate::display::PixelTarget,
    hid_session: Option<&mut HidKeyboardSession>,
    controller: &mut XhciReady,
    controller_id: [u8; 32],
    usb: &UsbDevice,
    pointer_session: Option<&mut HidPointerSession>,
    pointer_usb: Option<&UsbDevice>,
    usb_line_device: Option<&UsbDevice>,
    ps2_input: Option<&mut crate::arch::Ps2Input>,
    rescue_matcher: &mut LocalRescueMatcher,
    effect_bases: NativeProductBases,
    surface_provider: crate::product_bases::NativeSurfaceProvider,
    pending_join: Option<crate::native_boot_join::BootJoinOutcome>,
) -> Result<(), &'static str> {
    let mut display = DisplaySink::new(display, framebuffer_basis)?;
    let mut platform = RootPlatform {
        keyboard: hid_session,
        controller,
        controller_id,
        keyboard_device: usb,
        pointer_admitted: pointer_session.is_some() || ps2_input.is_some(),
        pointer: pointer_session.zip(pointer_usb),
        line_device: usb_line_device,
        ps2: ps2_input,
    };
    super::run_controller(
        identities,
        offer,
        make,
        framebuffer_basis,
        &mut display,
        &mut platform,
        rescue_matcher,
        effect_bases,
        surface_provider,
        pending_join,
    )
}

pub(super) struct RootPlatform<'a> {
    keyboard: Option<&'a mut HidKeyboardSession>,
    controller: &'a mut XhciReady,
    controller_id: [u8; 32],
    keyboard_device: &'a UsbDevice,
    pointer: Option<(&'a mut HidPointerSession, &'a UsbDevice)>,
    pointer_admitted: bool,
    line_device: Option<&'a UsbDevice>,
    ps2: Option<&'a mut crate::arch::Ps2Input>,
}

/// Borrowed finite Line observations; no device handle crosses the callback.
/// The current named connectivity harness has exactly LINE_LIFETIME_VALUES
/// payload opportunities and retains its existing authority/admission rules.
pub(super) struct ConnectivityObservation<'a> {
    pub status: crate::front_door::ConnectivityStatus,
    pub line_id: &'a str,
    pub value: Option<&'a str>,
}

impl RootPlatform<'_> {
    pub fn keyboard_available(&self) -> bool {
        self.keyboard.is_some() || self.ps2.is_some()
    }

    pub fn pointer_available(&self) -> bool {
        self.pointer_admitted
    }

    /// Reuse the admitted physical ingress and its bounded semantic slices.
    /// Yield returns controller ownership before a Line command is accepted.
    pub fn run_input(
        &mut self,
        interact: impl FnMut(ProductInputEvent) -> Result<ProductInputControl, &'static str>,
    ) -> Result<(), &'static str> {
        if let Some(ps2) = self.ps2.as_deref_mut() {
            keyboard_input::run_ps2_product(ps2, interact)
        } else {
            keyboard_input::run_product_with_pointer(
                self.keyboard
                    .as_deref_mut()
                    .ok_or("front-door-keyboard-realization-missing")?,
                self.controller,
                self.keyboard_device,
                &mut self.pointer,
                interact,
            )
        }
    }

    pub fn run_line(
        &mut self,
        identities: &BootIdentities,
        body_id: &str,
        mut observe: impl FnMut(ConnectivityObservation<'_>) -> Result<(), &'static str>,
    ) -> Result<(), &'static str> {
        let device = self
            .line_device
            .ok_or("product-usb-line-realization-absent")?;
        let ready = crate::arch::prepare_ftdi_line(
            self.controller,
            device,
            crate::boot::executable_physical_address,
        )
        .map_err(|error| error.as_str())?;
        let mut line =
            crate::product_usb_line::prepare(identities, self.controller_id, device, ready)?;
        line.run(
            self.controller,
            device,
            body_id,
            |status, line_id, value| {
                observe(ConnectivityObservation {
                    status,
                    line_id,
                    value,
                })
            },
        )
    }
}

/// A fixed display grant exposing only checked pixel effects. In particular,
/// policy cannot obtain the original framebuffer object or physical address.
/// PixelTarget compatibility keeps existing checked Mask/Show presentation
/// authoritative until that semantic controller is moved into its own guest.
pub(super) struct DisplaySink<'a, T: PixelTarget> {
    target: &'a mut T,
    format: DisplayFormat,
}

impl<'a, T: PixelTarget> DisplaySink<'a, T> {
    fn new(
        target: &'a mut T,
        basis: &conduit_observatory::FramebufferBasis,
    ) -> Result<Self, &'static str> {
        let format = target.format().validate().map_err(DisplayError::as_str)?;
        if format.width != basis.width
            || format.height != basis.height
            || format.pitch != basis.pitch_bytes
            || format.bits_per_pixel != basis.bits_per_pixel
        {
            return Err("product-display-grant-mismatch");
        }
        Ok(Self { target, format })
    }
}

impl<T: PixelTarget> PixelTarget for DisplaySink<'_, T> {
    fn format(&self) -> DisplayFormat {
        self.format
    }

    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
        if x >= self.format.width || y >= self.format.height {
            return Err(DisplayError::InvalidExtent);
        }
        self.target.write_pixel(x, y, pixel)
    }

    fn write_span(&mut self, x: u32, y: u32, pixels: &[u32]) -> Result<(), DisplayError> {
        if pixels.is_empty() {
            return Ok(());
        }
        let count = u32::try_from(pixels.len()).map_err(|_| DisplayError::InvalidExtent)?;
        if y >= self.format.height
            || x.checked_add(count)
                .is_none_or(|end| end > self.format.width)
        {
            return Err(DisplayError::InvalidExtent);
        }
        self.target.write_span(x, y, pixels)
    }
}
