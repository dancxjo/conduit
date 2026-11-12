//! Original owner signature over a real sealed schema-8 selected-Fore document.
//! CPU fixture only: no physical provider, native Ready or execution claim.
use super::*;
use conduit_core as c;
use std::collections::BTreeMap;

pub(super) fn run(
    source: &crate::plot_source::CanonicalSource,
    expanded: &conduit_plot::ExpandedAuthoringPlot,
    image: &build::PreparedResidentExpression,
    ad: &c::HostAdvertisement,
    boundaries: &BTreeMap<conduit_planner::ForeBoundaryKey, conduit_planner::ConnectionQueueLimits>,
    limits: build::BodyFragmentProjectionLimits,
) {
    let local = StdHost::new_with_composition(
        StdHostConfig {
            host_id: "host/signed-line-owner".into(),
            boot_id: "boot/signed-line-owner".into(),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal(),
    );
    let (mut owner, root) = super::super::member_leave::owner_fixture_with_plot(local, expanded);
    let (credential, _, ad) = super::super::member_leave::admit_advertisement(
        &mut owner,
        &root,
        ad.host_id.as_str(),
        ad.clone(),
    );
    let partitions = fore_line_planning::verify_current_member_line_selection(
        &owner, source, expanded, boundaries, &ad,
    );
    let local = owner.host.advertisement();
    owner
        .session
        .propose(partitions.clone(), &local.host_id, &local.boot_id)
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
            maximum_payload_bytes: limits.maximum_document_bytes,
            lifetime_millis: 10000,
        },
        [5; 32],
        100,
    )
    .unwrap();
    let challenge = *slot.challenge();
    let plot = resident(expanded);
    let fore = partitions[0]
        .plan
        .fragments
        .iter()
        .flat_map(|f| &f.fore_ports)
        .find(|f| f.selected_line.is_some())
        .unwrap();
    let session = || build::SelectedForeSession {
        front_port_id: fore.front_port_id.clone(),
        direction: fore.direction,
        track: fore.track,
        session: c::AssignedIdentity::from_text("fixture/exact-session"),
        session_contract: c::AssignedIdentity::from_text("fixture/physical-contract"),
    };
    let before = owner.session.evidence().clone();
    let proposal = owner.session.realization().unwrap().clone();
    // Legacy cannot downgrade a selected Line, and failed session selections
    // never occupy an approval slot or change the original Body proposal.
    assert!(owner
        .approve_remote_workset(&member, slot.challenge(), &[image], &[], limits)
        .is_err());
    for case in 0..4 {
        let mut row = session();
        if case == 0 {
            row.front_port_id = "foreign".into();
        }
        if case == 1 {
            row.direction = c::PortDirection::Output;
        }
        let key = if case == 2 {
            conduit_body::ResidentPlot::new("foreign/source".into(), "foreign/checked".into())
        } else {
            plot.clone()
        };
        let rows = if case == 3 {
            vec![row, session()]
        } else {
            vec![row]
        };
        let sessions = BTreeMap::from([(key, rows)]);
        assert!(owner
            .approve_remote_workset_with_line_fore(
                &member,
                slot.challenge(),
                &[image],
                &[],
                limits,
                &sessions
            )
            .is_err());
        assert_eq!(owner.session.evidence(), &before);
        assert_eq!(owner.session.realization(), Some(&proposal));
    }
    let sessions = BTreeMap::from([(plot, vec![session()])]);
    let command = owner
        .approve_remote_workset_with_line_fore(
            &member,
            slot.challenge(),
            &[image],
            &[],
            limits,
            &sessions,
        )
        .unwrap();
    let payload = command.payload().to_vec();
    let signature = *command.signature();
    let play = command.play().clone();
    assert_eq!(play.plan_id, proposal.plan.plan_id);
    let authenticated = slot.authenticate(&payload, &signature, 101).unwrap();
    assert_eq!(authenticated.payload(), payload);
    assert_eq!(&payload[..8], b"CBWSET01");
    assert_eq!(u16::from_le_bytes(payload[8..10].try_into().unwrap()), 1);
    let n = u32::from_le_bytes(payload[10..14].try_into().unwrap()) as usize;
    assert_eq!(payload.len(), 14 + n);
    let (metadata, projection) =
        conduit_body::decode_body_fragment_document(&payload[14..]).unwrap();
    assert_eq!(metadata.body_plan, play.plan_id.as_str());
    assert_eq!(metadata.body_play, play.active_play_id.as_str());
    assert_eq!(
        u16::from_le_bytes([projection[8], projection[9]]),
        c::ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA
    );
    let plan = &partitions[0].plan;
    let generated = build::generate_line_fore_native_plan(
        plan,
        &plan.fragments[0].fragment_id,
        limits.image,
        limits.computations,
        limits.maximum_fore_ports,
        &[session()],
    )
    .unwrap();
    let computations = build::assigned_computation_requirements(generated.kernel()).unwrap();
    let resources: Vec<_> = generated
        .kernel()
        .resources
        .iter()
        .map(|r| r.resource)
        .collect();
    let host_calls: Vec<_> = generated
        .kernel()
        .host_calls
        .iter()
        .map(|call| c::AssignedIdentity::from_text(&call.contract_id))
        .collect();
    c::decode_assigned_plan_with_line_fore(
        projection,
        limits.assigned,
        c::AssignedPlanRequirements {
            host: c::AssignedIdentity::from_text(ad.host_id.as_str()),
            boot: c::AssignedIdentity::from_text(ad.boot_id.as_str()),
            host_calls: &host_calls,
            resources: &resources,
            remote_bindings: generated.remote_bindings(),
        },
        c::AssignedComputationProfile {
            maximum_text_bytes: 65535,
            maximum_total_text_bytes: u32::MAX,
            requirements: &computations,
        },
        generated.profile(),
    )
    .unwrap();
    let inventory: Vec<_> = generated
        .fore()
        .iter()
        .flat_map(|f| f.payload().iter().copied())
        .collect();
    assert_eq!(metadata.fore_ports, inventory);
    assert!(owner
        .approve_remote_workset_with_line_fore(
            &member,
            &challenge,
            &[image],
            &[],
            limits,
            &sessions
        )
        .is_err());
    assert_eq!(owner.session.evidence(), &before);
    assert_eq!(owner.session.realization(), Some(&proposal));
    assert!(owner.require_remote_workset_retired().is_err());
    std::fs::remove_dir_all(root).unwrap();
}
