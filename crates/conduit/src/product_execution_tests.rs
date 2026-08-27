use crate::form_source;
use crate::product_execution::{ProductExecutionContext, ProductRuntime};
use conduit_core::{BootId, ConnectionBase, HostId, OfferGeneration};
use conduit_planner::{PlacementChoice, PlacementChoices};
use conduit_std_host::{StdHost, StdHostConfig};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn hello_form() -> conduit_form::ExpandedCanonicalForm {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello.conduit");
    form_source::load(&path)
        .expect("canonical hello source loads")
        .expand_entry()
        .expect("canonical hello expands")
}

fn lenia_form() -> conduit_form::ExpandedCanonicalForm {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/lenia-orbium.conduit");
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
        vec![ConnectionBase::Local, ConnectionBase::WebSocket],
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
        vec![ConnectionBase::Local],
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
        vec![ConnectionBase::Local],
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
fn fixture_only_connection_base_is_not_product_admission() {
    let host = host("product-host");
    let error = ProductExecutionContext::new(
        vec![host.advertisement().clone()],
        vec![ProductRuntime::std(host)],
        vec![ConnectionBase::FixtureFrame],
        Vec::new(),
    )
    .err()
    .expect("fixture Base must not become installed product truth");
    assert!(
        error.contains("FixtureFrame") && error.contains("not supported"),
        "{error}"
    );
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
            64
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
