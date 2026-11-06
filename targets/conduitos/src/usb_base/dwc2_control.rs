//! The existing selected control owner joined to exclusively owned DWC2 PIO.
//! Request/class meaning stays in Source; there is no second kernel or grant.
#![allow(dead_code)] // Normal product Base admission is a separate integration step.
use super::{
    control_owner::{
        ControlCallOwner, ControlOwnerRefusal, NativeControlObservation, NativeControlSubmission,
    },
    control_result::ControlTransferDisposition,
};
use crate::arch::dwc2::{
    Controller, EndpointRoute, PacketError, RegisterIo, TransactionTranslator,
};
use conduit_core::BaseCapabilityRefusal;
use conduit_kernel::{HostCallId, NodeId, RequestId};

/// These are Root-retained physical routing facts, never request fields.
#[derive(Clone, Copy)]
pub(crate) struct RootControlRoute {
    pub slot: u8,
    pub generation: u64,
    pub address: u8,
    pub maximum_packet: u8,
    pub translator: Option<TransactionTranslator>,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Dwc2ControlRefusal {
    Binding,
    Ownership(ControlOwnerRefusal),
    Native(PacketError),
}
pub(crate) struct Dwc2ControlHost<I> {
    controller: Controller<I>,
    owner: ControlCallOwner,
    route: RootControlRoute,
    input: [u8; 256],
    pending: Option<NativeControlSubmission>,
    lost: bool,
    revoked: bool,
    bus_reservation: Option<super::dwc2_bus_route::BusControlReservation>,
}
impl<I: RegisterIo> Dwc2ControlHost<I> {
    /// # Safety
    /// Root owns this exact controller and original attachment, including the
    /// address/default-address reservation, and has bound `owner` to its native
    /// resource envelope and generation. Copied matching IDs grant no possession.
    /// No other controller consumer may access the register window. Retain this
    /// host through acknowledged retirement; dropping it acknowledges nothing.
    pub unsafe fn bind_owned(
        controller: Controller<I>,
        owner: ControlCallOwner,
        route: RootControlRoute,
    ) -> Result<Self, Dwc2ControlRefusal> {
        let attachment = owner.native_attachment();
        let endpoint = EndpointRoute {
            translator: route.translator,
            ..EndpointRoute::control(route.address, route.maximum_packet)
        };
        if !endpoint.valid()
            || !controller.is_prepared()
            || !controller.is_quiescent()
            || attachment.slot != route.slot
            || attachment.generation != route.generation
            || attachment.maximum_data_bytes > 256
            || attachment.resource_bytes < 256
            || route.address > 127
            || !matches!(route.maximum_packet, 8 | 16 | 32 | 64)
            || owner.has_pending_lease()
        {
            return Err(Dwc2ControlRefusal::Binding);
        }
        Ok(Self {
            controller,
            owner,
            route,
            input: [0; 256],
            pending: None,
            lost: false,
            revoked: false,
            bus_reservation: None,
        })
    }
    /// Bind a controller-wide native scope and its independently retained
    /// finite wire-address reservation. This does not broaden a device claim.
    ///
    /// # Safety
    /// All `bind_owned` obligations hold, and `owner` really possesses the
    /// entire bus named by `reservation`, not merely one discovered device.
    pub unsafe fn bind_owned_bus(
        controller: Controller<I>,
        owner: ControlCallOwner,
        route: RootControlRoute,
        reservation: super::dwc2_bus_route::BusControlReservation,
    ) -> Result<Self, Dwc2ControlRefusal> {
        if !reservation.permits(route) {
            return Err(Dwc2ControlRefusal::Binding);
        }
        // SAFETY: Root supplies the original controller and bus-wide scope.
        let mut host = unsafe { Self::bind_owned(controller, owner, route)? };
        host.bus_reservation = Some(reservation);
        Ok(host)
    }

    /// Change only wire geometry within the retained bus reservation, between
    /// settled calls. Logical attachment identity and original possession stay
    /// exact. Ordinary per-device owners have no such reservation.
    pub fn select_reserved_route(
        &mut self,
        route: RootControlRoute,
    ) -> Result<(), Dwc2ControlRefusal> {
        if self.retains_submission() || !self.controller.is_quiescent() {
            return Err(Dwc2ControlRefusal::Native(PacketError::Retirement));
        }
        let endpoint = EndpointRoute {
            translator: route.translator,
            ..EndpointRoute::control(route.address, route.maximum_packet)
        };
        if self.lost
            || self.revoked
            || !self.controller.is_prepared()
            || route.slot != self.route.slot
            || route.generation != self.route.generation
            || !matches!(route.maximum_packet, 8 | 16 | 32 | 64)
            || !endpoint.valid()
            || self
                .bus_reservation
                .as_ref()
                .is_none_or(|scope| !scope.permits(route))
        {
            return Err(Dwc2ControlRefusal::Binding);
        }
        self.route = route;
        Ok(())
    }

