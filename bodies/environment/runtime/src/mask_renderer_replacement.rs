//! One affine successor, never a second physical display owner. The predecessor
//! retains its cleanup operation until the successor's complete frame is ACKed.
use super::*;
use delivery::PreparedPresentation;
/// Available metadata only: drops copied Face bytes and encoded packets as soon
/// as acknowledged delivery finishes, before preparing the next frame.
pub struct PresentedOccurrence<'a> {
    occurrence: f::PreparedMaskOccurrence<'a>,
    request: RequestId,
}

#[cfg(target_has_atomic = "ptr")]
impl<'a> PreparedPresentation<'a> {
    /// Pin exactly the immutable context already borrowed by this preparation.
    #[allow(clippy::result_large_err)]
    pub fn retain_context(
        self,
        context: alloc::sync::Arc<f::PreparedMaskShowContext>,
    ) -> Result<PreparedPresentation<'static>, (Self, alloc::sync::Arc<f::PreparedMaskShowContext>)>
    {
        let Self {
            occurrence,
            face,
            packets,
            request,
        } = self;
        match occurrence.retain_context(context) {
            Ok(occurrence) => Ok(PreparedPresentation {
                occurrence,
                face,
                packets,
                request,
            }),
            Err((occurrence, context)) => Err((
                Self {
                    occurrence,
                    face,
                    packets,
                    request,
                },
                context,
            )),
        }
    }
}

#[cfg(target_has_atomic = "ptr")]
impl<'a> PresentedOccurrence<'a> {
    /// Retain the original immutable prepared context without copying the Show.
    /// Exact pointer mismatch returns both affine inputs unchanged.
    #[allow(clippy::result_large_err)]
    pub fn retain_context(
        self,
        context: alloc::sync::Arc<f::PreparedMaskShowContext>,
    ) -> Result<PresentedOccurrence<'static>, (Self, alloc::sync::Arc<f::PreparedMaskShowContext>)>
    {
        let Self {
            occurrence,
            request,
        } = self;
        match occurrence.retain_context(context) {
            Ok(occurrence) => Ok(PresentedOccurrence {
                occurrence,
                request,
            }),
            Err((occurrence, context)) => Err((
                Self {
                    occurrence,
                    request,
                },
                context,
            )),
        }
    }
}

