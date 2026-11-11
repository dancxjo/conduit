//! Admitted renderer service: semantic Show and local output Fore, never GPIO.
//! Root drives this only after the complete local workset has Started.
extern crate alloc;
use alloc::vec;
#[path = "execution_mask_wire.rs"]
mod wire;
use conduit_core as c;
use conduit_environment_runtime::{
    display_mask::PreparedDisplayMask, publication_face_custody::Reader,
};
use conduit_face as f;
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_seven_segment_mask::PreparedAlternatingShow;

pub const CAPABILITY: &str = "conduit/seven-segment-renderer@1";
// Both directions carry full objects using conduit_face::postcard_wire::ABI.
pub const CALL: &str = "conduit/seven-segment-present@2";
pub const PROFILE: &str = "conduit/seven-segment-local-fore@2";
pub const ATTACHMENT: &str = "face/local-renderer-storage@1";
pub const AUTHORITY: &str = "conduit.authority/seven-segment-present@1";
pub const TEE: &str = "mcu.mask/tee@1";
pub const READ_ONLY: &str = "mcu.mask/interaction-close@1";
pub const ROUTING_ARTIFACT: &str = "conduit-mcu-host-runtime/mask-routing@1";
pub const ROUTING_PROFILE: &str = "conduit.mcu/prepared-mask-routing@1";
pub type Identity = conduit_tm1637_wire::installation::WriteNativeIdentity;
/// The BSP owns a distinct admitted logical clock slot. This interface grants
/// no timer, scheduling, output, or permission to share an exclusive provider.
pub trait RendererClock {
    fn now_ms(&self) -> u64;
    /// Actual externally allocated provider storage, excluding inline Self.
    fn retained_backing_bytes(&self) -> Option<usize>;
}
#[derive(Clone, Copy)]
pub struct Bounds {
    pub face_bytes: u32,
    pub show_bytes: u32,
    pub resource_bytes: u64,
    pub maximum_frames: u32,
}
#[derive(Debug)]
pub enum Refusal {
    Scope,
    Bounds,
    Face,
    Clock,
    Pending,
    Encoding,
    Output,
    Capability(c::BaseCapabilityRefusal),
    AssignedMask(conduit_seven_segment_mask::native_kernel::AssignedMaskRefusal),
}
/// Immutable contract construction for checked Make/flash images. This is
/// deliberately separate from Ready projection and grants no provider custody.
pub fn capability_offer(
    implementation_id: c::ImplementationId,
    artifact_id: c::ArtifactId,
    bounds: Bounds,
) -> c::CapabilityOffer {
    let mut offer = f::renderer_offer(f::RendererRealizationOffer {
        capability_id: CAPABILITY.into(),
        execution_profile_id: PROFILE.into(),
        implementation_id,
        artifact_id,
        host_call: c::HostCallRequirement {
            contract_id: CALL.into(),
            target_kind: Some(f::RENDERER_KIND.into()),
            maximum_in_flight: 1,
            maximum_input_bytes: bounds.face_bytes,
            maximum_output_bytes: bounds.show_bytes,
        },
        resource_requirement: c::resource_requirement(ATTACHMENT, 1),
        limits: c::CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: bounds.face_bytes.max(bounds.show_bytes),
        },
    });
    offer.authority_requirements.push(c::AuthorityRequirement {
        contract_id: AUTHORITY.into(),
        host_call_contract_id: CALL.into(),
        subject_kind: f::RENDERER_KIND.into(),
    });
    offer
}
/// Pure contract images use the same finite queue ceilings as actual Ready
/// routing construction. These numbers describe support, never a resource grant.
pub fn routing_offers(
    face_bytes: u32,
    show_bytes: u32,
) -> Result<[c::CapabilityOffer; 2], Refusal> {
    if face_bytes == 0 || show_bytes == 0 || face_bytes > 8192 || show_bytes > 8192 {
        return Err(Refusal::Bounds);
    }
    let implementation = |id: &str| c::ImplementationOffer {
        execution_profile_id: ROUTING_PROFILE.into(),
        implementation_id: id.into(),
        artifact_id: ROUTING_ARTIFACT.into(),
    };
    let limits = |bytes| c::CapabilityLimits {
        max_active_instances: 1,
        max_queue_items: 1,
        max_queue_bytes: bytes,
    };
    Ok([
        f::face_tee_offer(TEE.into(), implementation(TEE), limits(face_bytes)),
        f::face_interaction_close_offer(
            READ_ONLY.into(),
            implementation(READ_ONLY),
            limits(face_bytes.max(show_bytes)),
        ),
    ])
}
pub struct ReadyRenderer<C> {
    identity: Identity,
    clock: C,
    mask: PreparedDisplayMask,
    bounds: Bounds,
    wire: wire::Storage,
}
impl<C: RendererClock> ReadyRenderer<C> {
    /// # Safety
    /// Root transfers the actual initialized clock slot and admits all owned
    /// storage/metadata/temporary overlap under this exact provider identity.
    /// Identity, Look and bounds come from checked construction, never a Plan.
    pub unsafe fn new(
        identity: Identity,
        clock: C,
        mask: PreparedDisplayMask,
        bounds: Bounds,
    ) -> Result<Self, Refusal> {
        if identity.provider_generation == 0
            || bounds.face_bytes == 0
            || bounds.face_bytes > 8192
            || bounds.show_bytes == 0
            || bounds.show_bytes > 8192
            || bounds.maximum_frames == 0
            || bounds.resource_bytes < u64::from(bounds.face_bytes) + u64::from(bounds.show_bytes)
        {
            return Err(Refusal::Bounds);
        }
        let ready = Self {
            identity,
            clock,
            mask,
            bounds,
            wire: wire::Storage::prepare(bounds.face_bytes, bounds.show_bytes)
                .map_err(|_| Refusal::Bounds)?,
        };
        if ready
            .retained_backing_bytes()
            .and_then(|n| u64::try_from(n).ok())
            .is_none_or(|n| n > bounds.resource_bytes)
        {
            return Err(Refusal::Bounds);
        }
        Ok(ready)
    }
    /// Actual owned heap payload. Inline Self, allocator overhead and separate
    /// preparation overlap remain charged by the enclosing whole-workset owner.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        let id = &self.identity;
        let mut bytes = self
            .wire
            .retained_bytes()
            .checked_add(self.mask.retained_backing_bytes())?
            .checked_add(self.clock.retained_backing_bytes()?)?;
        for text in [
            &id.host_id.0,
            &id.boot_id.0,
            &id.base_id.0,
            &id.provider_instance_id.0,
            &id.resource_pool_id.0,
            &id.resource_generation_id.0,
            &id.envelope_id.0,
            &id.artifact_id.0,
            &id.implementation_id.0,
            &id.base_implementation_id.0,
        ] {
            bytes = bytes.checked_add(text.capacity())?;
        }
        Some(bytes)
    }
    pub fn native_identity(&self) -> &Identity {
        &self.identity
    }
    pub fn bounds(&self) -> Bounds {
        self.bounds
    }
    /// Project only this initialized service; no publication data or effect.
    pub fn append_to_advertisement(&self, host: &mut c::HostAdvertisement) -> Result<(), Refusal> {
        if host.host_id != self.identity.host_id
            || host.boot_id != self.identity.boot_id
            || host.offer_generation.0 == 0
        {
            return Err(Refusal::Scope);
        }
        let id = &self.identity;
        let offer = capability_offer(
            id.implementation_id.clone(),
            id.artifact_id.clone(),
            self.bounds,
        );
        let mut registry = c::BaseRegistry::new(c::BaseRegistryLimits {
            maximum_bases: 1,
            maximum_capabilities_per_base: 1,
            maximum_resources_per_base: 1,
            maximum_advertised_capabilities: u16::try_from(host.capabilities.len() + 1)
                .map_err(|_| Refusal::Bounds)?,
            maximum_advertised_resources: u16::try_from(host.resources.len() + 1)
                .map_err(|_| Refusal::Bounds)?,
        })
        .map_err(|_| Refusal::Bounds)?;
        registry
            .register(c::BaseProviderEntry {
                base_id: id.base_id.clone(),
                provider_instance_id: id.provider_instance_id.clone(),
                provider_generation: id.provider_generation,
                implementation_id: id.base_implementation_id.clone(),
                mechanism_family: ATTACHMENT.into(),
                enforcement_class: c::BaseEnforcementClass::Cooperative,
                lifecycle: c::BaseLifecycle::Ready,
                capabilities: vec![offer],
                resources: vec![c::resource_offer(
                    id.resource_pool_id.as_str(),
                    ATTACHMENT,
                    1,
                )],
            })
            .map_err(|_| Refusal::Scope)?;
        registry
            .project_ready_into(host)
            .map_err(|_| Refusal::Scope)
    }
    /// Prepare the original native ticket and real local Reader before Start.
    /// # Safety
    /// Root has checked exact assigned renderer binding, original table/claim,
    /// whole-workset resources and Source-selected local Face/output adapters.
    /// The returned service must not be polled until that whole workset Starts.
    #[allow(clippy::too_many_arguments)] // Exact admitted owners and finite buffers remain explicit.
    pub unsafe fn bind<'a>(
        &'a mut self,
        reader: Reader<'a>,
        context: f::PreparedRendererContext,
        active: c::ActivePlayIdentity,
        node: NodeId,
        table: c::BaseCapabilityTable,
        handle: c::BaseCapabilityHandle,
        claim: c::BaseOperationClaim,
    ) -> Result<RendererService<'a, C>, Refusal> {
        let id = &self.identity;
        if active.host_id != id.host_id
            || active.boot_id != id.boot_id
            || claim.host_id != id.host_id
            || claim.boot_id != id.boot_id
            || claim.base_instance_id != id.provider_instance_id
            || claim.base_provider_generation != id.provider_generation
            || claim.resource_pool_id != id.resource_pool_id
            || claim.resource_generation_id != id.resource_generation_id
            || claim.envelope_id != id.envelope_id
            || claim.implementation_id != id.implementation_id
        {
            return Err(Refusal::Scope);
        }
        let operation = c::BoundBaseOperation::bind(table, handle, claim)
            .map_err(|(_, _, _, reason)| Refusal::Capability(reason))?;
        let service = RendererService {
            ready: self,
            reader,
            context,
            active,
            node,
            operation,
            next_request: 0,
            pending: None,
            staged: None,
            face_input_len: 0,
            retained: None,
            last_clock: 0,
            frames: 0,
            frame_pending: false,
            last_frame: [0; conduit_tm1637_protocol::DISPLAY_FRAME_BYTES],
            last_frame_len: 0,
        };
        if service
            .retained_backing_bytes()
            .and_then(|n| u64::try_from(n).ok())
            .is_none_or(|n| n > service.ready.bounds.resource_bytes)
        {
            return Err(Refusal::Bounds);
        }
        Ok(service)
    }
}
struct Staged {
    show: f::Show,
    pages: PreparedAlternatingShow,
}
struct Pending {
    request: RequestId,
    lease: c::BaseOperationLease,
    show: f::Show,
    pages: PreparedAlternatingShow,
    started: u64,
}
struct Retained {
    show: f::Show,
    pages: PreparedAlternatingShow,
    started: u64,
}
/// Monotonic completion identity within one renderer service. Root pairs it
/// with the exact output Play/Host Call; a prior frame cannot acknowledge a
/// later pending frame even when its bytes happen to be identical.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameTicket(u32);
pub struct OutputFrame<'a> {
    pub ticket: FrameTicket,
    pub canonical: &'a [u8],
}
pub struct RendererService<'a, C> {
    ready: &'a mut ReadyRenderer<C>,
    reader: Reader<'a>,
    context: f::PreparedRendererContext,
    active: c::ActivePlayIdentity,
    node: NodeId,
    operation: c::BoundBaseOperation,
    next_request: u32,
    pending: Option<Pending>,
    staged: Option<Staged>,
    face_input_len: usize,
    retained: Option<Retained>,
    last_clock: u64,
    frames: u32,
    frame_pending: bool,
    last_frame: [u8; conduit_tm1637_protocol::DISPLAY_FRAME_BYTES],
    last_frame_len: usize,
}
impl<C: RendererClock> RendererService<'_, C> {
    /// Includes simultaneous pending/retained Shows; the shared Face Reader's
    /// publication owner is counted once by the enclosing group, not here.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        let mut bytes = self
            .ready
            .retained_backing_bytes()?
            .checked_add(self.context.retained_backing_bytes()?)?
            .checked_add(self.operation.retained_backing_bytes()?)?;
        for text in [
            &self.active.active_play_id.0,
            &self.active.plan_id.0,
            &self.active.host_id.0,
            &self.active.boot_id.0,
        ] {
            bytes = bytes.checked_add(text.capacity())?;
        }
        if let Some(staged) = &self.staged {
            bytes = bytes.checked_add(staged.show.retained_backing_bytes()?)?;
        }
        if let Some(pending) = &self.pending {
            bytes = bytes.checked_add(pending.show.retained_backing_bytes()?)?;
        }
        if let Some(retained) = &self.retained {
            bytes = bytes.checked_add(retained.show.retained_backing_bytes()?)?;
        }
        Some(bytes)
    }
    /// Freeze the exact serialized Face and prepared pages before placing its
    /// Value on the Mask Fore. A later publication cannot substitute a different
    /// revision while the admitted kernel advances to its renderer Host Call.
    /// All cold preparation and transient overlap remain funded by Root.
    pub fn stage_face(&mut self, prepared_sign: c::SignId) -> Result<&[u8], Refusal> {
        if self.pending.is_some() || self.staged.is_some() || self.frame_pending {
            return Err(Refusal::Pending);
        }
        let ready = &mut self.ready;
        let context = &self.context;
        let active = &self.active;
        let (show, pages, length) = self
            .reader
            .with_retained(|face, _| {
                let bytes = f::postcard_wire::encode_face(face, ready.wire.face_mut())
                    .map_err(|_| Refusal::Encoding)?
                    .len();
                let pages = ready.mask.prepare_show(face).map_err(|_| Refusal::Face)?;
                let show = context
                    .prepared_show(
                        face,
                        active.clone(),
                        "environment".into(),
                        "local-segmented-display".into(),
                        prepared_sign,
                    )
                    .map_err(|_| Refusal::Face)?;
                Ok((show, pages, bytes))
            })
            .map_err(|_| Refusal::Face)??;
        // The enclosing Root separately funds transient construction overlap;
        // retained old/new Shows must both fit this exact installed ceiling.
        let retained = self
            .retained_backing_bytes()
            .and_then(|n| n.checked_add(show.retained_backing_bytes()?))
            .and_then(|n| u64::try_from(n).ok());
        if retained.is_none_or(|n| n > self.ready.bounds.resource_bytes) {
            return Err(Refusal::Bounds);
        }
        self.staged = Some(Staged { show, pages });
        self.face_input_len = length;
        Ok(&self.ready.wire.bytes()[..length])
    }
    /// Cold per-Face preparation under the enclosing admitted preparation owner.
    /// It validates the exact kernel input; it performs no output or clock wait.
    pub fn prepare_request(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
        prepared_sign: c::SignId,
    ) -> Result<(), Refusal> {
        if node != self.node
            || call != HostCallId(0)
            || request != RequestId(self.next_request)
            || self.pending.is_some()
            || self.frame_pending
        {
            return Err(Refusal::Scope);
        }
        let next = self.next_request.checked_add(1).ok_or(Refusal::Bounds)?;
        let now = self.ready.clock.now_ms();
        if now < self.last_clock {
            return Err(Refusal::Clock);
        }
        if self.staged.is_none() {
            self.stage_face(prepared_sign)?;
        }
        if input != &self.ready.wire.bytes()[..self.face_input_len] {
            return Err(Refusal::Face);
        }
        let Staged { show, pages } = self.staged.take().ok_or(Refusal::Pending)?;
        let lease = self.operation.authorize().map_err(Refusal::Capability)?;
        self.next_request = next;
        self.last_clock = now;
        self.pending = Some(Pending {
            request,
            lease,
            show,
            pages,
            started: now,
        });
        Ok(())
    }
    /// Frame Info must be enqueued through the exact admitted output Plot Fore.
    /// Repeated polling cannot create another frame until Root acknowledges it.
    pub fn frame(&mut self) -> Result<Option<OutputFrame<'_>>, Refusal> {
        if self.frame_pending {
            return Ok(None);
        }
        if self.frames == self.ready.bounds.maximum_frames {
            return Err(Refusal::Bounds);
        }
        let now = self.ready.clock.now_ms();
        if now < self.last_clock {
            return Err(Refusal::Clock);
        }
        let (pages, started) = if let Some(pending) = &self.pending {
            (&pending.pages, pending.started)
        } else if let Some(retained) = &self.retained {
            (&retained.pages, retained.started)
        } else {
            return Ok(None);
        };
        self.last_clock = now;
        let frame = self
            .ready
            .mask
            .render_prepared(pages, now.checked_sub(started).ok_or(Refusal::Clock)?)
            .map_err(|_| Refusal::Face)?;
        if self.pending.is_none() && frame.canonical == &self.last_frame[..self.last_frame_len] {
            return Ok(None);
        }
        if frame.canonical.len() > self.last_frame.len() {
            return Err(Refusal::Bounds);
        }
        self.last_frame[..frame.canonical.len()].copy_from_slice(frame.canonical);
        self.last_frame_len = frame.canonical.len();
        self.frame_pending = true;
        self.frames += 1;
        Ok(Some(OutputFrame {
            ticket: FrameTicket(self.frames),
            canonical: frame.canonical,
        }))
    }
    /// # Safety
    /// Root observed completion of this exact pending frame by the admitted
    /// output fragment's original electrical Host Call. Transport delivery alone
    /// is insufficient. Refusal/cancellation must call revoke instead.
    pub unsafe fn acknowledge_frame(
        &mut self,
        ticket: FrameTicket,
        available_sign: c::SignId,
    ) -> Result<Option<(RequestId, &[u8])>, Refusal> {
        if !self.frame_pending || ticket != FrameTicket(self.frames) {
            return Err(Refusal::Pending);
        }
        self.frame_pending = false;
        let Some(pending) = self.pending.take() else {
            return Ok(None);
        };
        let prepared = (|| {
            let show = pending
                .show
                .into_transition(f::ManifestationLifecycle::Available, available_sign)
                .map_err(|_| Refusal::Output)?;
            let length = f::postcard_wire::encode_show(&show, self.ready.wire.show_mut())
                .map_err(|_| Refusal::Encoding)?
                .len();
            Ok((show, length))
        })();
        let (show, length) = match prepared {
            Ok(value) => value,
            Err(error) => {
                let _ = self.operation.revoke();
                self.retained = None;
                return Err(error);
            }
        };
        if let Err(error) = self.operation.complete(pending.lease, length as u32) {
            let _ = self.operation.revoke();
            self.retained = None;
            return Err(Refusal::Capability(error));
        }
        self.retained = Some(Retained {
            show,
            pages: pending.pages,
            started: pending.started,
        });
        Ok(Some((pending.request, &self.ready.wire.bytes()[..length])))
    }
    pub fn face_changed(&self) -> Result<bool, Refusal> {
        self.reader
            .with_retained(|face, status| {
                status.source_sequence > 0
                    && self
                        .retained
                        .as_ref()
                        .is_none_or(|old| old.show.face_id != face.identity)
            })
            .map_err(|_| Refusal::Face)
    }
    pub fn maximum_show_bytes(&self) -> u32 {
        self.ready.bounds.show_bytes
    }
    pub fn current_show(&self) -> Option<&f::Show> {
        self.retained.as_ref().map(|value| &value.show)
    }
    pub fn revoke(&mut self) -> Result<(), Refusal> {
        self.frame_pending = false;
        self.pending = None;
        self.staged = None;
        self.face_input_len = 0;
        self.retained = None;
        self.operation.revoke().map_err(Refusal::Capability)
    }
}

