//! Ordinary hosted planning and bounded assignment projection for this MCU.
use crate::{admission::Authority, checked, Result};
use conduit_body as b;
use conduit_core as c;
pub struct Prepared {
    pub body_plan: b::BodyPlan,
    pub wake: b::Wake,
    pub play: b::BodyPlayIdentity,
    pub fragment: c::FragmentId,
    pub active: c::ActivePlayIdentity,
    pub generation: u64,
    pub wire: Vec<u8>,
}
pub fn prepare(
    body: &mut b::BodyLifecycleSession,
    authority: &Authority,
    source: conduit_environment_runtime::construction::ApplianceSource,
    advertisement: c::HostAdvertisement,
    policy: serde_json::Value,
) -> Result<Prepared> {
    let recipe = checked(conduit_environment_make::NativeSoftwareSourceRecipe::prepare(&source))?;
    if policy["host_id"] != serde_json::to_value(&advertisement.host_id)?
        || policy["boot_id"] != serde_json::to_value(&advertisement.boot_id)?
        || policy["offer_generation"] != advertisement.offer_generation.0
        || policy["source_identity"]
            != serde_json::to_value(recipe.spec().source.expanded_plot_digest)?
    {
        return Err("Root policy differs from current Source/Host scope".into());
    }
    let grants: Vec<c::AuthorityGrant> =
        serde_json::from_value(policy["authority_grants"].clone())?;
    let plan =
        checked(source.plan_local(&advertisement, &["conduit.base/local@1".into()], &grants))?;
    let resident = b::ResidentPlot::new(
        source.expanded.source_document_id.clone(),
        source.expanded.checked_plot_id.clone(),
    );
    if plan.fragments.len() != 1 {
        return Err("expected exact single MCU fragment".into());
    }
    let lowered = checked(conduit_plan_lowering::lowering::lower_plan_fragment(
        &plan.fragments[0],
    ))?;
    let generated = checked(
        conduit_embedded_build::generate_embedded_plan_with_computations(
            &plan.fragments[0],
            &lowered,
            conduit_embedded_build::EmbeddedImageBounds {
                maximum_nodes: 15,
                maximum_cords: 14,
                maximum_routes: 15,
                maximum_route_targets: 14,
                maximum_host_calls: 15,
                maximum_resources: 8,
                maximum_sign_expectations: 46,
                maximum_configuration_entries: 32,
                maximum_ports_per_node: 1,
                maximum_remote_endpoints: 0,
                maximum_cord_value_slots: 14,
                maximum_cord_value_bytes: 32 * 1024,
                maximum_sign_items: 46,
                maximum_sign_bytes: 2862,
            },
            conduit_embedded_build::EmbeddedComputationBounds {
                maximum_program_bytes: 32 * 1024,
                maximum_total_program_bytes: 48 * 1024,
            },
        ),
    )?;
    let encoded = checked(conduit_embedded_build::encode_assigned_native_plan(
        &generated,
        c::AssignedPlanMaxima {
            // Exact independently provisioned appliance layout, including its nine
            // configuration records and two lifetime records.
            encoded_bytes: 48 * 1024,
            runtime_state_bytes: 32 * 1024,
            counts: [15, 25, 9, 14, 11, 14, 6, 2, 46, 0, 29, 30, 0],
        },
    ))?;
    let records = checked(c::AssignedPlanRecords::new(&encoded))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| format!("{e:?}"))?;
    let material: Vec<_> = recipe.resident_computation_records().collect();
    let mut wire =
        vec![0; conduit_environment_runtime::appliance::assignment_transfer::MAXIMUM_WIRE_BYTES];
    let size = checked(
        conduit_environment_runtime::appliance::assignment_transfer::encode(
            &encoded[..c::ASSIGNED_PLAN_HEADER_BYTES],
            &records,
            recipe.spec().source,
            &material,
            &mut wire,
        ),
    )?;
    wire.truncate(size);
    let realization = checked(body.propose(
        vec![b::BodyPlotPlan {
            plot: resident,
            plan,
        }],
        &authority.host.host_id,
        &authority.host.boot_id,
    ))?;
    let body_plan = realization.plan.clone();
    let wake = realization.wake.clone();
    let fragment = &body_plan.plots[0].plan.fragments[0];
    let active = c::bind_active_play(
        &body_plan.plots[0].plan.plan_id,
        &fragment.host_id,
        &fragment.boot_id,
        1,
    );
    let fragment = fragment.fragment_id.clone();
    let play = b::BodyPlayIdentity::bind(&body_plan, 1);
    Ok(Prepared {
        body_plan,
        wake,
        play,
        fragment,
        active,
        generation: advertisement.offer_generation.0,
        wire,
    })
}