    pub fn execute(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request_id: RequestId,
        input: &[u8],
    ) -> Result<&[u8], Dwc2ControlRefusal> {
        if self.pending.is_some() {
            return Err(Dwc2ControlRefusal::Ownership(ControlOwnerRefusal::Pressure));
        }
        if self.lost {
            return Err(Dwc2ControlRefusal::Native(PacketError::ProviderLost));
        }
        if !self.controller.is_prepared() {
            return Err(Dwc2ControlRefusal::Native(PacketError::NotPrepared));
        }
        let submission = self
            .owner
            .begin(node, call, request_id, input)
            .map_err(Dwc2ControlRefusal::Ownership)?;
        self.pending = Some(submission);
        let request = self
            .pending
            .as_ref()
            .expect("accepted submission retained")
            .request()
            .map_err(|reason| {
                Dwc2ControlRefusal::Ownership(ControlOwnerRefusal::Decode(
                    super::control_decode::ControlDecodeRefusal::Transfer(reason),
                ))
            })?;
        let incoming = request.input();
        let result = self.controller.control_on(
            EndpointRoute {
                translator: self.route.translator,
                ..EndpointRoute::control(self.route.address, self.route.maximum_packet)
            },
            *request.setup(),
            request.output(),
            &mut self.input,
        );
        self.lost |= matches!(result, Err(PacketError::ProviderLost));
        let observation = match result {
            Ok(actual) => NativeControlObservation::Completed {
                actual: actual as u16,
                input: if incoming { &self.input[..actual] } else { &[] },
            },
            Err(PacketError::Stall) if self.controller.is_quiescent() => {
                NativeControlObservation::Disposition(ControlTransferDisposition::Stalled)
            }
            Err(PacketError::ProviderLost) if self.controller.is_quiescent() => {
                NativeControlObservation::Disposition(ControlTransferDisposition::ProviderLost)
            }
            // NAK/NYET/deadline/pressure are not "unsupported" or success. The
            // native refusal retains its exact lease until explicit retirement.
            Err(reason) => return Err(Dwc2ControlRefusal::Native(reason)),
        };
        if !self.controller.is_quiescent() {
            return Err(Dwc2ControlRefusal::Native(PacketError::Retirement));
        }
        let submission = self.pending.take().expect("completed submission retained");
        // SAFETY: PIO stores no memory address in hardware. The exact retained
        // controller acknowledged every submitted packet's channel halt, or no
        // packet was submitted at all. A timeout alone never reaches this arm.
        unsafe { self.owner.finish_quiesced(&submission, observation) }
            .map_err(Dwc2ControlRefusal::Ownership)
    }
    /// Revoke first. A lost lease does not establish physical retirement.
    pub fn revoke(&mut self, node: NodeId, call: HostCallId) -> Result<(), Dwc2ControlRefusal> {
        self.owner
            .revoke(node, call)
            .map_err(Dwc2ControlRefusal::Ownership)?;
        self.revoked = true;
        Ok(())
    }
    /// Settle an abandoned/refused call only after controller-confirmed halt.
    /// The caller must separately terminate its kernel call/Play as refused;
    /// this method fabricates neither an output nor a successful completion.
    pub fn discard_retired(&mut self) -> Result<(), Dwc2ControlRefusal> {
        if !self.controller.is_quiescent() {
            return Err(Dwc2ControlRefusal::Native(PacketError::Retirement));
        }
        if let Some(submission) = self.pending.as_ref() {
            // SAFETY: this host exclusively retains the original PIO controller,
            // attachment and submission; its hardware retirement is recorded.
            match unsafe { self.owner.discard_quiesced(submission) } {
                Ok(())
                | Err(ControlOwnerRefusal::Capability(BaseCapabilityRefusal::StaleCompletion)) => {}
                Err(reason) => return Err(Dwc2ControlRefusal::Ownership(reason)),
            }
            self.pending = None;
        }
        Ok(())
    }
    /// Return the original controller only after settling every physical call
    /// and revoking this selection. Refusal returns the whole owner, preserving
    /// its original controller and pending lease rather than dropping custody.
    #[allow(clippy::result_large_err)] // Return original ownership without allocating on refusal.
    pub fn release_completed(
        mut self,
        node: NodeId,
        call: HostCallId,
    ) -> Result<Controller<I>, (Self, Dwc2ControlRefusal)> {
        if self.retains_submission() || !self.controller.is_quiescent() {
            return Err((self, Dwc2ControlRefusal::Native(PacketError::Retirement)));
        }
        if !self.revoked
            && let Err(reason) = self.revoke(node, call)
        {
            return Err((self, reason));
        }
        Ok(self.controller)
    }
    pub fn retains_submission(&self) -> bool {
        self.pending.is_some() || self.owner.has_pending_lease()
    }
}
