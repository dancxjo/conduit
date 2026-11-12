//! A finite synchronous kernel/IO mailbox, not a scheduler or transport.
//! Preparing storage or receiving public scope bytes confers no authority. Only
//! the actual bound Root carrier can activate its original Started bridge.
//! Delivered means transport/kernel custody, never durable recording. Normal or
//! abnormal terminal control is not implemented by this payload-only adapter.
use crate::{
    execution_workset::{PreparedLocalKernels, StartedEvidence},
    native_runner::Observer,
};
use conduit_core as c;
use conduit_kernel::{CordId, RemoteEndpointId};
use core::cell::{Cell, RefCell};
#[path = "fore_bridge/primitive.rs"]
mod primitive;
pub use primitive::PrimitiveIngressSpec;
#[path = "fore_bridge/mailbox.rs"]
mod mailbox;
pub use mailbox::{CarrierPort, Event};
#[path = "fore_bridge/observer.rs"]
mod observer;
pub use observer::{ForeObserver, ObserverRefusal};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Bounds,
    UnsupportedAbi,
    InvalidValue,
    Busy,
    Sequence,
    Inactive,
    Scope,
}
#[path = "fore_bridge/egress.rs"]
mod egress;
pub use egress::EgressSpec;
#[derive(Clone, Copy)]
struct Route {
    endpoint: RemoteEndpointId,
    cord: CordId,
}
impl Route {
    fn from_pin(pin: &c::AssignedLineForeRequirement) -> Self {
        Self {
            endpoint: RemoteEndpointId(pin.fore().endpoint()),
            cord: CordId(pin.fore().cord()),
        }
    }
}
/// Pure inert storage and typed maps. Supply disjoint preallocated payload
/// slices; the Root carrier must not charge these SAME slices a second time.
/// There is one pending item per direction and no hidden growth or IO.
pub struct PreparedForeBridge<'b, 'p> {
    shared: RefCell<mailbox::State<'b, 'p>>,
    retired: Cell<bool>,
    custody: [u8; 32],
    pins: [[u8; 32]; 2],
}
impl<'b, 'p> PreparedForeBridge<'b, 'p> {
    pub fn prepare(
        custody: [u8; 32],
        ingress: PrimitiveIngressSpec<'p>,
        egress: EgressSpec<'p>,
        ingress_bytes: &'b mut [u8],
        egress_bytes: &'b mut [u8],
    ) -> Result<Self, Refusal> {
        ingress.check()?;
        if egress.pin().fore().direction() != c::AssignedForeDirection::Output
            || egress.byte_capacity() == 0
            || ingress.byte_capacity as usize > ingress_bytes.len()
            || egress.byte_capacity() as usize > egress_bytes.len()
            || ingress.pin.fore().endpoint() == egress.pin().fore().endpoint()
            || ingress.pin.fore().cord() == egress.pin().fore().cord()
        {
            return Err(Refusal::Bounds);
        }
        let pins = [
            c::assigned_plan_payload_digest(ingress.pin.payload()),
            c::assigned_plan_payload_digest(egress.pin().payload()),
        ];
        Ok(Self {
            shared: RefCell::new(mailbox::State::new(
                ingress,
                egress,
                ingress_bytes,
                egress_bytes,
            )),
            custody,
            pins,
            retired: Cell::new(false),
        })
    }
    /// Descriptive consistency preflight BEFORE resource admission/Start. The
    /// original Root carrier must independently prove these inputs; matching
    /// public commitments neither admits resources nor activates this bridge.
    pub(crate) fn check_original_policy(
        &self,
        custody: &[u8; 32],
        input: &c::AssignedLineForeRequirement,
        output: &c::AssignedLineForeRequirement,
    ) -> Result<(), Refusal> {
        let pins = [
            c::assigned_plan_payload_digest(input.payload()),
            c::assigned_plan_payload_digest(output.payload()),
        ];
        self.check_policy_commitments(custody, &pins)
    }
    fn check_policy_commitments(
        &self,
        custody: &[u8; 32],
        pins: &[[u8; 32]; 2],
    ) -> Result<(), Refusal> {
        if &self.custody == custody && &self.pins == pins {
            Ok(())
        } else {
            Err(Refusal::Scope)
        }
    }
}
/// Bound by original whole-workset Start but still inert until Root activates
/// its same actual carrier and BOTH exact Fore scopes.
pub struct BoundForeBridge<'b, 'p>(PreparedForeBridge<'b, 'p>);
// SAFETY: owns only finite borrowed storage. Bind moves it with no allocation,
// provider transfer, effect or independent activation.
unsafe impl<'b, 'p> PreparedLocalKernels for PreparedForeBridge<'b, 'p> {
    type Bound = BoundForeBridge<'b, 'p>;
    fn retained_backing_bytes(&self) -> Option<usize> {
        let state = self.shared.borrow();
        core::mem::size_of::<Self>()
            .checked_add(state.ingress_bytes.len())?
            .checked_add(state.egress_bytes.len())
    }
    unsafe fn bind(self) -> Self::Bound {
        BoundForeBridge(self)
    }
}
/// Opaque affine crate-private activation; public hashes cannot construct it.
/// The parent Root actor owns issuance after its original carrier checks.
pub(crate) struct ForeBridgeActivation {
    custody: [u8; 32],
    pins: [[u8; 32]; 2],
}
impl ForeBridgeActivation {
    /// # Safety
    /// Caller MUST hold the actual affine BoundForeCarrierKernels, verify its
    /// matches_started(started) and both original per-Fore session/scope bindings,
    /// and retain that carrier and current peer possession through retirement.
    /// Neither StartedEvidence nor scope commitments alone satisfy this rule.
    pub(crate) unsafe fn from_verified_root(
        started: &StartedEvidence,
        ingress: &c::AssignedLineForeRequirement,
        egress: &c::AssignedLineForeRequirement,
    ) -> Self {
        Self {
            custody: *started.custody_identity(),
            pins: [
                c::assigned_plan_payload_digest(ingress.payload()),
                c::assigned_plan_payload_digest(egress.payload()),
            ],
        }
    }
}
/// Dropping either actor port fences further bridge transfers without
/// cancelling the original kernel or releasing its undelivered Values.
pub struct ObserverPort<'a, 'b, 'p> {
    shared: &'a RefCell<mailbox::State<'b, 'p>>,
    retired: &'a Cell<bool>,
}
impl Drop for ObserverPort<'_, '_, '_> {
    fn drop(&mut self) {
        self.retired.set(true);
    }
}
impl<'b, 'p> BoundForeBridge<'b, 'p> {
    /// Same descriptive original-policy check for the already bound owner.
    /// This does not replace the actual carrier/Started activation checks.
    pub(crate) fn check_original_policy(
        &self,
        custody: &[u8; 32],
        input: &c::AssignedLineForeRequirement,
        output: &c::AssignedLineForeRequirement,
    ) -> Result<(), Refusal> {
        self.0.check_original_policy(custody, input, output)
    }
    /// Root-only activation after checking the actual original carrier ticket.
    pub(crate) fn activate<'a>(
        &'a self,
        activation: ForeBridgeActivation,
    ) -> Result<(ObserverPort<'a, 'b, 'p>, CarrierPort<'a, 'b, 'p>), Refusal> {
        self.0
            .check_policy_commitments(&activation.custody, &activation.pins)?;
        let mut state = self.0.shared.borrow_mut();
        if state.active || self.0.retired.get() {
            return Err(Refusal::Inactive);
        }
        state.active = true;
        Ok((
            ObserverPort {
                shared: &self.0.shared,
                retired: &self.0.retired,
            },
            CarrierPort {
                shared: &self.0.shared,
                retired: &self.0.retired,
            },
        ))
    }
}

#[cfg(test)]
#[path = "fore_bridge/tests.rs"]
mod tests;
