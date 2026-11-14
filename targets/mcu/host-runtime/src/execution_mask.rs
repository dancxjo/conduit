//! Admitted renderer service: semantic Show and local output Fore, never GPIO.
//! Root drives this only after the complete local workset has Started.
extern crate alloc;
use alloc::vec;
#[path = "execution_mask_wire.rs"]
mod wire;
use conduit_core as c;
use conduit_environment_runtime::{
    display_mask::{PreparedDisplayMask, PreparedFrameEncoder, Tm1637Encoder},
    publication_face_custody::Reader,
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
/// FRAME is fixed local construction storage, not an incoming grant. Root
/// separately funds the bound service's inline frame cache and all preparation
/// overlap before Start; the selected encoder cannot enlarge this extent.
pub struct ReadyRenderer<
    C,
    E: PreparedFrameEncoder = Tm1637Encoder,
    const FRAME: usize = { conduit_tm1637_protocol::DISPLAY_FRAME_BYTES },
> {
    identity: Identity,
    clock: C,
    mask: PreparedDisplayMask<E>,
    bounds: Bounds,
    wire: wire::Storage,
}
impl<C: RendererClock> ReadyRenderer<C> {
    /// # Safety
    /// Same original custody and finite admission obligations as new_with_encoder.
    pub unsafe fn new(
        identity: Identity,
        clock: C,
        mask: PreparedDisplayMask,
        bounds: Bounds,
    ) -> Result<Self, Refusal> {
        // SAFETY: forwarded unchanged from this constructor's caller.
        unsafe { Self::new_with_encoder(identity, clock, mask, bounds) }
    }
}
impl<C: RendererClock, E: PreparedFrameEncoder, const FRAME: usize> ReadyRenderer<C, E, FRAME> {
    /// # Safety
    /// Root transfers the actual initialized clock slot and admits all owned
    /// storage/metadata/temporary overlap under this exact provider identity.
    /// Identity, Look and bounds come from checked construction, never a Plan.
    pub unsafe fn new_with_encoder(
        identity: Identity,
        clock: C,
        mask: PreparedDisplayMask<E>,
        bounds: Bounds,
    ) -> Result<Self, Refusal> {
        if FRAME == 0
            || mask.maximum_frame_bytes() > FRAME
            || identity.provider_generation == 0
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
    /// Exact selected canonical output extent, for checking the output Fore.
    pub fn maximum_frame_bytes(&self) -> usize {
        self.mask.maximum_frame_bytes()
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
    ) -> Result<RendererService<'a, C, E, FRAME>, Refusal> {
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
            last_frame: [0; FRAME],
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
#[path = "execution_mask_service.rs"]
mod service;
pub use service::{FrameTicket, OutputFrame, RendererService};

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
pub struct PreparedRendererService<
    'a,
    C,
    E: PreparedFrameEncoder = Tm1637Encoder,
    const FRAME: usize = { conduit_tm1637_protocol::DISPLAY_FRAME_BYTES },
>(RendererService<'a, C, E, FRAME>);
impl<'a, C: RendererClock, E: PreparedFrameEncoder, const FRAME: usize>
    PreparedRendererService<'a, C, E, FRAME>
{
    pub fn new(service: RendererService<'a, C, E, FRAME>) -> Self {
        Self(service)
    }
}
// SAFETY: binding moves only the already prepared original service and neither
// allocates nor performs an output. Whole-workset Start is its sole binder.
unsafe impl<'a, C: RendererClock, E: PreparedFrameEncoder, const FRAME: usize>
    super::execution_workset::PreparedLocalKernels for PreparedRendererService<'a, C, E, FRAME>
{
    type Bound = RendererService<'a, C, E, FRAME>;
    fn retained_backing_bytes(&self) -> Option<usize> {
        self.0.retained_backing_bytes()
    }
    #[inline(always)] // Fuse the affine owner transfer with its final destination.
    unsafe fn bind(self) -> Self::Bound {
        self.0
    }
}
