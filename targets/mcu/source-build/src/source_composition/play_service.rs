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
/// Inert original renderer owner, pinned to the exact approved workset.
/// Private fields prevent polling or manufacturing a substitute service.
pub struct PreparedPresentation<'a, C> {
    original: presentation::Presentation<'a, C>,
    workset: [u8; 32],
}
#[allow(clippy::too_many_arguments)]
#[inline(never)] // This cold frame must retire before admission/Start scratch.
pub fn prepare_presentation<'a, T: PreparedLocalKernels, C: RendererClock>(
    prepared: &PreparedLocalWorkset<'_, T, 3>,
    authority: crate::play::NativeAuthority<'_, '_, '_>,
    policy: Policy<'_>,
    renderer: &'a mut ReadyRenderer<C>,
    issuer: &mut Issuer,
    operation: &Operation<'_>,
    reader: Reader<'a>,
    face: &LocalPublicationFace,
) -> Result<PreparedPresentation<'a, C>, Refusal<&'static str>> {
    let workset = presentation::workset_pin(prepared);
    let original = presentation::prepare(
        prepared, authority, policy, renderer, issuer, operation, reader, face,
    )?;
    Ok(PreparedPresentation { original, workset })
}
// Original online preparation output, borrowed only during the cold renderer poll.
// The tuple owns the same opaque workset and mutable ledger; it grants no Start.
pub type OnlinePrepared<'w, 'r, T> = (
    PreparedLocalWorkset<'w, T, 3>,
    &'w b::HostCommandInput<{ crate::config::MAX_ASSIGNMENT_BYTES }>,
    &'r b::MemberBootstrap<'r>,
    &'r b::MembershipCredential,
    &'r conduit_mcu_host_runtime::root::Configuration,
    &'r mut c::ResourceAdmissionOwner,
    Policy<'r>,
    heapless::Vec<c::AssignedIdentity, 6>,
);
#[allow(clippy::too_many_arguments)]
pub async fn prepare_online_in_slot<'a, T: PreparedLocalKernels, C: RendererClock>(
    input: &Option<OnlinePrepared<'_, '_, T>>,
    target: &mut Option<PreparedPresentation<'a, C>>,
    renderer: &'a mut ReadyRenderer<C>,
    issuer: &mut Issuer,
    operation: &Operation<'_>,
    reader: Reader<'a>,
    face: &LocalPublicationFace,
) -> Result<(), Refusal<&'static str>> {
    if target.is_some() {
        return Err(Refusal::Scope);
    }
    let (prepared, _, bootstrap, member, configuration, resources, policy, _) =
        input.as_ref().ok_or(Refusal::Scope)?;
    let authority = b::NativePreparationContext {
        bootstrap,
        member,
        actual: resources.host(),
        image_id: configuration.image_id,
        configuration_id: configuration.configuration_id,
        now_millis: crate::runtime::now_ms(),
    };
    *target = Some(prepare_presentation(
        prepared,
        authority.into(),
        *policy,
        renderer,
        issuer,
        operation,
        reader,
        face,
    )?);
    Ok(())
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
    let authority = b::NativePreparationContext {
        bootstrap,
        member,
        actual: resources.host(),
        image_id: configuration.image_id,
        configuration_id: configuration.configuration_id,
        now_millis: crate::runtime::now_ms(),
    };
    let presentation = prepare_presentation(
        &prepared,
        authority.into(),
        policy,
        renderer,
        issuer,
        operation,
        reader,
        face,
    )?;
    start_prepared(
        prepared,
        workspace,
        bootstrap,
        member,
        configuration,
        resources,
        policy,
        presentation,
        face,
        held_pools,
        recording_service_bytes,
    )
}

/// Consume the separately prepared renderer only at ordinary whole-set admission.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
#[inline(always)]
pub fn start_prepared<'a, 'w, T: PreparedLocalKernels, C: RendererClock>(
    prepared: PreparedLocalWorkset<'w, T, 3>,
    workspace: &b::HostCommandInput<{ crate::config::MAX_ASSIGNMENT_BYTES }>,
    bootstrap: &b::MemberBootstrap<'_>,
    member: &b::MembershipCredential,
    configuration: &conduit_mcu_host_runtime::root::Configuration,
    resources: &mut c::ResourceAdmissionOwner,
    policy: Policy<'_>,
    presentation: PreparedPresentation<'a, C>,
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
        presentation,
        face,
        held_pools,
        recording_service_bytes,
    )
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
#[inline(always)]
pub fn start_prepared_into<'a, 'w, T: PreparedLocalKernels, C: RendererClock>(
    prepared: PreparedLocalWorkset<'w, T, 3>,
    workspace: &b::HostCommandInput<{ crate::config::MAX_ASSIGNMENT_BYTES }>,
    bootstrap: &b::MemberBootstrap<'_>,
    member: &b::MembershipCredential,
    configuration: &conduit_mcu_host_runtime::root::Configuration,
    resources: &mut c::ResourceAdmissionOwner,
    policy: Policy<'_>,
    presentation: PreparedPresentation<'a, C>,
    face: &LocalPublicationFace,
    held_pools: &[c::AssignedIdentity],
    recording_service_bytes: usize,
    target: &mut Option<StartedLocalWorkset<(T::Bound, RendererService<'a, C>)>>,
    scopes_target: &mut Option<Scopes>,
) -> Result<(), Refusal<&'static str>> {
    start_selected_into(
        prepared,
        Some(workspace),
        PresentationOrigin::Online {
            bootstrap,
            member,
            configuration,
        },
        resources,
        policy,
        presentation,
        face,
        held_pools,
        recording_service_bytes,
        target,
        scopes_target,
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
    presentation: PreparedPresentation<'a, C>,
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
        presentation,
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
    presentation: PreparedPresentation<'a, C>,
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
    let mut target = None;
    let mut scopes_target = None;
    start_selected_into(
        prepared,
        workspace,
        origin,
        resources,
        policy,
        presentation,
        face,
        held_pools,
        recording_service_bytes,
        &mut target,
        &mut scopes_target,
    )?;
    Ok((
        target.expect("successful Started slot"),
        scopes_target.expect("successful Scopes slot"),
    ))
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
#[inline(always)]
fn start_selected_into<'a, 'w, T: PreparedLocalKernels, C: RendererClock>(
    prepared: PreparedLocalWorkset<'w, T, 3>,
    workspace: Option<&b::HostCommandInput<{ crate::config::MAX_ASSIGNMENT_BYTES }>>,
    origin: PresentationOrigin<'_, '_, 'w>,
    resources: &mut c::ResourceAdmissionOwner,
    policy: Policy<'_>,
    presentation: PreparedPresentation<'a, C>,
    face: &LocalPublicationFace,
    held_pools: &[c::AssignedIdentity],
    recording_service_bytes: usize,
    target: &mut Option<StartedLocalWorkset<(T::Bound, RendererService<'a, C>)>>,
    scopes_target: &mut Option<Scopes>,
) -> Result<(), Refusal<&'static str>> {
    if target.is_some() || scopes_target.is_some() {
        return Err(Refusal::Scope);
    }
    if presentation.workset != presentation::workset_pin(&prepared) {
        return Err(Refusal::Back("renderer-workset"));
    }
    let presentation::Presentation {
        service,
        scopes,
        basis,
        shared,
    } = presentation.original;
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
        .start_into(&mut committed, workspace, policy, resources, target)
        .map_err(|refusal| match refusal {
            Refusal::Scope => Refusal::Back("authenticated-start-scope"),
            other => other,
        })?;
    *scopes_target = Some(scopes);
    Ok(())
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
