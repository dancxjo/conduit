//! Synthetic canonical operation facts, never a claim of native possession.
use conduit_body::*;
use conduit_core as c;
pub fn configure(plan: &mut c::Plan, operations: usize) {
    let p = &mut plan.fragments[0].placements[0];
    p.base = Some(c::BaseProviderBinding {
        base_id: "record/base".into(),
        provider_instance_id: "record/old".into(),
        provider_generation: 1,
        implementation_id: "record/implementation".into(),
        mechanism_family: "record/mechanism".into(),
        enforcement_class: c::BaseEnforcementClass::Cooperative,
    });
    for (i, operation) in ["read", "close"].iter().take(operations).enumerate() {
        let call = format!("record/{operation}@1");
        p.host_calls.push(c::HostCallRequirement {
            contract_id: call.clone().into(),
            target_kind: Some(format!("record/{operation}-subject").into()),
            maximum_in_flight: 1,
            maximum_input_bytes: 8,
            maximum_output_bytes: 8,
        });
        p.authority.push(c::AuthorityBinding {
            grant_id: format!("old/{i}").into(),
            contract_id: format!("authority/{operation}@1").into(),
            host_call_contract_id: call.into(),
            subject_kind: format!("record/{operation}-subject").into(),
            host_id: p.host_id.clone(),
            boot_id: p.boot_id.clone(),
            capability_id: p.capability_id.clone(),
        });
    }
    *plan = c::seal_plan(
        c::PlotIdentity {
            source_document_id: plan.source_document_id.clone(),
            checked_plot_id: plan.checked_plot_id.clone(),
            expanded_plot_id: plan.expanded_plot_id.clone(),
        },
        plan.fragments.clone(),
    );
}
pub fn refresh(p: &mut c::PlannedGear, boot: &str) {
    if let Some(base) = &mut p.base {
        base.provider_instance_id = "record/fresh".into();
        base.provider_generation = 2;
    }
    for (i, a) in p.authority.iter_mut().enumerate() {
        a.boot_id = boot.into();
        // Fresh names sort oppositely: correspondence MUST remain pinned recipe order.
        a.grant_id = format!("fresh/{}", 9 - i).into();
    }
}
pub fn rows(document: &[u8]) -> Vec<c::FreshAssignedNative> {
    let (_, projection) = decode_body_fragment_document(document).unwrap();
    let mut rows: Vec<_> = c::AssignedPlanRecords::new(projection)
        .unwrap()
        .map(Result::unwrap)
        .filter(|r| r.tag == c::ASSIGNED_HOST_CALL)
        .map(|r| {
            let binding = c::assigned_native_binding(r.payload).unwrap().unwrap();
            c::FreshAssignedNative {
                node: u16::from_le_bytes(r.payload[..2].try_into().unwrap()),
                call: u16::from_le_bytes(r.payload[2..4].try_into().unwrap()),
                contract: binding.call,
                binding,
            }
        })
        .collect();
    rows.reverse();
    rows
}
pub fn refusals(
    rows: &[c::FreshAssignedNative],
    recipe: &[c::FragmentSealPiece<'_>],
    mut refuses: impl FnMut(&[c::FreshAssignedNative]) -> bool,
) {
    for mutation in 0..9 {
        let mut changed = rows.to_vec();
        let id = c::AssignedIdentity::from_text;
        match mutation {
            0 => {
                changed.pop();
            }
            1 => changed.push(rows[0]),
            2 => changed[0].binding.grant = id("wrong/grant"),
            3 => changed[0].binding.boot = id("stale/boot"),
            4 => changed[0].binding.instance = id("other/instance"),
            5 => changed[0].binding.generation += 1,
            6 => changed[0].binding.authority = id("wrong/authority"),
            7 => changed[0].binding.subject = id("wrong/subject"),
            _ => changed[0].binding.authority_capability = id("wrong/capability"),
        }
        assert!(refuses(&changed), "mutation {mutation}");
    }
    if rows.len() == 2 {
        let mut changed = rows.to_vec();
        changed[0].binding.grant = rows[1].binding.grant;
        changed[1].binding.grant = rows[0].binding.grant;
        assert!(refuses(&changed), "sibling grants swapped");
        changed[0].binding.grant = changed[1].binding.grant;
        assert!(refuses(&changed), "one grant reused for two operations");
    }
    for piece in recipe {
        if let c::FragmentSealPiece::Field(field, _) = piece {
            if matches!(
                field.role,
                c::FragmentScopeRole::AuthorityGrant
                    | c::FragmentScopeRole::AuthorityHost
                    | c::FragmentScopeRole::AuthorityBoot
            ) {
                let key = continuation_recipe_authority(recipe, *field).unwrap();
                assert_eq!(
                    rows.iter()
                        .filter(|n| n.binding.authority == key.authority
                            && n.binding.call == key.call
                            && n.binding.subject == key.subject
                            && n.binding.capability == key.capability)
                        .count(),
                    1
                );
                let malformed = c::FragmentSealField {
                    entry: 500,
                    ..*field
                };
                assert!(continuation_recipe_authority(recipe, malformed).is_err());
                let mut nontextual = recipe.to_vec();
                let index = nontextual
                    .iter()
                    .position(|p| matches!(p, c::FragmentSealPiece::Field(f, _) if f == field))
                    .unwrap();
                nontextual[index] = c::FragmentSealPiece::Field(*field, false);
                assert!(continuation_recipe_authority(&nontextual, *field).is_err());
                let mut truncated = recipe.to_vec();
                if field.role == c::FragmentScopeRole::AuthorityGrant {
                    truncated[index + 1] = c::FragmentSealPiece::Literal(&[255, 255, 255, 255]);
                    assert!(continuation_recipe_authority(&truncated, *field).is_err());
                }
                let mut duplicate = recipe.to_vec();
                duplicate.push(*piece);
                assert!(continuation_recipe_authority(&duplicate, *field).is_err());
            }
        }
    }
}