impl<C> Drop for RendererService<'_, C> {
    fn drop(&mut self) {
        let _ = self.operation.revoke();
        self.ready.wire.clear();
    }
}

/// Actual initialized pure routing Backs. No capability advertisement can
/// substitute for their affine ownership. The renderer service is separate.
pub struct ReadyMaskRouting {
    roles: [conduit_seven_segment_mask::native_kernel::MaskRole; 3],
    backs: Option<[conduit_seven_segment_mask::native_kernel::MaskBack; 3]>,
    face_bytes: u32,
    show_bytes: u32,
}
pub struct PreparedMaskRouting<'a> {
    owner: &'a mut ReadyMaskRouting,
    storage: conduit_seven_segment_mask::native_kernel::PreparedAssignedMaskStorage,
    backing_bytes: usize,
}
impl ReadyMaskRouting {
    pub fn new(
        roles: [conduit_seven_segment_mask::native_kernel::MaskRole; 3],
        face_bytes: u32,
        show_bytes: u32,
    ) -> Result<Self, Refusal> {
        use conduit_seven_segment_mask::native_kernel::{MaskBack, MaskRole};
        if show_bytes == 0 || show_bytes > 8192 {
            return Err(Refusal::Bounds);
        }
        if face_bytes == 0
            || face_bytes > 8192
            || roles.iter().filter(|r| **r == MaskRole::Tee).count() != 1
            || roles.iter().filter(|r| **r == MaskRole::Renderer).count() != 1
            || roles.iter().filter(|r| **r == MaskRole::ReadOnly).count() != 1
        {
            return Err(Refusal::Bounds);
        }
        let backs = roles.map(|role| match role {
            MaskRole::Tee => MaskBack::Tee,
            MaskRole::Renderer => MaskBack::Renderer {
                state: 0,
                maximum_input_bytes: face_bytes,
                next_request: 0,
            },
            MaskRole::ReadOnly => MaskBack::ReadOnly { face: false },
        });
        Ok(Self {
            roles,
            backs: Some(backs),
            face_bytes,
            show_bytes,
        })
    }
    /// Actual Ready pure slots; renderer identity is supplied independently by
    /// the retained ReadyRenderer. No descriptor can replace a missing owner.
    pub fn routing_identities(
        &self,
    ) -> [Option<conduit_plan_lowering::resident_inventory::OtherNodeIdentity>; 3] {
        use conduit_seven_segment_mask::native_kernel::MaskRole;
        core::array::from_fn(|node| {
            self.backs.as_ref()?;
            let (kind, implementation) = match self.roles[node] {
                MaskRole::Tee => (f::FACE_TEE_KIND, TEE),
                MaskRole::ReadOnly => (f::FACE_INTERACTION_KIND, READ_ONLY),
                MaskRole::Renderer => return None,
            };
            Some(
                conduit_plan_lowering::resident_inventory::OtherNodeIdentity {
                    node: node as u16,
                    kind: c::AssignedIdentity::from_text(kind),
                    implementation: c::AssignedIdentity::from_text(implementation),
                    artifact: c::AssignedIdentity::from_text(ROUTING_ARTIFACT),
                },
            )
        })
    }
    /// Borrows actual Back owners throughout preparation. Dropping the ticket
    /// before authenticated Start preserves those owners for another attempt.
    pub fn prepare<'a>(
        &'a mut self,
        input: conduit_seven_segment_mask::native_kernel::AssignedMaskInput<'_>,
        profile: &conduit_seven_segment_mask::native_kernel::AssignedMaskProfile<'_>,
        scope: conduit_seven_segment_mask::native_kernel::AssignedMaskScope,
    ) -> Result<PreparedMaskRouting<'a>, Refusal> {
        if self.backs.is_none()
            || self.roles != profile.roles
            || self.face_bytes != profile.face_bytes
            || self.show_bytes != profile.show_bytes
        {
            return Err(Refusal::Scope);
        }
        let backing_bytes = conduit_seven_segment_mask::native_kernel::PreparedAssignedMaskKernel::required_heap_bytes(profile)
            .map_err(|_| Refusal::Bounds)?;
        let storage =
            conduit_seven_segment_mask::native_kernel::PreparedAssignedMaskKernel::prepare_storage(
                input, profile, scope,
            )
            .map_err(Refusal::AssignedMask)?;
        Ok(PreparedMaskRouting {
            owner: self,
            storage,
            backing_bytes,
        })
    }
}
impl PreparedMaskRouting<'_> {
    /// Exact boxed scheduler and canonical Value slots. Inline ticket/Back
    /// owners and allocator overhead are separately charged by the group.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        Some(self.backing_bytes)
    }

    /// # Safety
    /// Root has admitted the complete workset and authenticated its current
    /// Start. This transfers the original initialized Backs once, without any
    /// allocation, validation failure, or native effect.
    #[inline(always)] // Fuse the affine owner transfer with its final destination.
    pub unsafe fn bind(
        self,
    ) -> conduit_seven_segment_mask::native_kernel::PreparedAssignedMaskKernel {
        self.storage
            .bind(self.owner.backs.take().expect("affine prepared Mask owner"))
    }
}

/// Actual prepared renderer service held inert by whole-workset custody.
pub struct PreparedRendererService<'a, C>(RendererService<'a, C>);
impl<'a, C: RendererClock> PreparedRendererService<'a, C> {
    pub fn new(service: RendererService<'a, C>) -> Self {
        Self(service)
    }
}
// SAFETY: binding moves only the already prepared original service and neither
// allocates nor performs an output. Whole-workset Start is its sole binder.
unsafe impl<'a, C: RendererClock> super::execution_workset::PreparedLocalKernels
    for PreparedRendererService<'a, C>
{
    type Bound = RendererService<'a, C>;
    fn retained_backing_bytes(&self) -> Option<usize> {
        self.0.retained_backing_bytes()
    }
    #[inline(always)] // Fuse the affine owner transfer with its final destination.
    unsafe fn bind(self) -> Self::Bound {
        self.0
    }
}
