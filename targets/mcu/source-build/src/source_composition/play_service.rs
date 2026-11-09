// Actual semantic renderer preparation and the original whole-workset Start.
// No GPIO path exists here: every frame goes through the admitted output Plot.
use alloc::{format, vec::Vec};
use conduit_body as b;
use conduit_core as c;
use conduit_environment_runtime::publication_face_custody::{LocalPublicationFace, Reader};
use conduit_face::{AssignedRendererSource, FaceBasis, PreparedRendererContext};
use conduit_mcu_host_runtime::{
    execution::{Policy, Refusal},
    execution_authority::{Issuer, NativeFacts, Operation},
    execution_mask::{PreparedRendererService, ReadyRenderer, RendererClock, RendererService},
    execution_workset::{PreparedLocalKernels, PreparedLocalWorkset, StartedLocalWorkset},
};

mod presentation {
    include!(concat!(env!("OUT_DIR"), "/source_play_presentation.rs"));
}

pub struct Scopes {
    pub body: b::BodyId,
    pub active: c::ActivePlayIdentity,
    pub fragments: [c::FragmentId; 3],
}
impl Scopes {
    pub fn fragment(&self, index: usize) -> conduit_mcu_host_runtime::kernel_sign::Scope<'_> {
        conduit_mcu_host_runtime::kernel_sign::Scope {
            body: &self.body,
            active: &self.active,
            fragment: &self.fragments[index],
        }
    }
    fn backing_bytes(&self) -> usize {
        self.body.retained_backing_bytes()
            + self.active.active_play_id.0.capacity()
            + self.active.plan_id.0.capacity()
            + self.active.host_id.0.capacity()
            + self.active.boot_id.0.capacity()
            + self.fragments.iter().map(|f| f.0.capacity()).sum::<usize>()
    }
}
/// The Source factory owns all actual resident/native tickets before this call.
/// The five pool identities are captured from that BSP's initialized owners,
/// including its finite private-work pool. Incoming descriptors choose none.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
#[inline(always)] // The separate Start poll owns its affine return storage.
pub fn start<'a, 'w, T: PreparedLocalKernels, C: RendererClock>(
    prepared: PreparedLocalWorkset<'w, T, 3>,
    workspace: &b::HostCommandInput<{ crate::config::MAX_ASSIGNMENT_BYTES }>,
    bootstrap: &b::MemberBootstrap<'_>,
    member: &b::MembershipCredential,
    configuration: &conduit_mcu_host_runtime::root::Configuration,
    resources: &mut c::ResourceAdmissionOwner,
    policy: Policy<'_>,
    renderer: &'a mut ReadyRenderer<C>,
    issuer: &mut Issuer,
    operation: &Operation<'_>,
    reader: Reader<'a>,
    face: &LocalPublicationFace,
    held_pools: &[c::AssignedIdentity],
    recording_service_bytes: usize,
) -> Result<
    (
        StartedLocalWorkset<(T::Bound, RendererService<'a, C>)>,
        Scopes,
    ),
    Refusal<&'static str>,
> {
    start_selected(
        prepared,
        Some(workspace),
        PresentationOrigin::Online {
            bootstrap,
            member,
            configuration,
        },
        resources,
        policy,
        renderer,
        issuer,
        operation,
        reader,
        face,
        held_pools,
        recording_service_bytes,
    )
}

/// Same complete renderer/admission path with the actual checked installed
/// owner-intent and fresh local scope; no online command or member stamp exists.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
#[inline(always)]
pub fn start_continuation<'a, 'w, T: PreparedLocalKernels, C: RendererClock>(
    prepared: PreparedLocalWorkset<'w, T, 3>,
    bootstrap: &b::MemberBootstrap<'_>,
    verified: &b::VerifiedContinuationIntent<'_>,
    configuration: &conduit_mcu_host_runtime::root::Configuration,
    scope: &b::FreshContinuationScope<'w>,
    resources: &mut c::ResourceAdmissionOwner,
    policy: Policy<'_>,
    renderer: &'a mut ReadyRenderer<C>,
    issuer: &mut Issuer,
    operation: &Operation<'_>,
    reader: Reader<'a>,
    face: &LocalPublicationFace,
    held_pools: &[c::AssignedIdentity],
    recording_service_bytes: usize,
) -> Result<
    (
        StartedLocalWorkset<(T::Bound, RendererService<'a, C>)>,
        Scopes,
    ),
    Refusal<&'static str>,
> {
    start_selected(
        prepared,
        None,
        PresentationOrigin::Continuation {
            bootstrap,
            verified,
            configuration,
            scope,
        },
        resources,
        policy,
        renderer,
        issuer,
        operation,
        reader,
        face,
        held_pools,
        recording_service_bytes,
    )
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
#[inline(always)]
fn start_selected<'a, 'w, T: PreparedLocalKernels, C: RendererClock>(
    prepared: PreparedLocalWorkset<'w, T, 3>,
    workspace: Option<&b::HostCommandInput<{ crate::config::MAX_ASSIGNMENT_BYTES }>>,
    origin: PresentationOrigin<'_, '_, 'w>,
    resources: &mut c::ResourceAdmissionOwner,
    policy: Policy<'_>,
    renderer: &'a mut ReadyRenderer<C>,
    issuer: &mut Issuer,
    operation: &Operation<'_>,
    reader: Reader<'a>,
    face: &LocalPublicationFace,
    held_pools: &[c::AssignedIdentity],
    recording_service_bytes: usize,
) -> Result<
    (
        StartedLocalWorkset<(T::Bound, RendererService<'a, C>)>,
        Scopes,
    ),
    Refusal<&'static str>,
> {
    let authority = origin.native(resources.host(), policy)?;
    let presentation::Presentation {
        service,
        scopes,
        basis,
        shared,
    } = presentation::prepare(
        &prepared, authority, policy, renderer, issuer, operation, reader, face,
    )?;
    let observations = observe_held(&prepared, resources, held_pools)?;
    // The fixed outer table lives inline; its owned identity payload is heap.
    let observation_bytes = observations
        .iter()
        .map(|o| {
            o.host_id.0.capacity()
                + o.boot_id.0.capacity()
                + o.pool_id.0.capacity()
                + o.class_id.0.capacity()
                + o.sign_id.0.capacity()
        })
        .sum::<usize>();
    let shared = shared
        .checked_add(recording_service_bytes)
        .ok_or(Refusal::Bounds)?;
    let prepared = prepared.with_services(PreparedRendererService::new(service), shared)?;
    let mut committed = prepared.admit(resources, &observations, observation_bytes)?;
    // Scope storage was fully prepared and charged before admission. Binding
    // only moves it; invalid provenance cannot expose a partially scoped Face.
    face.bind_scope(basis)
        .map_err(|_| Refusal::Back("face-scope"))?;
    origin
        .native(resources.host(), policy)?
        .start(&mut committed, workspace, policy, resources)
        .map(|started| (started, scopes))
        .map_err(|refusal| match refusal {
            Refusal::Scope => Refusal::Back("authenticated-start-scope"),
            other => other,
        })
}

/// Original immutable authority references only: no Host loan survives mutable
/// batch admission. Rechecking does not create a receipt or a command stamp.
enum PresentationOrigin<'a, 'scope, 'work> {
    Online {
        bootstrap: &'a b::MemberBootstrap<'a>,
        member: &'a b::MembershipCredential,
        configuration: &'a conduit_mcu_host_runtime::root::Configuration,
    },
    Continuation {
        bootstrap: &'a b::MemberBootstrap<'a>,
        verified: &'a b::VerifiedContinuationIntent<'a>,
        configuration: &'a conduit_mcu_host_runtime::root::Configuration,
        scope: &'scope b::FreshContinuationScope<'work>,
    },
}
impl<'scope, 'work> PresentationOrigin<'_, 'scope, 'work> {
    fn native<'a>(
        &'a self,
        actual: &'a c::HostAdvertisement,
        policy: Policy<'_>,
    ) -> Result<crate::play::NativeAuthority<'a, 'scope, 'work>, Refusal<&'static str>> {
        match self {
            Self::Online {
                bootstrap,
                member,
                configuration,
            } => Ok(b::NativePreparationContext {
                bootstrap,
                member,
                actual,
                image_id: configuration.image_id,
                configuration_id: configuration.configuration_id,
                now_millis: crate::runtime::now_ms(),
            }
            .into()),
            Self::Continuation {
                bootstrap,
                verified,
                configuration,
                scope,
            } => {
                let context = conduit_mcu_host_runtime::continuation_native::ContinuationNativeContext::check_installation(
                    bootstrap, actual, verified, configuration.image_id, configuration.configuration_id, policy,
                ).map_err(|_| Refusal::Scope)?;
                Ok(crate::play::NativeAuthority::Continuation { context, scope })
            }
        }
    }
}

fn observe_held<T: PreparedLocalKernels>(
    prepared: &PreparedLocalWorkset<'_, T, 3>,
    owner: &c::ResourceAdmissionOwner,
    held: &[c::AssignedIdentity],
) -> Result<heapless::Vec<c::ResourceObservation, 6>, Refusal<&'static str>> {
    if !owner.admissions().is_empty() {
        return Err(Refusal::Scope);
    }
    let actual = owner.host();
    let mut observations = heapless::Vec::<c::ResourceObservation, 6>::new();
    for fragment in prepared.fragments() {
        for binding in b::PlainFragmentResources::new(fragment.metadata().resource_bindings) {
            let binding = binding.map_err(|_| Refusal::Scope)?;
            if !held.contains(&c::AssignedIdentity::from_text(binding.pool)) {
                return Err(Refusal::Scope);
            }
            if observations
                .iter()
                .any(|o: &c::ResourceObservation| o.pool_id.as_str() == binding.pool)
            {
                continue;
            }
            let mut matching = actual
                .resources
                .iter()
                .filter(|p| p.pool_id.as_str() == binding.pool);
            let pool = matching.next().ok_or(Refusal::Scope)?;
            if matching.next().is_some()
                || pool.class_id.as_str() != binding.class
                || pool.capacity_units < binding.units
                || pool.compute.is_some()
                || pool.content.is_some()
            {
                return Err(Refusal::Scope);
            }
            observations
                .push(c::ResourceObservation {
                    host_id: actual.host_id.clone(),
                    boot_id: actual.boot_id.clone(),
                    pool_id: pool.pool_id.clone(),
                    class_id: pool.class_id.clone(),
                    offer_generation: actual.offer_generation,
                    health: c::ResourceHealth::Ready,
                    unreserved_units: pool.capacity_units,
                    utilized_units: 0,
                    sign_id: format!(
                        "{}/held-workset-resources/1/{}",
                        actual.boot_id.as_str(),
                        binding.pool
                    )
                    .into(),
                })
                .map_err(|_| Refusal::Bounds)?;
        }
    }
    Ok(observations)
}
