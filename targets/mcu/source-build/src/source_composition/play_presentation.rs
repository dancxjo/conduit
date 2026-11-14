use super::*;

pub(super) struct Presentation<'a, C> {
    pub service: RendererService<'a, C>,
    pub scopes: Scopes,
    pub basis: FaceBasis,
    pub shared: usize,
}
/// Prepare the semantic renderer while all execution providers stay inert.
/// Its cold context/authority scratch must retire before batch admission/bind.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(super) fn prepare<'a, 'w, T: PreparedLocalKernels, C: RendererClock>(
    prepared: &PreparedLocalWorkset<'w, T, 3>,
    authority: crate::play::NativeAuthority<'_, '_, '_>,
    policy: Policy<'_>,
    renderer: &'a mut ReadyRenderer<C>,
    issuer: &mut Issuer,
    operation: &Operation<'_>,
    reader: Reader<'a>,
    face: &LocalPublicationFace,
) -> Result<Presentation<'a, C>, Refusal<&'static str>> {
    use crate::workset::generated::{mask_layout as mask, sampling_layout as sample};
    let actual = authority.actual();
    let body = authority.body_id();
    let sampling = prepared
        .fragments()
        .iter()
        .find(|f| {
            let m = f.metadata();
            m.source == sample::SOURCE_DOCUMENT_ID
                && m.checked_plot == sample::CHECKED_PLOT_ID
                && m.expanded_plot == sample::EXPANDED_PLOT_ID
        })
        .ok_or(Refusal::Back("sampling-fragment"))?;
    let approved = prepared
        .fragments()
        .iter()
        .find(|f| {
            let m = f.metadata();
            m.source == mask::SOURCE_DOCUMENT_ID
                && m.checked_plot == mask::CHECKED_PLOT_ID
                && m.expanded_plot == mask::EXPANDED_PLOT_ID
        })
        .ok_or(Refusal::Back("mask-fragment"))?;
    let sm = sampling.metadata();
    let m = approved.metadata();
    let output = prepared
        .fragments()
        .iter()
        .find(|f| {
            f.metadata().checked_plot == crate::workset::generated::output_layout::CHECKED_PLOT_ID
        })
        .ok_or(Refusal::Back("output-fragment"))?;
    let scopes = Scopes {
        body: body.clone(),
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

    if m.body != body.as_str()
        || sm.body != m.body
        || sm.wake != m.wake
        || sm.body_plan != m.body_plan
        || sm.body_play != m.body_play
    {
        return Err(Refusal::Back("fragment-body-scope"));
    }
    let wake = sampling.wake_id();
    let basis = FaceBasis {
        body_id: Some(body.clone()),
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
        .ok_or(Refusal::Back("renderer-role"))?
        .iter()
        .position(|role| *role == conduit_seven_segment_mask::native_kernel::MaskRole::Renderer)
        .ok_or(Refusal::Back("renderer-role-node"))? as u16;
    let gear: c::GearId = mask::GEAR_IDS
        .iter()
        .find(|(n, _)| *n == node)
        .ok_or(Refusal::Back("renderer-gear"))?
        .1
        .into();
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
    .map_err(|_| Refusal::Back("renderer-context"))?;
    let available = crate::runtime::remaining();
    let (table, handle, claim) = authority
        .issue(
            issuer,
            policy,
            approved,
            &NativeFacts::renderer(renderer),
            operation,
            available,
        )
        .map_err(|reason| Refusal::Authority { available, reason })?;
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
    Ok(Presentation {
        service,
        scopes,
        basis,
        shared,
    })
}

// Local ticket correspondence only, never a grant, wire identity or Start proof.
// Hash the exact already approved metadata without allocating or decoding again.
#[inline(never)]
pub(super) fn workset_pin<T: PreparedLocalKernels>(
    prepared: &PreparedLocalWorkset<'_, T, 3>,
) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"conduit.native/prepared-renderer-workset@1");
    for fragment in prepared.fragments() {
        let m = fragment.metadata();
        for bytes in [
            m.body.as_bytes(),
            m.wake.as_bytes(),
            m.body_plan.as_bytes(),
            m.body_play.as_bytes(),
            m.source.as_bytes(),
            m.checked_plot.as_bytes(),
            m.expanded_plot.as_bytes(),
            m.plot_plan.as_bytes(),
            m.fragment.as_bytes(),
            m.host.as_bytes(),
            m.boot.as_bytes(),
            m.resource_bindings,
            m.fore_ports,
        ] {
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
        hash.update(m.schema.to_le_bytes());
        hash.update(m.play_sequence.to_le_bytes());
        hash.update(m.workload_revision.to_le_bytes());
        hash.update(m.offer_generation.to_le_bytes());
        hash.update(m.local_fragment_count.to_le_bytes());
        hash.update(m.local_fragment_digest);
        hash.update(m.projection_length.to_le_bytes());
        hash.update(m.projection_digest);
    }
    hash.finalize().into()
}
