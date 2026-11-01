//! One acknowledged presentation and one explicit, acknowledged withdrawal.
//! Encoded Available bytes remain private until electrical and lease completion.
use super::*;
use alloc::string::String;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RendererState {
    Ready,
    Available,
    /// A successor write failed; no Available assertion, but old cleanup remains.
    Interrupted,
    Withdrawn,
    Failed,
    Cancelled,
}
#[derive(Debug)]
pub enum DeliveryRefusal<E> {
    Binding,
    Lifecycle,
    Face,
    Show(f::MaskShowError),
    Encoding(f::MaskJsonRefusal),
    Mask(crate::display_mask::Refusal),
    Packets(crate::display_packets::DisplayPacketRefusal),
    Capability(c::BaseCapabilityRefusal),
    Electrical(conduit_tm1637_wire::Failure<E>),
}
pub struct RendererCall<P> {
    pub(super) ready: ReadyRenderer<P>,
    pub(super) table: c::BaseCapabilityTable,
    pub(super) handle: c::BaseCapabilityHandle,
    pub(super) claim: c::BaseOperationClaim,
    pub(super) active: c::ActivePlayIdentity,
    pub(super) node: NodeId,
    pub(super) state: RendererState,
}
/// Inputs come from the currently admitted Mask invocation. The Face remains
/// semantic; elapsed time selects the admitted Look's finite page behavior.
pub struct PresentationBasis<'face, 'context> {
    pub face: &'face f::Face,
    pub face_wire: &'face [u8],
    pub context: &'context f::PreparedMaskShowContext,
    pub elapsed_ms: u64,
    pub front: String,
    pub target: String,
    pub prepared_sign: c::SignId,
    pub available_sign: c::SignId,
}
/// Pre-Play material. Available metadata is private and grants no delivery;
/// it can leave the semantic Face read scope but pins its exact input bytes.
pub struct PreparedPresentation<'a> {
    pub(super) occurrence: f::PreparedMaskOccurrence<'a>,
    pub(super) face: f::PreparedFaceValue,
    pub(super) packets: crate::display_packets::SourceDisplayPackets,
    pub(super) request: RequestId,
}
/// Exact input already checked against the prepared Face. It borrows only
/// immutable preparation, so the kernel can reserve a different output slot.
/// Private construction prevents bypassing the input check across that borrow.
pub struct VerifiedPresentation<'p, 'context> {
    pub(super) prepared: &'p PreparedPresentation<'context>,
}
impl<P: OpenDrain> RendererCall<P> {
    pub fn request_sequence(&self) -> RequestId {
        self.ready.request_sequence()
    }
    pub fn state(&self) -> RendererState {
        self.state
    }
    pub fn inspection(&self) -> &c::CapabilityInspection {
        self.table
            .inspections()
            .next()
            .expect("single renderer capability")
    }
    /// Cold preparation only. Root admits its temporary heap/stack, current
    /// Body route/Face/Look and output capacity before releasing the Mask Play.
    pub fn prepare_presentation<'a>(
        &mut self,
        input: PresentationBasis<'_, 'a>,
    ) -> Result<PreparedPresentation<'a>, DeliveryRefusal<P::Error>> {
        use DeliveryRefusal as E;
        if self.state != RendererState::Ready || self.ready.next_request == u32::MAX {
            return Err(E::Lifecycle);
        }
        self.prepare_for(self.active.clone(), input)
    }
    pub(super) fn prepare_for<'a>(
        &mut self,
        active: c::ActivePlayIdentity,
        input: PresentationBasis<'_, 'a>,
    ) -> Result<PreparedPresentation<'a>, DeliveryRefusal<P::Error>> {
        use DeliveryRefusal as E;
        let (face, validated) = f::PreparedFaceValue::prepare_with_view(
            input.face,
            input.face_wire,
            self.ready.bounds.face_bytes as usize,
        )
        .map_err(|_| E::Face)?;
        let mut occurrence = input
            .context
            .prepared_show(
                input.face,
                active,
                input.front,
                input.target,
                input.prepared_sign,
            )
            .map_err(E::Show)?;
        let show = occurrence.show();
        let native = &self.ready.identity;
        if show.presenter_capability_id.as_str() != CAPABILITY
            || show.presenter_implementation_id != native.implementation_id
            || show.presenter_artifact_id != native.artifact_id
        {
            return Err(E::Binding);
        }
        let brightness = self.ready.mask.brightness();
        let frame = self
            .ready
            .mask
            .render_validated(validated, input.elapsed_ms)
            .map_err(E::Mask)?;
        let packets = self
            .ready
            .packets
            .encode(frame.registers, brightness, true)
            .map_err(E::Packets)?;
        occurrence
            .transition(f::ManifestationLifecycle::Available, input.available_sign)
            .map_err(E::Show)?;
        Ok(PreparedPresentation {
            occurrence,
            face,
            packets,
            request: RequestId(self.ready.next_request),
        })
    }
    /// Output must be private admitted backing, discarded entirely on error
    /// (normally the shared kernel encoded-value reservation). Root separately
    /// checks its current Body/Face route before dispatch. No retry or parsing.
    #[allow(clippy::too_many_arguments)]
    pub fn present_encoded(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        face_input: &[u8],
        prepared: &PreparedPresentation<'_>,
        output: &mut [u8],
        serialize: impl FnMut(f::MaskJsonPart<'_>, &mut [u8]) -> Result<usize, f::MaskJsonRefusal>,
        cancelled: impl FnMut() -> bool,
    ) -> Result<usize, DeliveryRefusal<P::Error>> {
        let verified = self.verify_input(node, call, request, face_input, prepared)?;
        self.present_verified(node, call, request, verified, output, serialize, cancelled)
    }
    pub fn verify_input<'p, 'context>(
        &self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        face_input: &[u8],
        prepared: &'p PreparedPresentation<'context>,
    ) -> Result<VerifiedPresentation<'p, 'context>, DeliveryRefusal<P::Error>> {
        self.check_request(node, call, request, prepared)?;
        if !prepared.face.matches(face_input) {
            return Err(DeliveryRefusal::Face);
        }
        Ok(VerifiedPresentation { prepared })
    }
    fn check_request(
        &self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        prepared: &PreparedPresentation<'_>,
    ) -> Result<(), DeliveryRefusal<P::Error>> {
        use DeliveryRefusal as E;
        if node != self.node
            || call != HostCallId(0)
            || request != RequestId(self.ready.next_request)
            || request != prepared.request
            || prepared.occurrence.show().active_play_id != self.active.active_play_id
        {
            return Err(E::Binding);
        }
        if self.state != RendererState::Ready {
            return Err(E::Lifecycle);
        }
        Ok(())
    }
    /// The verified token carries no borrowed input slot. This can run inside
    /// the shared kernel's transactional output reservation without a Face copy.
    #[allow(clippy::too_many_arguments)]
    pub fn present_verified(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        verified: VerifiedPresentation<'_, '_>,
        output: &mut [u8],
        serialize: impl FnMut(f::MaskJsonPart<'_>, &mut [u8]) -> Result<usize, f::MaskJsonRefusal>,
        cancelled: impl FnMut() -> bool,
    ) -> Result<usize, DeliveryRefusal<P::Error>> {
        use DeliveryRefusal as E;
        let prepared = verified.prepared;
        self.check_request(node, call, request, prepared)?;
        let maximum = self.ready.bounds.show_bytes as usize;
        let bound = output.len().min(maximum);
        let length = prepared
            .occurrence
            .encode_json_with(&mut output[..bound], serialize)
            .map_err(E::Encoding)?;
        let next = self.ready.next_request.checked_add(1).ok_or(E::Lifecycle)?;
        let lease = self
            .table
            .authorize(&self.handle, &self.claim)
            .map_err(E::Capability)?;
        self.ready.next_request = next;
        self.state = RendererState::Failed;
        let pending = Pending {
            table: &mut self.table,
            handle: &mut self.handle,
            lease: Some(lease),
        };
        let packets = prepared.packets.packets().map(|bytes| Packet { bytes });
        let result = conduit_tm1637_wire::transmit(&mut self.ready.provider, &packets, cancelled);
        pending
            .finish(
                if result.is_ok() { length as u32 } else { 0 },
                result.is_err(),
            )
            .map_err(E::Capability)?;
        result.map_err(E::Electrical)?;
        self.state = RendererState::Available;
        Ok(length)
    }
    /// Explicit cleanup stays within the same admitted two-operation lifetime.
    /// A successful second transaction blanks/disables the latched module before
    /// capability revocation. Failure remains uncertain and cannot return Ready.
    pub fn withdraw(
        &mut self,
        cancelled: impl FnMut() -> bool,
    ) -> Result<(), DeliveryRefusal<P::Error>> {
        use DeliveryRefusal as E;
        if !matches!(
            self.state,
            RendererState::Available | RendererState::Interrupted
        ) {
            return Err(E::Lifecycle);
        }
        let batch = self
            .ready
            .packets
            .encode([0; 6], 0, false)
            .map_err(E::Packets)?;
        let lease = self
            .table
            .authorize(&self.handle, &self.claim)
            .map_err(E::Capability)?;
        self.state = RendererState::Failed;
        let pending = Pending {
            table: &mut self.table,
            handle: &mut self.handle,
            lease: Some(lease),
        };
        let slices = batch.packets();
        let packets = slices.map(|bytes| Packet { bytes });
        let result = conduit_tm1637_wire::transmit(&mut self.ready.provider, &packets, cancelled);
        pending.finish(0, true).map_err(E::Capability)?;
        result.map_err(E::Electrical)?;
        self.state = RendererState::Withdrawn;
        Ok(())
    }
    /// Owner-side terminal metadata after actual acknowledged blanking. Its
    /// bounded metadata transition belongs after Play, outside the no-allocation
    /// delivery step. This owner control value is not another Mask Fore result:
    /// its separately admitted output slice must cover the additional terminal
    /// sign. Failed withdrawal can never produce Closed bytes here.
    pub fn closed_encoded(
        &self,
        prepared: &mut PreparedPresentation<'_>,
        sign: c::SignId,
        output: &mut [u8],
        serialize: impl FnMut(f::MaskJsonPart<'_>, &mut [u8]) -> Result<usize, f::MaskJsonRefusal>,
    ) -> Result<usize, DeliveryRefusal<P::Error>> {
        if self.state != RendererState::Withdrawn
            || prepared.occurrence.show().active_play_id != self.active.active_play_id
            || prepared.request.0.checked_add(1) != Some(self.ready.next_request)
        {
            return Err(DeliveryRefusal::Lifecycle);
        }
        prepared
            .occurrence
            .transition(f::ManifestationLifecycle::Closed, sign)
            .map_err(DeliveryRefusal::Show)?;
        prepared
            .occurrence
            .encode_json_with(output, serialize)
            .map_err(DeliveryRefusal::Encoding)
    }
    pub fn cancel_before_delivery(&mut self) -> Result<(), DeliveryRefusal<P::Error>> {
        if self.state != RendererState::Ready {
            return Err(DeliveryRefusal::Lifecycle);
        }
        self.table
            .revoke(&self.handle)
            .map_err(DeliveryRefusal::Capability)?;
        self.state = RendererState::Cancelled;
        Ok(())
    }
    /// Refusal keeps custody intact. Failed cleanup cannot revive readiness.
    pub fn retire_retained(owner: &mut Option<Self>) -> Result<ReadyRenderer<P>, &'static str> {
        let current = owner.as_ref().ok_or("renderer-unavailable")?;
        if !matches!(
            current.state,
            RendererState::Withdrawn | RendererState::Cancelled
        ) || current.inspection().lifecycle != c::CapabilityLifecycle::Revoked
            || current.inspection().in_flight_operations != 0
        {
            return Err("renderer-not-retirable");
        }
        Ok(owner.take().expect("preflighted renderer").ready)
    }
}
struct Pending<'a> {
    table: &'a mut c::BaseCapabilityTable,
    handle: &'a mut c::BaseCapabilityHandle,
    lease: Option<c::BaseOperationLease>,
}
impl Pending<'_> {
    fn finish(mut self, bytes: u32, revoke: bool) -> Result<(), c::BaseCapabilityRefusal> {
        self.table.complete(
            self.handle,
            self.lease.as_ref().expect("pending").clone(),
            bytes,
        )?;
        if revoke {
            self.table.revoke(self.handle)?;
        }
        self.lease = None;
        Ok(())
    }
}
impl Drop for Pending<'_> {
    fn drop(&mut self) {
        if self.lease.is_some() {
            let _ = self.table.revoke(self.handle);
        }
    }
}
