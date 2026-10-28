//! Actual checked Source and retained provider offers; no radio or credential effects.
use super::*;
use conduit_plot::{
    CheckedConnectionRef, CheckedGearRef, LinkedPlanningFactsRef, PlanningOriginRef,
};
static FACTS: std::sync::OnceLock<LinkedPlanningFactsRef<'static>> = std::sync::OnceLock::new();
fn facts() -> Result<LinkedPlanningFactsRef<'static>, conduit_plot::flat_image::ImageRefusal> {
    FACTS
        .get()
        .copied()
        .ok_or(conduit_plot::flat_image::ImageRefusal::UnsupportedGraph)
}
pub(super) fn assert_plan(
    source: &conduit_plot::ExpandedCanonicalPlot,
    host: &c::HostAdvertisement,
    bases: &[c::BaseImplementationId],
    grants: &[c::AuthorityGrant],
    expected: &c::Plan,
) {
    // Test-only pinning models the emitted immutable ROM callback. Every field is
    // taken from this very checked Source, not from a hand-authored numeric graph.
    let pinned = Box::leak(Box::new(source.clone()));
    let gears = Box::leak(
        pinned
            .gears
            .iter()
            .map(CheckedGearRef::from)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    let connections = Box::leak(
        pinned
            .connections
            .iter()
            .map(CheckedConnectionRef::from)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    let provenance = Box::leak(
        pinned
            .provenance
            .iter()
            .map(|v| PlanningOriginRef {
                gear_id: v.gear_id.as_str(),
                source_span: v.source_span,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    assert!(FACTS
        .set(LinkedPlanningFactsRef {
            source_document_id: pinned.source_document_id.as_str(),
            checked_plot_id: pinned.checked_plot_id.as_str(),
            expanded_plot_id: pinned.expanded_plot_id.as_str(),
            name: &pinned.name,
            completion: pinned.completion,
            gears,
            connections,
            provenance
        })
        .is_ok());
    let bytes = conduit_plot::flat_image::indexed::encode(source).unwrap();
    // SAFETY: this exact fixture just checked/encoded Source and pinned all of its
    // complete callback facts above. No received caller graph replaces them.
    let image = unsafe {
        conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(&bytes)
            .unwrap()
            .with_make_planning_facts(facts)
    };
    let (_, allocation) = allocation_probe::observe(|| {
        conduit_planner::borrowed_local::with_linked_local_plan(
            &image,
            host.into(),
            bases,
            c::AuthorityGrantRefs::Owned(grants),
            1,
            MAXIMUM_JOIN_INPUT_BYTES,
            |scope| {
                let flat = scope.verified().view();
                let fragment = &expected.fragments[0];
                assert_eq!(flat.plan_id, expected.plan_id.as_str());
                assert_eq!(flat.fragment_id, fragment.fragment_id.as_str());
                assert_eq!(flat.placements.len(), fragment.placements.len());
                for (a, b) in flat.placements.iter().zip(&fragment.placements) {
                    assert_gear(a, b.into());
                }
                assert_eq!(flat.tail.connections.len(), fragment.connections.len());
                for (a, b) in flat.tail.connections.iter().zip(&fragment.connections) {
                    assert_connection(a, b.into());
                }
                assert_eq!(flat.tail.sign_storage_budget, &fragment.sign_storage_budget);
            },
        )
        .unwrap()
    });
    assert_refusals(&image, host, bases, grants);
    assert_eq!(allocation.retained_bytes(), 0);
    println!("CPU actual Source borrowed ordinary planner peak={} retained={} allocations={} (excludes Ready/Root/kernel custody; not target admission)",allocation.peak_bytes,allocation.retained_bytes(),allocation.allocations);
}
fn assert_gear(a: c::PlannedGearRef<'_>, b: c::PlannedGearRef<'_>) {
    assert_eq!(a.placement_id, b.placement_id, "placement_id");
    assert_eq!(a.gear_id, b.gear_id, "gear_id");
    assert_eq!(a.kind_id, b.kind_id, "kind_id");
    assert_eq!(
        a.kind_contract_revision, b.kind_contract_revision,
        "kind_contract_revision"
    );
    assert_eq!(
        a.execution_profile_id, b.execution_profile_id,
        "execution_profile_id"
    );
    assert_eq!(a.host_id, b.host_id, "host_id");
    assert_eq!(a.boot_id, b.boot_id, "boot_id");
    assert_eq!(a.capability_id, b.capability_id, "capability_id");
    assert_eq!(
        a.implementation_id, b.implementation_id,
        "implementation_id"
    );
    assert_eq!(a.artifact_id, b.artifact_id, "artifact_id");
    assert_eq!(a.source_span, b.source_span, "source_span");
    assert_eq!(a.offer_generation, b.offer_generation, "offer_generation");
    assert_eq!(a.configuration, b.configuration, "configuration");
    assert_eq!(
        a.realization_characteristics, b.realization_characteristics,
        "realization_characteristics"
    );
    assert_eq!(
        a.realization_properties, b.realization_properties,
        "realization_properties"
    );
    assert_eq!(a.limits, b.limits, "limits");
    assert_eq!(
        a.semantic_contract, b.semantic_contract,
        "semantic_contract"
    );
    assert_eq!(
        a.terminal_transductions, b.terminal_transductions,
        "terminal_transductions"
    );
    assert_eq!(a.pool_references, b.pool_references, "pool_references");
    assert_eq!(a.base.map(base), b.base.map(base));
    assert!(a.inputs.iter().eq(b.inputs.iter()));
    assert!(a.outputs.iter().eq(b.outputs.iter()));
    assert!(a.host_calls.iter().eq(b.host_calls.iter()));
    assert_eq!(a.resources.len(), b.resources.len());
    for (a, b) in a.resources.iter().zip(b.resources.iter()) {
        assert_eq!(
            (
                a.pool_id,
                a.class_id,
                a.units,
                a.protected,
                a.compute,
                a.content
            ),
            (
                b.pool_id,
                b.class_id,
                b.units,
                b.protected,
                b.compute,
                b.content
            )
        );
    }
    assert_eq!(a.authority.len(), b.authority.len());
    for (a, b) in a.authority.iter().zip(b.authority.iter()) {
        assert_eq!(authority(a), authority(b));
    }
}
fn base(
    v: c::BaseProviderBindingRef<'_>,
) -> (&str, &str, u64, &str, &str, c::BaseEnforcementClass) {
    (
        v.base_id,
        v.provider_instance_id,
        v.provider_generation,
        v.implementation_id,
        v.mechanism_family,
        v.enforcement_class,
    )
}
fn authority(v: c::AuthorityBindingRef<'_>) -> (&str, &str, &str, &str, &str, &str, &str) {
    (
        v.grant_id,
        v.contract_id,
        v.host_call_contract_id,
        v.subject_kind,
        v.host_id,
        v.boot_id,
        v.capability_id,
    )
}
fn assert_connection(a: c::PlannedConnectionRef<'_>, b: c::PlannedConnectionRef<'_>) {
    assert_eq!(a.connection_id, b.connection_id, "connection_id");
    assert_eq!(
        a.source_placement_id, b.source_placement_id,
        "source_placement_id"
    );
    assert_eq!(a.source_port_id, b.source_port_id, "source_port_id");
    assert_eq!(
        a.sink_placement_id, b.sink_placement_id,
        "sink_placement_id"
    );
    assert_eq!(a.sink_port_id, b.sink_port_id, "sink_port_id");
    assert_eq!(a.value_kind, b.value_kind, "value_kind");
    assert!(a.resource.is_none());
    assert!(b.resource.is_none());
    assert_eq!(a.abnormal_kind, b.abnormal_kind, "abnormal_kind");
    assert_eq!(a.track, b.track, "track");
    assert_eq!(a.temporal, b.temporal, "temporal");
    assert_eq!(a.pressure_policy, b.pressure_policy, "pressure_policy");
    assert_eq!(a.selected_line, b.selected_line, "selected_line");
    assert_eq!(a.admitted_lines, b.admitted_lines, "admitted_lines");
    assert_eq!(a.item_capacity, b.item_capacity, "item_capacity");
    assert_eq!(a.byte_capacity, b.byte_capacity, "byte_capacity");
}

fn assert_refusals(
    image: &conduit_plot::flat_image::indexed::IndexedSourceImage<'_>,
    host: &c::HostAdvertisement,
    bases: &[c::BaseImplementationId],
    grants: &[c::AuthorityGrant],
) {
    conduit_planner::borrowed_local::with_linked_local_plan(
        image,
        host.into(),
        bases,
        c::AuthorityGrantRefs::Owned(grants),
        1,
        MAXIMUM_JOIN_INPUT_BYTES,
        |scope| {
            let original = scope.verified().view();
            let mut entries = original
                .placements
                .iter()
                .map(|gear| c::PlainLocalPlacementRef {
                    placement_id: gear.placement_id,
                    gear_id: gear.gear_id,
                    source_span: gear.source_span,
                    capability_index: host
                        .capabilities
                        .iter()
                        .position(|offer| offer.capability_id.as_str() == gear.capability_id)
                        .unwrap(),
                    base_index: gear.base.map(|base| {
                        host.bases
                            .iter()
                            .position(|offer| offer.base_id.as_str() == base.base_id)
                            .unwrap()
                    }),
                    resources: gear.resources,
                    authority: gear.authority,
                })
                .collect::<Vec<_>>();
            {
                let projection = c::PlainLocalPlannedGears::new(host.into(), &entries).unwrap();
                let mut view = original;
                view.placements = c::PlannedGearRefs::Projected(&projection);
                assert!(view.verify().is_ok());
                assert!(projection.get(entries.len()).is_none());
            }
            entries[0].gear_id = "forged/projected-gear";
            {
                let projection = c::PlainLocalPlannedGears::new(host.into(), &entries).unwrap();
                let mut view = original;
                view.placements = c::PlannedGearRefs::Projected(&projection);
                assert!(
                    view.verify().is_err(),
                    "projection must not bypass canonical verification"
                );
            }
            entries[0].capability_index = host.capabilities.len();
            assert!(c::PlainLocalPlannedGears::new(host.into(), &entries).is_err());
            entries[0].capability_index = 0;
            entries[0].base_index = Some(host.bases.len());
            assert!(c::PlainLocalPlannedGears::new(host.into(), &entries).is_err());
        },
    )
    .unwrap();
    for mutation in 0..14 {
        let mut changed = host.clone();
        let mut authority = grants.to_vec();
        match mutation {
            0 => changed
                .capabilities
                .retain(|c| c.capability_id.as_str() != JOIN_CAPABILITY),
            1 => changed.resources.clear(),
            2 => authority.clear(),
            3 => authority[0].boot_id = "wrong/boot".into(),
            4 => authority.push(authority[0].clone()),
            5 => authority[0].subject_kind = "".into(),
            6 => changed.resources[0].capacity_units = 0,
            7 => changed.bases[0].provider_generation = 0,
            8 => {
                changed
                    .capabilities
                    .iter_mut()
                    .find(|c| c.capability_id.as_str() == JOIN_CAPABILITY)
                    .unwrap()
                    .limits
                    .max_queue_bytes = 1
            }
            9 => {
                changed
                    .capabilities
                    .iter_mut()
                    .find(|c| c.capability_id.as_str() == JOIN_CAPABILITY)
                    .unwrap()
                    .limits
                    .max_active_instances = 0
            }
            10 => {
                changed
                    .capabilities
                    .iter_mut()
                    .find(|c| c.capability_id.as_str() == JOIN_CAPABILITY)
                    .unwrap()
                    .inputs[0]
                    .port_id = "wrong".into()
            }
            11 => changed.bases.push(changed.bases[0].clone()),
            _ => {
                let mut duplicate = changed
                    .capabilities
                    .iter()
                    .find(|c| c.capability_id.as_str() == JOIN_CAPABILITY)
                    .unwrap()
                    .clone();
                if mutation == 12 {
                    duplicate.kind_contract_revision = "wrong/revision".into();
                } else {
                    duplicate.inputs[0].port_id = "wrong".into();
                }
                changed.capabilities.insert(0, duplicate);
            }
        }
        let empty_bases = std::collections::BTreeMap::new();
        let empty_candidates = std::collections::BTreeMap::new();
        let empty_limits = std::collections::BTreeMap::new();
        let old = conduit_planner::indexed_source::default_placements(image, &[changed.clone()])
            .and_then(|choices| {
                conduit_planner::indexed_source::plan_with_connection_limits(
                    image,
                    &[changed.clone()],
                    &choices,
                    bases,
                    conduit_planner::PlanningOptions {
                        connection_bases: &empty_bases,
                        line_candidates: &empty_candidates,
                        connection_item_capacity: 1,
                        connection_byte_capacity: MAXIMUM_JOIN_INPUT_BYTES,
                        authority_grants: &authority,
                        protected_resource_grants: &[],
                        line_offers: &[],
                    },
                    &empty_limits,
                )
            });
        let old = old.expect_err("mutation must refuse rich planning");
        let new = conduit_planner::borrowed_local::with_linked_local_plan(
            image,
            (&changed).into(),
            bases,
            c::AuthorityGrantRefs::Owned(&authority),
            1,
            MAXIMUM_JOIN_INPUT_BYTES,
            |_| (),
        )
        .unwrap_err();
        match new {
            conduit_planner::borrowed_local::BorrowedLocalPlanningRefusal::Planning(new) => {
                assert_eq!(old, new, "mutation {mutation}")
            }
            other => panic!("wrong refusal {mutation}: {other:?}"),
        }
    }
}
