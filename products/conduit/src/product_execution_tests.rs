use crate::form_source;
use crate::product_execution::{ProductExecutionContext, ProductRuntime};
use conduit_core::{BaseImplementationId, BootId, HostId, OfferGeneration};
use conduit_planner::{PlacementChoice, PlacementChoices};
use conduit_std_host::{StdHost, StdHostConfig};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn hello_form() -> conduit_form::ExpandedCanonicalForm {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../forms/hello/main.conduit");
    form_source::load(&path)
        .expect("canonical hello source loads")
        .expand_entry()
        .expect("canonical hello expands")
}

fn lenia_form() -> conduit_form::ExpandedCanonicalForm {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../forms/lenia-orbium/main.conduit");
    form_source::load(&path)
        .expect("canonical Lenia source loads")
        .expand_entry()
        .expect("canonical Lenia demo expands")
}

fn host(name: &str) -> StdHost {
    StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from(name),
        boot_id: BootId::from(format!("{name}/boot")),
        offer_generation: OfferGeneration(1),
    })
}

fn line_offer(
    source: &StdHost,
    sink: &StdHost,
    maximum_value_bytes: u32,
) -> conduit_core::LineOffer {
    let mut offer = crate::std_websocket_line::line_offer(source, sink);
    offer.binding.limits.maximum_in_flight_items = 4;
    offer.binding.limits.maximum_payload_bytes = maximum_value_bytes;
    offer.binding.limits.maximum_buffered_bytes = maximum_value_bytes;
    offer.binding.limits.maximum_frame_bytes = maximum_value_bytes;
    offer
}

