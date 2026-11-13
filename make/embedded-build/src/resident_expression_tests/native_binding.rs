//! Current Native selection and physical resource/authority correspondence.
use super::*;

#[test]
fn mixed_current_binding_requires_exact_complete_unambiguous_native_selection() {
    let (d, p) = context();
    let image = prepare(&d, &p);
    let plan = plan(&d, &p, &image, 1000);
    let fragment = &plan.fragments[0];
    let native = &fragment.placements[0];
    let offer = conduit_core::capability_offer_from_parts! {
        startup_parameters: vec![], shorthand: None,
        capability_id: native.capability_id.clone(), kind_id: native.kind_id.clone(),
        kind_contract_revision: native.kind_contract_revision.clone(),
        inputs: native.inputs.clone(), outputs: native.outputs.clone(),
        semantic_contract: native.semantic_contract.clone(),
        implementation: ImplementationOffer {
            execution_profile_id: native.execution_profile_id.clone(),
            implementation_id: native.implementation_id.clone(), artifact_id: native.artifact_id.clone(),
        },
        host_calls: native.host_calls.clone(), resource_requirements: vec![],
        authority_requirements: vec![], limits: native.limits.clone(),
    };
    let actual = HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        offer_generation: fragment.offer_generation,
        profile: "fixture/mixed-current".into(),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vec![offer.clone(), image.offer_description().clone()],
    };
    let bind = |actual: &HostAdvertisement,
                images: &[&PreparedResidentExpression],
                native: &[&CapabilityOffer]| {
        bind_current_mixed_resident_expressions(
            &plan,
            &fragment.fragment_id,
            actual,
            images,
            native,
        )
    };
    let bound = bind(&actual, &[&image], &[&offer]).unwrap();
    assert_eq!(bound.len(), 1);
    assert_eq!(bound[0].computation_requirement().node, 1);
    assert!(
        bind_current_resident_expressions(&plan, &fragment.fragment_id, &actual, &[&image])
            .is_err()
    );
    assert!(bind(&actual, &[&image], &[]).is_err());
    assert!(bind(&actual, &[&image], &[&offer, &offer]).is_err());
    assert!(bind(&actual, &[&image, &image], &[&offer]).is_err());
    assert!(bind(&actual, &[&image], &[&offer, image.offer_description()]).is_err());
    let mut extra = offer.clone();
    extra.capability_id = "fixture/unselected-native".into();
    assert!(bind(&actual, &[&image], &[&offer, &extra]).is_err());
    let mut changed = offer.clone();
    changed.limits.max_queue_bytes += 1;
    assert!(bind(&actual, &[&image], &[&changed]).is_err());
    for mutation in 0..5 {
        let mut current = actual.clone();
        match mutation {
            0 => current.boot_id = "fixture/stale-boot".into(),
            1 => current.offer_generation.0 += 1,
            2 => current.capabilities.push(offer.clone()),
            3 => current.capabilities[0] = changed.clone(),
            _ => {
                current.capabilities.remove(0);
            }
        }
        assert!(bind(&current, &[&image], &[&offer]).is_err());
    }
    // Updating both descriptive observations cannot silently add requirements
    // absent from the sealed placement, even at the same offer generation.
    for authority in [false, true] {
        let mut changed = offer.clone();
        if authority {
            changed.authority_requirements.push(AuthorityRequirement {
                contract_id: "fixture/new-authority".into(),
                host_call_contract_id: "fixture/native-call".into(),
                subject_kind: changed.kind_id.clone(),
            });
        } else {
            changed.resource_requirements.push(ResourceRequirement {
                class_id: "fixture/new-resource".into(),
                units: 1,
                protected_role: None,
                compute: None,
                content: None,
            });
        }
        let mut current = actual.clone();
        current.capabilities[0] = changed.clone();
        assert!(bind(&current, &[&image], &[&changed]).is_err());
    }
    assert!(bind(&actual, &[&image], &[&offer]).is_ok());
}

#[test]
fn native_binding_metadata_requires_exact_plain_resources_and_authority_scope() {
    let (d, p) = context();
    let image = prepare(&d, &p);
    let plan = plan(&d, &p, &image, 1000);
    let mut placement = plan.fragments[0].placements[0].clone();
    let mut offer = image.offer_description().clone();
    offer.authority_requirements = vec![AuthorityRequirement {
        contract_id: "fixture/native-authority".into(),
        host_call_contract_id: "fixture/native-call".into(),
        subject_kind: placement.kind_id.clone(),
    }];
    placement.resources = vec![ResourceBinding {
        pool_id: "fixture/native-pool".into(),
        class_id: offer.resource_requirements[0].class_id.clone(),
        units: offer.resource_requirements[0].units,
        protected: None,
        compute: None,
        content: None,
    }];
    placement.authority = vec![AuthorityBinding {
        grant_id: "fixture/independent-grant".into(),
        contract_id: offer.authority_requirements[0].contract_id.clone(),
        host_call_contract_id: offer.authority_requirements[0]
            .host_call_contract_id
            .clone(),
        subject_kind: placement.kind_id.clone(),
        host_id: placement.host_id.clone(),
        boot_id: placement.boot_id.clone(),
        capability_id: placement.capability_id.clone(),
    }];
    assert!(crate::resident_expression::native_bindings_correspond(
        &placement, &offer
    ));
    for mutation in 0..5 {
        let mut changed = offer.clone();
        match mutation {
            0 => changed.resource_requirements[0].units += 1,
            1 => changed.resource_requirements[0].class_id = "fixture/other-resource".into(),
            2 => changed.resource_requirements[0].protected_role = Some("fixture/protected".into()),
            3 => changed.authority_requirements[0].contract_id = "fixture/other-authority".into(),
            _ => changed
                .resource_requirements
                .push(changed.resource_requirements[0].clone()),
        }
        assert!(!crate::resident_expression::native_bindings_correspond(
            &placement, &changed
        ));
    }
    placement.authority[0].boot_id = "fixture/stale-boot".into();
    assert!(!crate::resident_expression::native_bindings_correspond(
        &placement, &offer
    ));
}
