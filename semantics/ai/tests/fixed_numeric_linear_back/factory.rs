//! Preparation through exact Plan cords and immutable adopted resource custody.
use super::*;
use conduit_ai::{
    fixed_tensor_resource::AdmittedFixedTensorResource,
    operation_owners::fixed_numeric::{
        compact_linear_offer_layout, fixed_numeric_offer, FixedNumericOperationFactory,
    },
};
use conduit_composite::KernelOperationFactory;
use std::{collections::BTreeMap, sync::Arc};
fn owners(plan: &Plan, tensor: &TensorValue, packed: &[u8]) -> Vec<FixedNumericOperationFactory> {
    let adopted = Arc::new(
        AdmittedFixedTensorResource::adopt(
            Arc::new(tensor.clone()),
            Arc::from(packed),
            &access(tensor),
        )
        .unwrap(),
    );
    let source = plan.fragments[0]
        .placements
        .iter()
        .find(|g| g.kind_id.as_str() == "numeric-test/weights")
        .unwrap();
    FixedNumericOperationFactory::for_plan(
        plan,
        &BTreeMap::from([(source.placement_id.clone(), adopted)]),
    )
    .unwrap()
}
pub(super) fn prepare(
    plan: &Plan,
    tensor: &TensorValue,
    packed: &[u8],
    gear: &PlannedGear,
) -> Box<dyn StepBack<PORTS>> {
    let owners = owners(plan, tensor, packed);
    let owner = owners
        .iter()
        .find(|f| f.implementation_id() == &gear.implementation_id)
        .unwrap();
    let back = owner.prepare_compact_reusable(gear).unwrap();
    let layout =
        compact_linear_offer_layout(&fixed_numeric_offer(gear.kind_id.as_str()).unwrap()).unwrap();
    assert_eq!(
        back.compact_f32_input(layout.numeric_input.0),
        Some(layout.numeric_input.1)
    );
    assert_eq!(back.compact_f32_output(), Some(layout.output));
    assert!(back.compact_f32_input(layout.canonical_inputs[0]).is_none());
    back
}
#[test]
fn compact_factory_requires_adopted_resource_cord_and_exact_selected_gear() {
    let plan = plan();
    assert!(FixedNumericOperationFactory::for_plan(&plan, &BTreeMap::new()).is_err());
    let packed = bytes(&[0.25; 192 * 4]);
    let tensor = resource_tensor(&[192, 4], &packed);
    let owners = owners(&plan, &tensor, &packed);
    let gear = plan.fragments[0]
        .placements
        .iter()
        .find(|g| g.kind_id.as_str() == "numeric/linear192x4")
        .unwrap();
    let owner = owners
        .iter()
        .find(|f| f.implementation_id() == &gear.implementation_id)
        .unwrap();
    for mutation in 0..3 {
        let mut wrong = gear.clone();
        match mutation {
            0 => wrong.artifact_id = "forged/artifact".into(),
            1 => wrong.placement_id = "forged/placement".into(),
            _ => wrong.kind_contract_revision = "stale/contract".into(),
        }
        assert!(owner.prepare_compact_reusable(&wrong).is_err());
    }
    assert!(owner.prepare_compact_reusable(gear).is_ok());
}

fn shape_plan(i: usize, o: usize) -> Plan {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profile).unwrap();
    let mut offers = Vec::new();
    for (name, ty, source) in [
        ("numeric-test/input", format!("NumericF32Vector{i}"), true),
        (
            "numeric-test/weights",
            format!("NumericF32MatrixRef{i}x{o}"),
            true,
        ),
        ("numeric-test/sink", format!("NumericF32Vector{o}"), false),
    ] {
        let offer = fixture(name, &fixed_numeric_type(&ty).unwrap(), source);
        startup
            .insert(conduit_plot::KindSignature {
                kind: name.into(),
                startup_parameters: vec![],
            })
            .unwrap();
        profile
            .insert_kind(Kind {
                kind_id: offer.kind_id.clone(),
                kind_contract_revision: offer.kind_contract_revision.clone(),
                startup_parameters: vec![],
                shorthand: None,
                configuration: vec![],
                inputs: offer.inputs.clone(),
                outputs: offer.outputs.clone(),
                semantic_laws: vec![KindSemanticLaw::ValueContracts(
                    offer.semantic_contract.value_contracts().to_vec(),
                )],
                limits: offer.limits.clone(),
            })
            .unwrap();
        offers.push(offer);
    }
    offers.push(fixed_numeric_offer(&format!("numeric/linear{i}x{o}")).unwrap());
    let source=format!("plot linear-preparation {{\n input: numeric-test/input\n weights: numeric-test/weights\n linear: numeric/linear{i}x{o}\n sink: numeric-test/sink\n input.value >> linear.value\n weights.value >> linear.weights\n linear.result >> sink.value\n}}\n");
    let plot = conduit_plot::parse_with_startup(&source, &startup, &profile).unwrap();
    let hosts = [HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "numeric-test".into(),
        boot_id: "numeric-test-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "numeric-test@1".into(),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: offers,
    }];
    let placements = conduit_planner::default_placements(&plot, &hosts).unwrap();
    let plan = conduit_planner::plan_with_connection_limits(
        &plot,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        1,
        16384,
    )
    .unwrap();
    assert!(verify_plan(&plan));
    plan
}
#[test]
fn every_reviewed_linear_kind_prepares_through_its_exact_admitted_plan_cord() {
    let mut count = 0;
    for kind in fixed_numeric_contracts().unwrap() {
        if !kind
            .kind_contract_revision
            .as_str()
            .ends_with("/read-immutable-input@1")
        {
            continue;
        }
        let (i, o) = kind
            .kind_id
            .as_str()
            .strip_prefix("numeric/linear")
            .unwrap()
            .split_once('x')
            .unwrap();
        let i = i.parse::<usize>().unwrap();
        let o = o.parse::<usize>().unwrap();
        let plan = shape_plan(i, o);
        let packed = bytes(&vec![0.25; i * o]);
        let tensor = resource_tensor(&[i as u64, o as u64], &packed);
        let gear = plan.fragments[0]
            .placements
            .iter()
            .find(|g| g.kind_id == kind.kind_id)
            .unwrap();
        let back = prepare(&plan, &tensor, &packed, gear);
        assert_eq!(back.compact_f32_input(KPort(0)).unwrap().lanes as usize, i);
        assert_eq!(back.compact_f32_output().unwrap().1.lanes as usize, o);
        count += 1;
    }
    assert_eq!(count, 16);
}
