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
use serde::Deserialize;

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
#[inline(never)] // Admission and Start have a separate finite stack lifetime.
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
    held_pools: [c::AssignedIdentity; 5],
) -> Result<
    (
        StartedLocalWorkset<(T::Bound, RendererService<'a, C>)>,
        Scopes,
    ),
    Refusal<&'static str>,
> {
    use crate::workset::generated::{mask_layout as mask, sampling_layout as sample};
    let sampling = prepared
        .fragments()
        .iter()
        .find(|f| {
            let m = f.metadata();
            m.source == sample::SOURCE_DOCUMENT_ID
                && m.checked_plot == sample::CHECKED_PLOT_ID
                && m.expanded_plot == sample::EXPANDED_PLOT_ID
        })
        .ok_or(Refusal::Scope)?;
    let approved = prepared
        .fragments()
        .iter()
        .find(|f| {
            let m = f.metadata();
            m.source == mask::SOURCE_DOCUMENT_ID
                && m.checked_plot == mask::CHECKED_PLOT_ID
                && m.expanded_plot == mask::EXPANDED_PLOT_ID
        })
        .ok_or(Refusal::Scope)?;
    let sm = sampling.metadata();
    let m = approved.metadata();
    let output = prepared
        .fragments()
        .iter()
        .find(|f| {
            f.metadata().checked_plot == crate::workset::generated::output_layout::CHECKED_PLOT_ID
        })
        .ok_or(Refusal::Scope)?;
    let scopes = Scopes {
        body: member.body_id.clone(),
        active: c::ActivePlayIdentity {
            active_play_id: m.body_play.into(),
            plan_id: m.body_plan.into(),
            host_id: m.host.into(),
            boot_id: m.boot.into(),
            play_sequence: m.play_sequence,
        },
        fragments: [
            sm.fragment.into(),
            m.fragment.into(),
            output.metadata().fragment.into(),
        ],
    };

    if m.body != member.body_id.as_str()
        || sm.body != m.body
        || sm.wake != m.wake
        || sm.body_plan != m.body_plan
        || sm.body_play != m.body_play
    {
        return Err(Refusal::Scope);
    }
    // Deserialize the actual opaque approval's Wake identity, never birth a
    // surrogate Body just to obtain a conveniently typed identifier.
    let wake = b::WakeId::deserialize(serde::de::value::BorrowedStrDeserializer::<
        serde::de::value::Error,
    >::new(sm.wake))
    .map_err(|_| Refusal::Scope)?;
    let basis = FaceBasis {
        body_id: Some(member.body_id.clone()),
        wake_id: Some(wake),
        source_document_id: Some(sm.source.into()),
        checked_plot_id: Some(sm.checked_plot.into()),
        expanded_plot_id: Some(sm.expanded_plot.into()),
        plan_id: Some(sm.plot_plan.into()),
        active_play_id: Some(sm.body_play.into()),
        sign_ids: Vec::new(),
    };
    let basis_bytes = basis
        .body_id
        .as_ref()
        .map_or(0, |x| x.retained_backing_bytes())
        + basis
            .wake_id
            .as_ref()
            .map_or(0, |x| x.retained_backing_bytes())
        + basis
            .source_document_id
            .as_ref()
            .map_or(0, |x| x.0.capacity())
        + basis.checked_plot_id.as_ref().map_or(0, |x| x.0.capacity())
        + basis
            .expanded_plot_id
            .as_ref()
            .map_or(0, |x| x.0.capacity())
        + basis.plan_id.as_ref().map_or(0, |x| x.0.capacity())
        + basis.active_play_id.as_ref().map_or(0, |x| x.0.capacity());
    let node = mask::MASK_ROLES
        .ok_or(Refusal::Scope)?
        .iter()
        .position(|role| *role == conduit_seven_segment_mask::native_kernel::MaskRole::Renderer)
        .ok_or(Refusal::Scope)? as u16;
    let gear: c::GearId = mask::GEAR_IDS
        .iter()
        .find(|(n, _)| *n == node)
        .ok_or(Refusal::Scope)?
        .1
        .into();
    let actual = resources.host();
    let active = c::bind_active_play(
        &m.plot_plan.into(),
        &actual.host_id,
        &actual.boot_id,
        m.play_sequence,
    );
    let identity = renderer.native_identity();
    let capability: c::CapabilityId = conduit_mcu_host_runtime::execution_mask::CAPABILITY.into();
    // SAFETY: the Source factory has checked the exact full Mask projection and
    // retains its original Back owners. Source Gear ordinal and this actual
    // native owner supply all independent presenter identity.
    let context = unsafe {
        PreparedRendererContext::from_assigned(
            approved,
            AssignedRendererSource {
                gear: &gear,
                host: &actual.host_id,
                boot: &actual.boot_id,
                capability: &capability,
                implementation: &identity.implementation_id,
                artifact: &identity.artifact_id,
                generation: actual.offer_generation,
            },
        )
    }
    .map_err(|_| Refusal::Scope)?;
    let native_context = b::NativePreparationContext {
        bootstrap,
        member,
        actual,
        image_id: configuration.image_id,
        configuration_id: configuration.configuration_id,
        now_millis: crate::runtime::now_ms(),
    };
    let (table, handle, claim) = issuer
        .issue(
            native_context,
            policy,
            approved,
            &NativeFacts::renderer(renderer),
            operation,
            crate::runtime::remaining(),
        )
        .map_err(|_| Refusal::Back("renderer-authority"))?;
    // SAFETY: the service retains original issuance and actual Reader/renderer,
    // and remains inert in PreparedRendererService until original group Start.
    let service = unsafe {
        renderer.bind(
            reader,
            context,
            active,
            conduit_kernel::NodeId(node),
            table,
            handle,
            claim,
        )
    }
    .map_err(|_| Refusal::Back("renderer-preparation"))?;
    let shared = face
        .retained_backing_bytes()
        .map_err(|_| Refusal::Bounds)?
        .checked_add(basis_bytes)
        .and_then(|n| n.checked_add(scopes.backing_bytes()))
        .ok_or(Refusal::Bounds)?;
    let observations = observe_held(&prepared, resources, held_pools)?;
    let observation_bytes = observations.capacity()
        * core::mem::size_of::<c::ResourceObservation>()
        + observations
            .iter()
            .map(|o| {
                o.host_id.0.capacity()
                    + o.boot_id.0.capacity()
                    + o.pool_id.0.capacity()
                    + o.class_id.0.capacity()
                    + o.sign_id.0.capacity()
            })
            .sum::<usize>();
    let prepared = prepared.with_services(PreparedRendererService::new(service), shared)?;
    let committed = prepared.admit(resources, &observations, observation_bytes)?;
    // Scope storage was fully prepared and charged before admission. Binding
    // only moves it; invalid provenance cannot expose a partially scoped Face.
    face.bind_scope(basis).map_err(|_| Refusal::Scope)?;
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
        .map_err(|(_, refusal)| refusal)
}

fn observe_held<T: PreparedLocalKernels>(
    prepared: &PreparedLocalWorkset<'_, T, 3>,
    owner: &c::ResourceAdmissionOwner,
    held: [c::AssignedIdentity; 5],
) -> Result<Vec<c::ResourceObservation>, Refusal<&'static str>> {
    if !owner.admissions().is_empty() {
        return Err(Refusal::Scope);
    }
    let actual = owner.host();
    let mut observations = Vec::new();
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
            observations.try_reserve(1).map_err(|_| Refusal::Bounds)?;
            observations.push(c::ResourceObservation {
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
            });
        }
    }
    Ok(observations)
}