#[test]
fn two_advertisement_context_reaches_ordinary_planner_placement() {
    let first = host("product-host-a");
    let second = host("product-host-b");
    let advertisements = vec![
        first.advertisement().clone(),
        second.advertisement().clone(),
    ];
    let context = ProductExecutionContext::new(
        advertisements,
        vec![ProductRuntime::std(first), ProductRuntime::std(second)],
        vec![
            BaseImplementationId::from("conduit.base/local@1"),
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        ],
        Vec::new(),
        Vec::new(),
    )
    .expect("two Hosts and two product Bases are finite context truth");
    let form = hello_form();
    let second_advertisement = context
        .default_placements(&form)
        .expect("ordinary default placement sees the context");
    assert_eq!(second_advertisement.by_gear.len(), form.gears.len());

    let second_host = HostId::from("product-host-b");
    let default_plan = context
        .plan_with_placements(&form, &second_advertisement)
        .expect("default context placement plans");
    let capabilities = default_plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .map(|placement| (placement.gear_id.clone(), placement.capability_id.clone()))
        .collect::<Vec<_>>();
    let placements = PlacementChoices {
        by_gear: capabilities
            .into_iter()
            .map(|(gear_id, capability_id)| {
                (
                    gear_id,
                    PlacementChoice {
                        host_id: second_host.clone(),
                        capability_id,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>(),
    };
    let plan = context
        .plan_with_placements(&form, &placements)
        .expect("ordinary planner accepts explicit placement on the second Host");
    assert_eq!(plan.fragments.len(), 1);
    assert_eq!(plan.fragments[0].host_id, second_host);
}

#[test]
fn duplicate_host_id_is_refused_before_planning() {
    let first = host("duplicate-host");
    let mut duplicate = first.advertisement().clone();
    duplicate.boot_id = BootId::from("different-boot");
    let error = ProductExecutionContext::new(
        vec![first.advertisement().clone(), duplicate],
        vec![ProductRuntime::std(first)],
        vec![BaseImplementationId::from("conduit.base/local@1")],
        Vec::new(),
        Vec::new(),
    )
    .err()
    .expect("duplicate HostId must refuse");
    assert!(
        error.contains("duplicate HostId 'duplicate-host'"),
        "{error}"
    );
}

#[test]
fn planned_local_fragment_without_runtime_is_refused() {
    let advertised_only = host("advertised-only");
    let advertisement = advertised_only.advertisement().clone();
    let mut context = ProductExecutionContext::new(
        vec![advertisement],
        Vec::new(),
        vec![BaseImplementationId::from("conduit.base/local@1")],
        Vec::new(),
        Vec::new(),
    )
    .expect("remote advertisements need not imply local runtime authority");
    let plan = context
        .plan(&hello_form(), None)
        .expect("ordinary planning uses advertised truth");
    let error = context
        .execute(plan, &mut Vec::new())
        .err()
        .expect("dispatch without a runtime must refuse");
    assert!(error.contains("has no runtime handle"), "{error}");
}

#[test]
fn product_context_does_not_classify_base_identity_names() {
    let host = host("product-host");
    ProductExecutionContext::new(
        vec![host.advertisement().clone()],
        vec![ProductRuntime::std(host)],
        vec![BaseImplementationId::from("conduit.proof/frame@1")],
        Vec::new(),
        Vec::new(),
    )
    .expect("an identity name alone neither grants nor denies a runnable Line");
}

#[test]
fn portable_lenia_demo_executes_four_fields_through_the_product_entrance() {
    let mut context = ProductExecutionContext::local_std().unwrap();
    let form = lenia_form();
    let plan = context.plan(&form, None).unwrap();
    for connection in plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
    {
        let expected = if connection.value_kind.as_str() == conduit_alife::SCALAR_FIELD2_INFO_ID {
            conduit_alife::LENIA_MAXIMUM_FIELD_BYTES
        } else {
            16
        };
        assert_eq!(connection.byte_capacity, expected);
    }
    let mut output = Vec::new();
    let execution = context.execute(plan, &mut output).unwrap();
    assert_eq!(execution.plan.fragments.len(), 1);
    let output = String::from_utf8(output).unwrap();
    assert_eq!(output.matches("SCALAR-FIELD title=\"Orbium\"").count(), 4);
    for generation in 1..=4 {
        assert!(output.contains(&format!("generation={generation} width=128 height=128")));
    }
}

#[test]
fn heterogeneous_lines_use_their_exact_connection_bounds_independent_of_offer_order() {
    let form = lenia_form();
    let seed = host("bounds-seed");
    let clock = host("bounds-clock");
    let evolve = host("bounds-evolve");
    let show = host("bounds-show");
    let host_for_kind = |kind: &str| match kind {
        conduit_alife::ORBIUM_SEED_KIND => seed.advertisement(),
        conduit_time::TIME_EVERY_KIND => clock.advertisement(),
        conduit_alife::LENIA_STEP_KIND => evolve.advertisement(),
        conduit_alife::SCALAR_FIELD_PRESENTATION_KIND => show.advertisement(),
        other => panic!("unexpected Lenia demo Kind '{other}'"),
    };
    let placements = PlacementChoices {
        by_gear: form
            .gears
            .iter()
            .map(|gear| {
                let advertisement = host_for_kind(gear.kind_id.as_str());
                let capability = advertisement
                    .capabilities
                    .iter()
                    .find(|capability| capability.kind_id == gear.kind_id)
                    .unwrap();
                (
                    gear.gear_id.clone(),
                    PlacementChoice {
                        host_id: advertisement.host_id.clone(),
                        capability_id: capability.capability_id.clone(),
                    },
                )
            })
            .collect(),
    };
    let field_bytes = conduit_alife::LENIA_MAXIMUM_FIELD_BYTES;
    let offers = vec![
        line_offer(&evolve, &show, field_bytes),
        line_offer(&clock, &evolve, 16),
        line_offer(&seed, &evolve, field_bytes),
    ];
    let advertisements = [&seed, &clock, &evolve, &show]
        .into_iter()
        .map(|host| host.advertisement().clone())
        .collect();
    let context = ProductExecutionContext::new(
        advertisements,
        Vec::new(),
        vec![BaseImplementationId::from(
            "conduit.base/websocket-rfc6455@1",
        )],
        offers.clone(),
        Vec::new(),
    )
    .unwrap();
    let plan = context.plan_with_placements(&form, &placements).unwrap();
    let capacities = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .map(|connection| {
            (
                connection.connection_id.clone(),
                (connection.value_kind.clone(), connection.byte_capacity),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert!(capacities
        .values()
        .any(|(kind, bytes)| kind.as_str() == conduit_time::TICK_VALUE_KIND && *bytes == 16));
    assert_eq!(
        capacities
            .values()
            .filter(|(kind, bytes)| {
                kind.as_str() == conduit_alife::SCALAR_FIELD2_INFO_ID && *bytes == field_bytes
            })
            .count(),
        2
    );

    let mut undersized = offers;
    undersized[0].binding.limits.maximum_payload_bytes = field_bytes - 1;
    undersized[0].binding.limits.maximum_buffered_bytes = field_bytes - 1;
    undersized[0].binding.limits.maximum_frame_bytes = field_bytes - 1;
    let advertisements = [&seed, &clock, &evolve, &show]
        .into_iter()
        .map(|host| host.advertisement().clone())
        .collect();
    let error = ProductExecutionContext::new(
        advertisements,
        Vec::new(),
        vec![BaseImplementationId::from(
            "conduit.base/websocket-rfc6455@1",
        )],
        undersized,
        Vec::new(),
    )
    .unwrap()
    .plan_with_placements(&form, &placements)
    .unwrap_err();
    assert!(
        error.contains("lenia-orbium-demo/organism/evolve")
            && error.contains("lenia-orbium-demo/show"),
        "{error}"
    );
}