type Lease = (
    c::ActivePlayIdentity,
    c::BaseCapabilityTable,
    c::BaseCapabilityHandle,
    c::BaseOperationClaim,
);
/// Root-issued capability staged against the exact retained invocation. No pin
/// owner is transferred and no electrical work occurs during construction.
pub struct ReplacementLease {
    lease: Option<Lease>,
    predecessor: c::ActivePlayIdentity,
    request: RequestId,
    spent: bool,
}
/// Produced only after successful successor delivery. Owns the actual revoked
/// predecessor table until its distinct Replaced evidence has been accepted.
pub struct ReplacedRenderer {
    table: c::BaseCapabilityTable,
    active: c::ActivePlayIdentity,
    request: RequestId,
}
impl ReplacementLease {
    pub fn active(&self) -> Option<&c::ActivePlayIdentity> {
        self.lease.as_ref().map(|lease| &lease.0)
    }
}
impl ReplacedRenderer {
    pub fn active(&self) -> &c::ActivePlayIdentity {
        &self.active
    }
    pub fn inspection(&self) -> &c::CapabilityInspection {
        self.table
            .inspections()
            .next()
            .expect("single predecessor capability")
    }
    pub fn terminal_encoded<E>(
        &self,
        prepared: &mut PresentedOccurrence<'_>,
        sign: c::SignId,
        output: &mut [u8],
        serialize: impl FnMut(f::MaskJsonPart<'_>, &mut [u8]) -> Result<usize, f::MaskJsonRefusal>,
    ) -> Result<usize, DeliveryRefusal<E>> {
        if prepared.occurrence.show().active_play_id != self.active.active_play_id
            || prepared.request != self.request
            || self.inspection().lifecycle != c::CapabilityLifecycle::Revoked
            || self.inspection().in_flight_operations != 0
            || self.inspection().completed_operations != 1
        {
            return Err(DeliveryRefusal::Lifecycle);
        }
        prepared
            .occurrence
            .transition(f::ManifestationLifecycle::Replaced, sign)
            .map_err(DeliveryRefusal::Show)?;
        prepared
            .occurrence
            .encode_json_with(output, serialize)
            .map_err(DeliveryRefusal::Encoding)
    }
}
impl PreparedRendererExecution {
    /// Refusal returns every actual incoming capability owner unchanged.
    #[allow(clippy::result_large_err)]
    pub fn stage_replacement<P: OpenDrain>(
        &self,
        current: &RendererCall<P>,
        lease: Lease,
    ) -> Result<ReplacementLease, (Lease, &'static str)> {
        let check = || -> Result<(), &'static str> {
            let (active, table, _, claim) = &lease;
            if !self.bound
                || !self.matches_last(&current.active)
                || current.state != RendererState::Available
                || !self.matches_owner(&current.ready)
                || current.node != self.node
                || current.inspection().completed_operations != 1
                || current.inspection().in_flight_operations != 0
                || current.inspection().lifecycle != c::CapabilityLifecycle::Issued
                || !self.matches_parent(active)
                || active.play_sequence <= current.active.play_sequence
                || c::bind_active_play(
                    &active.plan_id,
                    &active.host_id,
                    &active.boot_id,
                    active.play_sequence,
                ) != *active
                || current.ready.next_request == u32::MAX
            {
                return Err("renderer-replacement-owner");
            }
            let mut inspections = table.inspections();
            let next = inspections
                .next()
                .ok_or("renderer-replacement-possession")?;
            if inspections.next().is_some()
                || next.lifecycle != c::CapabilityLifecycle::Issued
                || next.completed_operations != 0
                || next.in_flight_operations != 0
            {
                return Err("renderer-replacement-possession");
            }
            let scope = &next.scope;
            if !self.matches_scope(scope)
                || scope.active_play_id != active.active_play_id
                || claim.host_id != scope.host_id
                || claim.boot_id != scope.boot_id
                || claim.plan_id != scope.plan_id
                || claim.active_play_id != scope.active_play_id
                || claim.base_instance_id != scope.base_instance_id
                || claim.base_provider_generation != scope.base_provider_generation
                || claim.implementation_id != scope.implementation_id
                || claim.operation_contract_id != scope.operation_contract_id
                || claim.subject_kind != scope.subject_kind
                || claim.resource_pool_id != scope.resource_pool_id
                || claim.resource_generation_id != scope.resource_generation_id
                || claim.envelope_id != scope.envelope_id
                || claim.parameter_bytes != self.bounds.face_bytes
                || claim.work_units != 1
            {
                return Err("renderer-replacement-scope");
            }
            Ok(())
        };
        if let Err(reason) = check() {
            return Err((lease, reason));
        }
        Ok(ReplacementLease {
            lease: Some(lease),
            predecessor: current.active.clone(),
            request: RequestId(current.ready.next_request),
            spent: false,
        })
    }
    /// Exact Face bytes are checked before output encoding or any electrical
    /// effect. A failed successor keeps the old call and its cleanup budget.
    #[allow(clippy::too_many_arguments)]
    pub fn replace_encoded<P: OpenDrain>(
        &mut self,
        current: &mut RendererCall<P>,
        successor: &mut ReplacementLease,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
        prepared: &PreparedPresentation<'_>,
        output: &mut [u8],
        serialize: impl FnMut(f::MaskJsonPart<'_>, &mut [u8]) -> Result<usize, f::MaskJsonRefusal>,
        cancelled: impl FnMut() -> bool,
    ) -> Result<(usize, ReplacedRenderer), DeliveryRefusal<P::Error>> {
        let verified =
            current.verify_replacement(successor, node, call, request, input, prepared)?;
        self.replace_verified(
            current, successor, node, call, request, verified, output, serialize, cancelled,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn replace_verified<P: OpenDrain>(
        &mut self,
        current: &mut RendererCall<P>,
        successor: &mut ReplacementLease,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        verified: VerifiedPresentation<'_, '_>,
        output: &mut [u8],
        serialize: impl FnMut(f::MaskJsonPart<'_>, &mut [u8]) -> Result<usize, f::MaskJsonRefusal>,
        cancelled: impl FnMut() -> bool,
    ) -> Result<(usize, ReplacedRenderer), DeliveryRefusal<P::Error>> {
        let prepared = verified.prepared;
        use DeliveryRefusal as E;
        if !self.bound
            || !self.matches_last(&current.active)
            || successor.predecessor != current.active
            || successor.spent
            || current.state != RendererState::Available
            || node != current.node
            || call != HostCallId(0)
            || request != successor.request
            || request != prepared.request
            || request != RequestId(current.ready.next_request)
        {
            return Err(E::Binding);
        }
        let (active, table, handle, claim) = successor.lease.as_mut().ok_or(E::Lifecycle)?;
        if prepared.occurrence.show().active_play_id != active.active_play_id {
            return Err(E::Binding);
        }
        let bound = output.len().min(current.ready.bounds.show_bytes as usize);
        let length = prepared
            .occurrence
            .encode_json_with(&mut output[..bound], serialize)
            .map_err(E::Encoding)?;
        let operation = table.authorize(handle, claim).map_err(E::Capability)?;
        successor.spent = true;
        current.state = RendererState::Interrupted;
        let result = conduit_tm1637_wire::transmit(
            &mut current.ready.provider,
            &prepared.packets.packets().map(|bytes| Packet { bytes }),
            cancelled,
        );
        table
            .complete(
                handle,
                operation,
                if result.is_ok() { length as u32 } else { 0 },
            )
            .map_err(E::Capability)?;
        if let Err(error) = result {
            table.revoke(handle).map_err(E::Capability)?;
            current.state = RendererState::Interrupted;
            return Err(E::Electrical(error));
        }
        // The old capability remains usable for cleanup up to this ACK. It has
        // exactly one completed operation; unused cleanup is revoked, not spent.
        current
            .table
            .revoke(&current.handle)
            .map_err(E::Capability)?;
        let (active, table, handle, claim) = successor.lease.take().expect("checked successor");
        let receipt = ReplacedRenderer {
            table: core::mem::replace(&mut current.table, table),
            active: core::mem::replace(&mut current.active, active.clone()),
            request: RequestId(current.ready.next_request - 1),
        };
        current.handle = handle;
        current.claim = claim;
        current.ready.next_request += 1;
        current.state = RendererState::Available;
        self.record_last(&active);
        Ok((length, receipt))
    }
}
impl<P: OpenDrain> RendererCall<P> {
    #[allow(clippy::too_many_arguments)]
    pub fn verify_replacement<'p, 'context>(
        &self,
        successor: &ReplacementLease,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
        prepared: &'p PreparedPresentation<'context>,
    ) -> Result<VerifiedPresentation<'p, 'context>, DeliveryRefusal<P::Error>> {
        if self.state != RendererState::Available
            || successor.spent
            || successor.predecessor != self.active
            || node != self.node
            || call != HostCallId(0)
            || request != successor.request
            || request != prepared.request
            || request != RequestId(self.ready.next_request)
            || successor.active().map(|active| &active.active_play_id)
                != Some(&prepared.occurrence.show().active_play_id)
        {
            return Err(DeliveryRefusal::Binding);
        }
        if !prepared.face.matches(input) {
            return Err(DeliveryRefusal::Face);
        }
        Ok(VerifiedPresentation { prepared })
    }
    /// The actual available call consumes its own presentation, releasing the
    /// Face copy and packet staging while preserving exact lifecycle metadata.
    #[allow(clippy::result_large_err)] // Refusal preserves the exact prepared owner.
    pub fn retain_presentation<'a>(
        &self,
        prepared: PreparedPresentation<'a>,
    ) -> Result<PresentedOccurrence<'a>, PreparedPresentation<'a>> {
        if self.state != RendererState::Available
            || prepared.occurrence.show().active_play_id != self.active.active_play_id
            || prepared.request.0.checked_add(1) != Some(self.ready.next_request)
        {
            return Err(prepared);
        }
        Ok(PresentedOccurrence {
            occurrence: prepared.occurrence,
            request: prepared.request,
        })
    }
    pub fn closed_presented_encoded(
        &self,
        prepared: &mut PresentedOccurrence<'_>,
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
    pub fn retained_identity(&self) -> &RendererIdentity {
        &self.ready.identity
    }
    pub fn prepare_replacement<'a>(
        &mut self,
        successor: &ReplacementLease,
        input: PresentationBasis<'_, 'a>,
    ) -> Result<PreparedPresentation<'a>, DeliveryRefusal<P::Error>> {
        if self.state != RendererState::Available
            || successor.spent
            || successor.predecessor != self.active
            || successor.request != RequestId(self.ready.next_request)
        {
            return Err(DeliveryRefusal::Lifecycle);
        }
        let active = &successor
            .lease
            .as_ref()
            .ok_or(DeliveryRefusal::Lifecycle)?
            .0;
        self.prepare_for(active.clone(), input)
    }
}
