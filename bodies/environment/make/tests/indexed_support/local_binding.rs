//! Local preparation transfers the same lean owners used by firmware. No native
//! provider is moved or invoked; Body start and complete memory admission remain separate.
use super::*;
use conduit_environment_runtime::appliance::assigned::{
    NativePreparationRefusal, PreparedResidentSoftware,
};
use conduit_plan_lowering::local_tables::{DirectLocalKernelTables, VerifiedLocalLowering};
type DirectTables<'a> = DirectLocalKernelTables<'a, 15, 14, 1, 15, 14, 15>;
#[path = "../../../runtime/tests/support/verified_metadata.rs"]
mod verified_metadata;

#[test]
fn local_verified_binding_preserves_owners_on_refusal_and_transfers_once() {
    let source = compile(1, 0).unwrap();
    let recipe = NativeSoftwareSourceRecipe::prepare(&source).unwrap();
    let computations: Vec<_> = recipe.resident_computations().collect();
    let offers = recipe.resident_offers();
    // SAFETY: independently checked immutable Make output from this recipe.
    let image = unsafe {
        conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(
            recipe.planning_image(),
        )
    }
    .unwrap();
    let mut visited = 0;
    let (count, streamed) = allocation_probe::observe(|| {
        image
            .visit_connections(|cord| {
                assert_eq!(cord, &source.expanded.connections[visited]);
                visited += 1;
            })
            .unwrap()
    });
    assert_eq!(count, 14);
    assert_eq!(visited, 14);
    assert_eq!(streamed.retained_bytes(), 0);
    let (_, projected) = allocation_probe::observe(|| image.planning_metadata().unwrap());
    assert!(streamed.peak_bytes < projected.peak_bytes);
    eprintln!(
        "sampler Cord inspection peak={} projected_metadata_peak={}",
        streamed.peak_bytes, projected.peak_bytes
    );
    let mut slot = Some(Box::new(
        PreparedResidentSoftware::prepare_source(
            &recipe.spec(),
            &computations,
            conduit_environment_runtime::appliance::prepare_value_storage().unwrap(),
        )
        .unwrap(),
    ));
    let (ready, effects) = pulse::ready(0);
    let mut host = appliance_support::native_advertisement(&ready);
    for bytes in slot
        .as_ref()
        .unwrap()
        .source_offers(&recipe.spec(), &offers)
        .unwrap()
        .iter()
    {
        host.capabilities
            .push(serde_json::from_slice(bytes).unwrap());
    }
    let placements =
        conduit_planner::indexed_source::default_placements(&image, core::slice::from_ref(&host))
            .unwrap();
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    let grants = [pulse::grant()];
    let plan = conduit_planner::indexed_source::plan_compact_with_connection_limits(
        &image,
        core::slice::from_ref(&host),
        &placements,
        &["conduit.base/local@1".into()],
        PlanningOptions {
            connection_bases: &empty,
            line_candidates: &lines,
            connection_item_capacity: 1,
            connection_byte_capacity: 1024,
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &limits(&source),
    )
    .unwrap()
    .into_verified_metadata()
    .unwrap();
    let id = &plan.preparation_metadata().fragments[0].fragment_id;
    let lowered = VerifiedLocalLowering::prepare(&plan, id).unwrap();
    let scope = (&host.host_id, &host.boot_id, host.offer_generation);
    slot.as_ref()
        .unwrap()
        .check_local(&lowered, &image, &offers, scope)
        .unwrap();
    let (tables, table_memory) =
        allocation_probe::observe(|| DirectTables::prepare(&plan, id).unwrap());
    assert_eq!(
        (
            table_memory.allocations,
            table_memory.reallocations,
            table_memory.peak_bytes
        ),
        (0, 0, 0)
    );
    let ordinary = conduit_plan_lowering::local_tables::VerifiedLocalKernelTables::<
        15,
        14,
        1,
        15,
        14,
        15,
    >::from_verified_lowering(&lowered)
    .unwrap();
    assert_eq!(*tables.nodes(), ordinary.nodes);
    assert_eq!(*tables.cords(), ordinary.cords);
    assert_eq!(tables.startup_order(), ordinary.startup_order());
    assert_eq!(
        tables.cord_value_slots(),
        lowered.lowered().cord_value_slots
    );
    assert_eq!(
        tables.cord_value_bytes(),
        lowered.lowered().cord_value_bytes
    );
    assert_eq!(tables.sign_items(), lowered.lowered().sign_items);
    assert_eq!(tables.sign_bytes(), lowered.lowered().sign_bytes);
    for node in 0..15 {
        let route = |routes: &conduit_kernel::FixedRoutes<15, 14>| {
            routes
                .route(conduit_kernel::NodeId(node), conduit_kernel::PortId(0))
                .map(|r| r.collect::<Vec<_>>())
        };
        assert_eq!(route(tables.routes()), route(&ordinary.routes));
    }
    slot.as_ref()
        .unwrap()
        .check_local_direct(&tables, &image, &offers, scope)
        .unwrap();
    for change in 0..3 {
        let mut h = host.host_id.clone();
        let mut b = host.boot_id.clone();
        let mut g = host.offer_generation;
        match change {
            0 => h = "foreign".into(),
            1 => b = "stale".into(),
            _ => g = c::OfferGeneration(g.0 + 1),
        }
        assert!(PreparedResidentSoftware::bind_local_retained(
            &mut slot,
            &lowered,
            &image,
            &offers,
            (&h, &b, g)
        )
        .is_err());
        assert!(slot.is_some());
        assert!(PreparedResidentSoftware::bind_local_direct_retained(
            &mut slot,
            DirectTables::prepare(&plan, id).unwrap(),
            &image,
            &offers,
            (&h, &b, g)
        )
        .is_err());
        assert!(slot.is_some());
    }
    // A separately sealed, Core-verified Plan cannot replace retained software facts.
    let original = source
        .plan_local(&host, &["conduit.base/local@1".into()], &grants)
        .unwrap();
    for change in 0..2 {
        let mut fragments = original.fragments.clone();
        let gear = fragments[0]
            .placements
            .iter_mut()
            .find(|p| {
                p.kind_id.as_str()
                    == conduit_environment_runtime::publication::PublicationChannel::Reading
                        .kind_id()
            })
            .unwrap();
        if change == 0 {
            gear.artifact_id = "foreign/artifact".into();
        } else {
            gear.configuration.push(c::ConfigurationEntry {
                key: "extra".into(),
                value: c::ConfigurationValue::U64(1),
            });
        }
        let forged = c::seal_plan_with_realization_backs_and_completion(
            c::PlotIdentity {
                source_document_id: original.source_document_id.clone(),
                checked_plot_id: original.checked_plot_id.clone(),
                expanded_plot_id: original.expanded_plot_id.clone(),
            },
            original.completion_policy,
            original.realization_backs.clone(),
            fragments,
        );
        let verified = verified_metadata::from_plan(&forged);
        let lowering = VerifiedLocalLowering::prepare(
            &verified,
            &verified.preparation_metadata().fragments[0].fragment_id,
        )
        .unwrap();
        assert!(PreparedResidentSoftware::bind_local_retained(
            &mut slot, &lowering, &image, &offers, scope
        )
        .is_err());
        assert!(slot.is_some());
        let bad_tables = DirectTables::prepare(
            &verified,
            &verified.preparation_metadata().fragments[0].fragment_id,
        )
        .unwrap();
        assert!(PreparedResidentSoftware::bind_local_direct_retained(
            &mut slot, bad_tables, &image, &offers, scope
        )
        .is_err());
        assert!(slot.is_some());
    }
    let prepared = PreparedResidentSoftware::bind_local_direct_retained(
        &mut slot, tables, &image, &offers, scope,
    )
    .unwrap();
    assert!(slot.is_none());
    assert_eq!(prepared.roles, recipe.spec().roles);
    match &prepared.drivers[usize::from(prepared.roles.publications[0].0)] {
        conduit_environment_runtime::appliance::ApplianceBack::SourcePublication(publication) => {
            assert!(publication.retained_value_storage_bytes() < 128)
        }
        _ => panic!("actual checked reading publication"),
    }

    assert!(prepared.tables.calls().is_sealed());
    assert!(matches!(
        PreparedResidentSoftware::bind_local_direct_retained(
            &mut slot,
            DirectTables::prepare(&plan, id).unwrap(),
            &image,
            &offers,
            scope
        ),
        Err(NativePreparationRefusal::Initialization)
    ));
    let sign_bytes = lowered.lowered().sign_bytes;
    let sign_slots = lowered.lowered().sign_items.max(
        u16::try_from(
            (sign_bytes as usize).div_ceil(core::mem::size_of::<conduit_kernel::KernelEvent>()),
        )
        .unwrap(),
    );
    let signs = conduit_kernel::HostedSignLog::new(sign_slots, sign_bytes).unwrap();
    // The transfer destination feeds the existing scheduler directly. Construction
    // performs no Step and cannot exercise the separately retained pulse provider.
    let tables = prepared.tables.into_parts();
    let _kernel = conduit_kernel::scheduler::FixedScheduler::<_, _, _, 15, 14, 1, 14, 15, 14, 15, 15>::new_boxed_with_active_counts_and_host_calls(
        15, 14, tables.nodes, tables.cords, tables.routes,
        tables.calls, prepared.drivers, prepared.values, signs,
    ).unwrap();
    assert_eq!(effects.get(), 0);
}
