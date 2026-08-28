mod common;

use conduit_body::{Body, BodyResourceAllowance, BodyResourceEnvelope, PartId};
use conduit_core::{CheckedFormId, ResourceHealth, ResourceObservation, SignId, SourceDocumentId};
use conduit_planner::{default_placements, plan_with_resource_allowances, PlannerError};

#[test]
fn ordinary_planning_cannot_exceed_body_allowance_despite_host_capacity() {
    let form = common::generate_text_form();
    let mut host = conduit_ai::generate_text_base_fixtures()[0]
        .advertisement
        .clone();
    let requirement = &mut host.capabilities[0].resource_requirements[0];
    requirement.units = 4;
    requirement.compute = None;
    let pool = host
        .resources
        .iter_mut()
        .find(|pool| pool.class_id == requirement.class_id)
        .unwrap();
    pool.capacity_units = 8;
    pool.compute = None;
    let pool_id = pool.pool_id.clone();

    let body = Body::born(
        SourceDocumentId::from("body-envelope-source"),
        CheckedFormId::from("body-envelope-form"),
        1,
        SignId::from("body-born"),
    )
    .unwrap();
    let part = PartId::bind(&body.body_id, "workstation-part", 1).unwrap();
    let mut allowances = host
        .resources
        .iter()
        .map(|resource| BodyResourceAllowance {
            pool_id: resource.pool_id.clone(),
            class_id: resource.class_id.clone(),
            maximum_units: resource.capacity_units,
        })
        .collect::<Vec<_>>();
    allowances.sort_by(|left, right| {
        (&left.pool_id, &left.class_id).cmp(&(&right.pool_id, &right.class_id))
    });
    allowances
        .iter_mut()
        .find(|allowance| allowance.pool_id == pool_id)
        .unwrap()
        .maximum_units = 2;
    let envelope = BodyResourceEnvelope::new(
        body.body_id.clone(),
        part.clone(),
        &host,
        allowances.clone(),
    )
    .unwrap();
    let observations = host
        .resources
        .iter()
        .map(|resource| ResourceObservation {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            pool_id: resource.pool_id.clone(),
            class_id: resource.class_id.clone(),
            health: ResourceHealth::Ready,
            unreserved_units: resource.capacity_units,
            utilized_units: 0,
            sign_id: SignId::from(format!("{}-unreserved", resource.pool_id.as_str())),
        })
        .collect::<Vec<_>>();
    let hosts = vec![host.clone()];
    let placements = default_placements(&form, &hosts).unwrap();
    assert!(matches!(
        plan_with_resource_allowances(
            &form,
            &hosts,
            &placements,
            &[],
            core::slice::from_ref(&envelope.planning_allowances()),
            &observations,
        ),
        Err(PlannerError::ResourceAllowanceUnsatisfied(_))
    ));

    host.capabilities[0].resource_requirements[0].units = 2;
    let hosts = vec![host.clone()];
    let placements = default_placements(&form, &hosts).unwrap();
    let plan = plan_with_resource_allowances(
        &form,
        &hosts,
        &placements,
        &[],
        &[envelope.planning_allowances()],
        &observations,
    )
    .expect("the exact two-unit requirement fits the Body allowance");
    assert_eq!(plan.fragments[0].placements[0].resources[0].units, 2);

    allowances
        .iter_mut()
        .find(|allowance| allowance.pool_id == pool_id)
        .unwrap()
        .maximum_units = 4;
    let expanded = BodyResourceEnvelope::new(body.body_id, part, &host, allowances).unwrap();
    assert_ne!(envelope.envelope_id(), expanded.envelope_id());
    host.capabilities[0].resource_requirements[0].units = 4;
    let hosts = vec![host];
    let placements = default_placements(&form, &hosts).unwrap();
    let expanded_plan = plan_with_resource_allowances(
        &form,
        &hosts,
        &placements,
        &[],
        &[expanded.planning_allowances()],
        &observations,
    )
    .expect("a new planning attempt can use the expanded allowance");
    assert_eq!(
        expanded_plan.fragments[0].placements[0].resources[0].units,
        4
    );
    assert_eq!(plan.fragments[0].placements[0].resources[0].units, 2);
}
