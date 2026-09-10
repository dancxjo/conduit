//! Checked Forms and ordinary retained Body lifecycle for browser execution tests.
use super::super::*;

pub(in crate::form_runner) fn request() -> BodyStartRequest {
    request_from_sources(&[
        "form first {\n complete\n text: text/literal(\"first\")\n show: presentation/text\n text > show\n}\n",
        "form second {\n complete\n text: text/literal(\"second\")\n show: presentation/text\n text > show\n}\n",
    ])
}

pub(in crate::form_runner) fn request_from_sources(sources: &[&str]) -> BodyStartRequest {
    let (startup, catalog) = crate::installed_browser::catalogs().unwrap();
    let hosts = [crate::installed_browser::advertisement(
        "body-host".into(),
        "body-boot".into(),
    )];
    let plans = sources
        .iter()
        .map(|source| {
            let checked = conduit_form::check_syntax_document(
                &conduit_form::parse_syntax_document(source),
                &startup,
            )
            .unwrap();
            let entry = super::executable_entry(&checked).unwrap();
            let form = conduit_form::expand_canonical_form(&checked, &entry, &catalog).unwrap();
            let placements = default_expanded_placements(&form, &hosts).unwrap();
            plan_expanded_canonical_with_options(
                &form,
                &hosts,
                &placements,
                &local_bases(),
                PlanningOptions {
                    connection_bases: &BTreeMap::new(),
                    line_candidates: &BTreeMap::new(),
                    connection_item_capacity: 1,
                    connection_byte_capacity: crate::installed_browser::MAXIMUM_BROWSER_VALUE_BYTES
                        as u32,
                    authority_grants: &[],
                    protected_resource_grants: &[],
                    line_offers: &[],
                },
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let body = conduit_body::Body::born(
        plans[0].source_document_id.clone(),
        plans[0].checked_form_id.clone(),
        1,
        "sign/born".into(),
    )
    .unwrap();
    let mut history = conduit_body::BodyBiographyEvidence::born(
        body.clone(),
        conduit_body::BodyMembership::new(body.body_id.clone()).unwrap(),
        "Test Body".into(),
    )
    .unwrap();
    let mut body = body;
    for (index, plan) in plans.iter().enumerate().skip(1) {
        body = body
            .admit_form(
                conduit_body::ResidentForm::new(
                    plan.source_document_id.clone(),
                    plan.checked_form_id.clone(),
                ),
                format!("sign/admit-{index}").into(),
            )
            .unwrap();
        history
            .append_body_workload_events(
                body.clone(),
                &[(
                    format!("sign/admit-{index}").into(),
                    history.records.last().unwrap().sequence + 1,
                )],
            )
            .unwrap();
    }
    let (body, wake) = body.wake(1, "sign/wake".into()).unwrap();
    history
        .append_wake(
            body,
            wake.clone(),
            history.records.last().unwrap().sequence + 1,
        )
        .unwrap();
    let plan = BodyPlan::seal(
        &wake,
        plans
            .into_iter()
            .map(|plan| conduit_body::BodyFormPlan {
                form: conduit_body::ResidentForm::new(
                    plan.source_document_id.clone(),
                    plan.checked_form_id.clone(),
                ),
                plan,
            })
            .collect(),
    )
    .unwrap();
    let host = crate::installed_browser::advertisement("body-host".into(), "body-boot".into());
    let observations = host
        .resources
        .iter()
        .enumerate()
        .map(|(index, pool)| ResourceObservation {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            pool_id: pool.pool_id.clone(),
            class_id: pool.class_id.clone(),
            health: conduit_core::ResourceHealth::Ready,
            unreserved_units: pool.capacity_units,
            utilized_units: 0,
            sign_id: format!("sign/observed-{index}").into(),
        })
        .collect();
    BodyStartRequest {
        wake,
        plan,
        play_sequence: 7,
        observations,
        body_evidence: Some(history),
    }
}
