//! Actual framed IO over the original bound Root loan, with no kernel loan.
use super::*;
use crate::fore_bridge::{
    self as bridge, BoundForeBridge, CarrierPort, EgressSpec, Event, ForeBridgeActivation,
    ObserverPort, PreparedForeBridge, PrimitiveIngressSpec,
};
use conduit_wire::{
    ExternalForeSessionBinding as Binding, ExternalForeSessionMachine as Machine,
    ExternalForeSessionMessage as Message, SessionMessage, SessionRole,
};
#[path = "actor/io.rs"]
mod io;
#[path = "actor/state.rs"]
mod state;
use state::{Pending, Route};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForeIoRefusal {
    Scope,
    Bounds,
    Wire,
    Bridge,
    Io,
    Unsupported,
    Inactive,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForeIoProgress {
    Idle,
    Progress,
    Busy,
}
/// Both exact CNDF machines prepared before ordinary admission/Start. No IO,
/// activation, carrier, strings, schema copies or extra payload buffers live here.
pub struct PreparedForeSessions {
    routes: [Route; 2],
    custody: [u8; 32],
    pending: Pending,
    used_frames: u32,
    maximum_frames: u32,
    activated: bool,
    retired: bool,
}
pub struct BoundForeSessions(PreparedForeSessions);
// SAFETY: finite protocol state only; bind moves it without effects/allocation.
unsafe impl PreparedLocalKernels for PreparedForeSessions {
    type Bound = BoundForeSessions;
    fn retained_backing_bytes(&self) -> Option<usize> {
        Some(core::mem::size_of::<Self>())
    }
    unsafe fn bind(self) -> Self::Bound {
        BoundForeSessions(self)
    }
}
impl<C: MessageTransport, const PORTS: usize> PreparedForeCarrier<'_, '_, C, PORTS> {
    /// Check BOTH actual selected routes, their ABI policy and original bridge
    /// before admission. Group native kernels, bridge and this result using the
    /// existing PreparedKernelGroup, then attach to THIS carrier ticket.
    pub fn prepare_bridge_sessions(
        &self,
        bridge: &PreparedForeBridge<'_, '_>,
        input: PrimitiveIngressSpec<'_>,
        output: EgressSpec<'_>,
    ) -> Result<PreparedForeSessions, ForeIoRefusal> {
        bridge
            .check_original_policy(&self.custody, input.pin(), output.pin())
            .map_err(|_| ForeIoRefusal::Scope)?;
        let make = |pin: &c::AssignedLineForeRequirement,
                    payload: u32,
                    role|
         -> Result<Route, ForeIoRefusal> {
            let endpoint = pin.fore().endpoint();
            let cord = pin.fore().cord();
            let original = self
                .pins
                .iter()
                .find(|p| p.fore().endpoint() == endpoint && p.fore().cord() == cord)
                .ok_or(ForeIoRefusal::Scope)?;
            if original.payload() != pin.payload() {
                return Err(ForeIoRefusal::Scope);
            }
            let limits = self
                .selected_line_limits(endpoint, cord)
                .ok_or(ForeIoRefusal::Scope)?;
            if payload == 0
                || payload > limits.maximum_payload_bytes
                || payload > limits.maximum_buffered_bytes
                || payload
                    .checked_add(52)
                    .is_none_or(|n| n > limits.maximum_frame_bytes)
                || limits.maximum_frame_bytes as usize > self.backing.len()
                || limits.maximum_in_flight_items == 0
            {
                return Err(ForeIoRefusal::Bounds);
            }
            let binding = Binding {
                scope_commitment: self
                    .fore_scope_digest(endpoint, cord)
                    .ok_or(ForeIoRefusal::Scope)?,
                limits: conduit_wire::SessionLimits {
                    maximum_in_flight_items: 1,
                    maximum_payload_bytes: payload,
                    maximum_buffered_bytes: payload,
                },
                abnormal_kind: None,
                maximum_frame_bytes: limits.maximum_frame_bytes,
            };
            Route::new(
                endpoint,
                cord,
                Machine::new(binding, role).map_err(|_| ForeIoRefusal::Wire)?,
            )
        };
        if input.pin().fore().direction() != c::AssignedForeDirection::Input
            || output.pin().fore().direction() != c::AssignedForeDirection::Output
        {
            return Err(ForeIoRefusal::Scope);
        }
        let routes = [
            make(input.pin(), input.byte_capacity(), SessionRole::Sink)?,
            make(output.pin(), output.byte_capacity(), SessionRole::Source)?,
        ];
        if routes[0].machine.binding().scope_commitment
            == routes[1].machine.binding().scope_commitment
        {
            return Err(ForeIoRefusal::Scope);
        }
        Ok(PreparedForeSessions {
            routes,
            custody: self.custody,
            pending: Pending::None,
            used_frames: 0,
            maximum_frames: self._bounds.maximum_frames,
            activated: false,
            retired: false,
        })
    }
}
/// The actor borrows the real platform framed transport intact. No raw extractor
/// or hash-based constructor exists. It holds no kernel/RefCell loan across IO.
pub struct ForeIoActor<'a, 'b, 'p, C: MessageTransport> {
    link: &'a mut C,
    buffer: &'a mut [u8],
    state: &'a mut PreparedForeSessions,
    port: CarrierPort<'a, 'b, 'p>,
}
impl<'c, 'b, 'p, C: MessageTransport, K, const PORTS: usize>
    BoundForeCarrierKernels<'c, 'p, C, (K, BoundForeBridge<'b, 'p>, BoundForeSessions), PORTS>
{
    /// Scope refusal leaves ALL original owners in place. Success splits only
    /// disjoint loans, avoiding a full native-group move/copy. The sampler and
    /// display can keep running if this independent actor later retires.
    #[allow(clippy::type_complexity)]
    pub fn activate_actor<'a>(
        &'a mut self,
        started: &StartedEvidence,
    ) -> Result<
        (
            &'a mut K,
            ObserverPort<'a, 'b, 'p>,
            ForeIoActor<'a, 'b, 'p, C>,
        ),
        ForeIoRefusal,
    > {
        if !self.matches_started(started) {
            return Err(ForeIoRefusal::Scope);
        }
        let (kernels, bridge, sessions) = &mut self.kernels;
        let carrier = &mut self._carrier;
        let state = &mut sessions.0;
        if state.activated || state.retired || state.custody != carrier.custody {
            return Err(ForeIoRefusal::Scope);
        }
        let mut pins = [None; 2];
        for (index, route) in state.routes.iter().enumerate() {
            let pin = carrier
                .pins
                .iter()
                .find(|pin| {
                    pin.fore().endpoint() == route.endpoint && pin.fore().cord() == route.cord
                })
                .ok_or(ForeIoRefusal::Scope)?;
            if carrier.fore_scope_digest(route.endpoint, route.cord)
                != Some(route.machine.binding().scope_commitment)
            {
                return Err(ForeIoRefusal::Scope);
            }
            pins[index] = Some(pin);
        }
        let input = pins[0].ok_or(ForeIoRefusal::Scope)?;
        let output = pins[1].ok_or(ForeIoRefusal::Scope)?;
        bridge
            .check_original_policy(&carrier.custody, input, output)
            .map_err(|_| ForeIoRefusal::Scope)?;
        // SAFETY: actual original affine bound carrier retained here, SAME Start
        // custody, BOTH exact installed rows/scopes and bridge policy checked.
        let activation =
            unsafe { ForeBridgeActivation::from_verified_root(started, input, output) };
        let (observer, port) = bridge
            .activate(activation)
            .map_err(|_| ForeIoRefusal::Bridge)?;
        state.activated = true;
        Ok((
            kernels,
            observer,
            ForeIoActor {
                link: carrier._link,
                buffer: carrier.backing,
                state,
                port,
            },
        ))
    }
}
#[cfg(test)]
#[path = "actor/tests.rs"]
mod tests;
