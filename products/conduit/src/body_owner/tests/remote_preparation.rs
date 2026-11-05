//! CPU signed-owner/receiver transaction, not a physical prepared Play.
use super::*;
use conduit_body::{
    HostCommandReceiveBounds, IssuedHostCommandChallenge, HOST_PREPARED_WORKSET_CONTRACT,
};
use conduit_embedded_build as build;

#[test]
fn current_owner_approves_exact_resident_workset_once_without_starting() {
    let source = crate::plot_source::parse(
        "type Reading = {\n value: U8\n}\nplot relay (\n reading: Reading...| >> echoed: Reading...|\n) = (.)\n",
    ).unwrap();
    let checked = source.check().unwrap();
    let expanded = source.expand_entry_for_authoring().unwrap();
    let conduit_core::ConfigurationValue::Text(text) =
        &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("expected checked expression program")
    };
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(text).unwrap();
    let image = build::PreparedResidentExpression::prepare(
        &checked,
        &program,
        conduit_core::PortTemporal::Flow { closes: true },
        build::ResidentExpressionBounds {
            native: conduit_plot::rust_binding::PreparedNativeBounds {
                canonical_bytes: 4096,
                scopes: 64,
                invariants: 64,
                contracts: 64,
            },
            maximum_occurrences: 4096,
            maximum_program_bytes: 512 * 1024,
            maximum_generated_bytes: 1024 * 1024,
            private_work_units: 1000,
        },
    )
    .unwrap();
    let local = StdHost::new_with_composition(
        StdHostConfig {
            host_id: "host/approval-owner".into(),
            boot_id: "boot/approval-owner".into(),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal(),
    );
    let (mut owner, root) = super::member_leave::owner_fixture_with_plot(local, &expanded);
    let ad = conduit_core::HostAdvertisement {
        declared_name: None,
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: "host/resident-member".into(),
        boot_id: "boot/resident-member".into(),
        offer_generation: OfferGeneration(1),
        profile: "fixture/resident-profile".into(),
        bases: vec![],
        resources: vec![conduit_core::resource_offer(
            "pool/resident-work",
            conduit_core::COOPERATIVE_STEP_WORK_CLASS_ID,
            1000,
        )],
        capabilities: vec![image.offer_description().clone()],
        planner_capabilities: vec![],
    };
    let (credential, _, ad) =
        super::member_leave::admit_advertisement(&mut owner, &root, "host/resident-member", ad);
    let before_plan = owner.session.evidence().clone();
    assert!(owner.plan_with_source(&source, &expanded).is_err());
    assert_eq!(owner.session.evidence(), &before_plan);
    let byte_capacity = program
        .maximum_prepared_input_bytes()
        .unwrap()
        .max(program.maximum_prepared_output_bytes().unwrap());
    let mut boundary = std::collections::BTreeMap::new();
    for (direction, bindings) in [
        (conduit_core::PortDirection::Input, &expanded.input_bindings),
        (
            conduit_core::PortDirection::Output,
            &expanded.output_bindings,
        ),
    ] {
        for binding in bindings {
            boundary.insert(
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity,
                },
            );
        }
    }
    for invalid in 0..3 {
        let mut changed = boundary.clone();
        match invalid {
            0 => {
                changed.pop_first();
            }
            1 => {
                changed.values_mut().next().unwrap().item_capacity = 0;
            }
            _ => {
                changed.values_mut().next().unwrap().byte_capacity = u32::MAX;
            }
        }
        assert!(owner
            .plan_workset_with_source_boundaries(
                &[(&source, &expanded)],
                &std::collections::BTreeMap::from([(resident(&expanded), changed)]),
            )
            .is_err());
        assert_eq!(owner.session.evidence(), &before_plan);
        assert!(owner.session.realization().is_none());
    }
    owner
        .plan_workset_with_source_boundaries(
            &[(&source, &expanded)],
            &std::collections::BTreeMap::from([(resident(&expanded), boundary)]),
        )
        .unwrap();
    let issuer = owner.prepare_current_member_carrier_receiver().unwrap();
    let member =
        crate::durable_host::owner::MemberPresentationBasis::new(credential.clone(), ad.clone())
            .unwrap();
    let slot = IssuedHostCommandChallenge::issue(
        &credential,
        &ad,
        &issuer,
        HostCommandReceiveBounds {
            payload_contract: HOST_PREPARED_WORKSET_CONTRACT,
            maximum_payload_bytes: 4096,
            lifetime_millis: 10000,
        },
        [7; 32],
        100,
    )
    .unwrap();
    let challenge = *slot.challenge();
    let limits = build::BodyFragmentProjectionLimits {
        image: build::EmbeddedImageBounds::HOST_TOOLING,
        computations: build::EmbeddedComputationBounds {
            maximum_program_bytes: 512 * 1024,
            maximum_total_program_bytes: 1024 * 1024,
        },
        assigned: conduit_core::AssignedPlanMaxima {
            encoded_bytes: 4096,
            runtime_state_bytes: 16384,
            counts: [32; conduit_core::ASSIGNED_PLAN_COUNT_KINDS],
        },
        maximum_fore_ports: 2,
        maximum_document_bytes: 4096,
    };
    let before = owner.session.evidence().clone();
    let proposal = owner.session.realization().unwrap().clone();
    for mutation in 0..15 {
        let mut changed = challenge;
        match mutation {
            0 => changed.body_id = "foreign-body",
            1 => changed.part_id = "foreign-part",
            2 => changed.credential_id = "foreign-credential",
            3 => changed.receiver_host = "foreign-host",
            4 => changed.receiver_boot = "foreign-boot",
            5 => changed.receiver_generation += 1,
            6 => changed.receiver_profile = "foreign-profile",
            7 => changed.issuer_host = "foreign-owner",
            8 => changed.issuer_boot = "foreign-owner-boot",
            9 => changed.issuer_generation += 1,
            10 => changed.issuer_key[0] ^= 1,
            11 => changed.payload_contract = "foreign-contract",
            12 => changed.maximum_payload_bytes = 8192,
            13 => changed.expires_at_millis = changed.issued_at_millis,
            _ => changed.nonce = [0; 32],
        }
        assert!(
            owner
                .approve_remote_resident_workset(&member, &changed, &[&image], limits)
                .is_err(),
            "substitution {mutation} accepted"
        );
    }
    let command = owner
        .approve_remote_resident_workset(&member, &challenge, &[&image], limits)
        .unwrap();
    assert_eq!(command.play().plan_id, proposal.plan.plan_id);
    assert_eq!(
        *command.transcript(),
        conduit_body::host_command_attribution_transcript(&challenge, command.payload()).unwrap()
    );
    let attributed = slot
        .authenticate(command.payload(), command.signature(), 101)
        .unwrap();
    assert_eq!(attributed.payload(), command.payload());
    assert!(owner
        .approve_remote_resident_workset(&member, &challenge, &[&image], limits)
        .is_err());
    assert_eq!(owner.session.evidence(), &before);
    assert_eq!(owner.session.realization(), Some(&proposal));
    assert!(owner.last_execution.is_none());
    owner.lull().unwrap();
    // Lull cannot silently release an approval that the receiver may retain.
    assert!(owner
        .approve_remote_resident_workset(&member, &challenge, &[&image], limits)
        .is_err());
    std::fs::remove_dir_all(root).unwrap();
}
