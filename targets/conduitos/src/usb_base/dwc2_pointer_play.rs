//! Existing mouse Source over one original retained controller.
pub(crate) use super::dwc2_hid_play::{
    HidPlayRefusal as PointerPlayRefusal, HidPlaySchemas as PointerPlaySchemas,
};
use super::{
    dwc2_endpoint_read::RootEndpointRoute, dwc2_hid_play::*, installation::UsbNativeIdentity,
};
use crate::{
    arch::dwc2::{Controller, RegisterIo},
    protocol_source::NativeProtocolIssuer,
};
use conduit_composite::{KernelCompositeHost, KernelCompositeStatus};
pub(crate) struct Dwc2PointerPlay<I> {
    controller: Controller<I>,
    play: Dwc2HidPlay,
}
impl<I: RegisterIo> Dwc2PointerPlay<I> {
    /// # Safety
    /// Root retains the original configured controller and exact endpoint grant.
    pub(crate) unsafe fn prepare(
        controller: Controller<I>,
        route: RootEndpointRoute,
        identity: &UsbNativeIdentity,
        issuer: NativeProtocolIssuer,
    ) -> Result<(Self, PointerPlaySchemas), PointerPlayRefusal> {
        let (play, schemas) =
            unsafe { Dwc2HidPlay::prepare(&controller, false, route, identity, issuer) }?;
        Ok((Self { controller, play }, schemas))
    }
    pub(crate) fn kernel_mut(&mut self) -> &mut KernelCompositeHost {
        self.play.kernel_mut()
    }
    pub(crate) fn step(&mut self) -> Result<KernelCompositeStatus, PointerPlayRefusal> {
        self.play.step(&mut self.controller)
    }
    /// The caller can borrow this sole bus while retaining independent admitted
    /// endpoint custody. The next transfer still validates physical quiescence.
    pub(crate) fn controller(&mut self) -> &mut Controller<I> {
        &mut self.controller
    }
}
#[cfg(test)]
mod tests;
