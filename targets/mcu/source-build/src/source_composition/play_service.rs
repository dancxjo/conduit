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
    workspace: &b::HostCommandInput<16384>,
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
    let presentation::Presentation {
        service,
        scopes,
        basis,
        shared,
    } = presentation::prepare(
        &prepared,
        bootstrap,
        member,
        configuration,
        resources.host(),
        policy,
        renderer,
        issuer,
        operation,
        reader,
        face,
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
    let context = b::NativePreparationContext {
        bootstrap,
        member,
        actual: resources.host(),
        image_id: configuration.image_id,
        configuration_id: configuration.configuration_id,
        now_millis: crate::runtime::now_ms(),
    };
    committed
        .start(workspace, context, policy, resources)
        .map(|started| (started, scopes))
        .map_err(|refusal| match refusal {
            Refusal::Scope => Refusal::Back("authenticated-start-scope"),
            other => other,
        })
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
