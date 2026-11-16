//! Actual local signed membership/session custody; no physical display claim.
use super::super::member_leave::owner_fixture_with_host;
use super::*;

fn image_owner(heap: usize) -> (Owner, std::path::PathBuf) {
    let source = format!(
        r#"host bounded-viewer-owner {{
  schema = 1
  target = {{architecture: "x86_64", machine: "computer", os: "linux"}}
  base = {{kind: "timer/monotonic", implementation: "hosted/monotonic-clock@1"}}
  capability = {{kind: "time/tick", pinned_implementation: "std/kernel-time-tick@2"}}
  need = {{id: "timer/main", class: "conduit.resource/timer-slot@1", slots: 1, bytes: 512}}
  limits = {{static_memory_bytes: 16777216, heap_arena_bytes: {heap}, queue_items: 4096, buffered_bytes: 16777216, active_instances: 512, operation_slots: 256, timer_slots: 128, line_sessions: 64, evidence_items: 4096}}
}}
"#
    );
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = conduit_host_make::check_host_configuration(
        conduit_host_make::parse_host_configuration_conduit(&source).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let image = conduit_host_make::build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &conduit_host_make::BuildInputs {
            source_identity: "fixture/bounded-native-viewers".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0;
    let host = StdHost::from_selected_timer_image(
        StdHostConfig {
            host_id: "fixture/viewer-owner".into(),
            boot_id: "fixture/viewer-owner/boot".into(),
            offer_generation: OfferGeneration(1),
        },
        &image,
        &catalog,
    )
    .unwrap();
    let (mut owner, root) = owner_fixture_with_host(host);
    owner.prepare_current_member_carrier_receiver().unwrap();
    (owner, root)
}

#[test]
fn two_original_image_carriers_coexist_and_retire_independently() {
    let (mut owner, root) = image_owner(1_048_576);
    let (a, key_a, ad_a) = admit_with_key(&mut owner, &root, "fixture/viewer-a");
    let (b, key_b, ad_b) = admit_with_key(&mut owner, &root, "fixture/viewer-b");
    let (c, _, ad_c) = admit_with_key(&mut owner, &root, "fixture/viewer-c");
    let (challenge_a, _) = begin(&mut owner, &a, &ad_a);
    owner
        .complete_current_member_carrier(&challenge_a, signature(&challenge_a, &key_a))
        .unwrap();
    owner
        .renew_current_member_presence(&challenge_a.session_id, 1)
        .unwrap();
    let (challenge_b, _) = begin(&mut owner, &b, &ad_b);
    owner
        .complete_current_member_carrier(&challenge_b, signature(&challenge_b, &key_b))
        .unwrap();
    owner
        .renew_current_member_presence(&challenge_b.session_id, 1)
        .unwrap();
    for challenge in [&challenge_a, &challenge_b] {
        assert!(
            owner
                .inspect_current_member_carrier(&challenge.session_id)
                .unwrap()
                .membership_current
        );
        owner
            .verify_native_viewer_image_custody(&challenge.session_id)
            .unwrap();
    }
    assert!(owner
        .begin_current_member_carrier(
            &c,
            &ad_c,
            "fixture/third-line".into(),
            LineSecurity::AuthenticatedEncrypted,
            crate::durable_host::current_time_millis().unwrap() + 60_000,
            128
        )
        .is_err());
    // A different signed context cannot consume either original issuance.
    assert!(owner
        .complete_current_member_carrier(&challenge_a, signature(&challenge_b, &key_b))
        .is_err());
    owner
        .renew_current_member_presence(&challenge_a.session_id, 2)
        .unwrap();
    owner
        .renew_current_member_presence(&challenge_b.session_id, 2)
        .unwrap();
    owner
        .retire_current_member_carrier(&challenge_a.session_id)
        .unwrap();
    assert!(owner
        .inspect_current_member_carrier(&challenge_a.session_id)
        .is_err());
    assert!(owner
        .inspect_current_member_carrier(&challenge_b.session_id)
        .is_ok());
    owner
        .verify_native_viewer_image_custody(&challenge_b.session_id)
        .unwrap();
    owner
        .retire_current_member_carrier(&challenge_b.session_id)
        .unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn insufficient_original_image_budget_refuses_before_issuing_any_viewer() {
    let (mut owner, root) = image_owner(8192);
    let (credential, _, ad) = admit_with_key(&mut owner, &root, "fixture/pressured-viewer");
    let before = owner.session.evidence().clone();
    let error = owner
        .begin_current_member_carrier(
            &credential,
            &ad,
            "fixture/line".into(),
            LineSecurity::AuthenticatedEncrypted,
            crate::durable_host::current_time_millis().unwrap() + 60_000,
            128,
        )
        .unwrap_err();
    assert_eq!(
        error,
        "selected IMAGE managed heap backing capacity is unavailable"
    );
    // A failed reservation must not occupy issuance: a second attempt reaches
    // the same capacity gate rather than refusing an occupied challenge slot.
    let retry = owner
        .begin_current_member_carrier(
            &credential,
            &ad,
            "fixture/line".into(),
            LineSecurity::AuthenticatedEncrypted,
            crate::durable_host::current_time_millis().unwrap() + 60_000,
            128,
        )
        .unwrap_err();
    assert_eq!(retry, error);
    assert_eq!(owner.session.evidence(), &before);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn two_selected_image_views_keep_independent_acknowledgements() {
    let (mut owner, root) = image_owner(67_108_864);
    let mut viewers = Vec::new();
    let members: Vec<_> = ["fixture/native-a", "fixture/native-b"]
        .into_iter()
        .map(|name| super::presentation::native_member_named(&mut owner, &root, true, name))
        .collect();
    for (index, (credential, key, ad)) in members.into_iter().enumerate() {
        let (challenge, _) = begin(&mut owner, &credential, &ad);
        owner
            .complete_current_member_carrier(&challenge, signature(&challenge, &key))
            .unwrap();
        owner
            .renew_current_member_presence(&challenge.session_id, 1)
            .unwrap();
        let basis =
            crate::durable_host::owner::MemberPresentationBasis::new(credential.clone(), ad)
                .unwrap();
        let (face, returned, input) =
            crate::durable_host::routed_join::native_lines::offer_graphical(
                owner.host.advertisement(),
                &basis,
                challenge.session_id.as_str(),
            )
            .unwrap();
        let seal = owner
            .select_member_carrier_mask_route_with_interaction(
                &challenge.session_id,
                &face,
                &returned,
                input.as_ref(),
            )
            .unwrap();
        let shown = super::presentation::show(&mut owner, &seal, index as u64 + 1);
        let request = super::presentation::request(&credential);
        owner
            .acknowledge_member_carrier_show(&challenge.session_id, &request, &shown)
            .unwrap();
        viewers.push((challenge.session_id, request, shown));
    }
    for (session, request, shown) in &viewers {
        owner.select_native_viewer_session(session).unwrap();
        owner.validate_native_mask_show(request, shown).unwrap();
        owner.verify_native_viewer_image_custody(session).unwrap();
    }
    assert!(owner
        .acknowledge_member_carrier_show(&viewers[0].0, &viewers[1].1, &viewers[1].2)
        .is_err());
    owner.retire_current_member_carrier(&viewers[1].0).unwrap();
    owner.select_native_viewer_session(&viewers[0].0).unwrap();
    owner
        .validate_native_mask_show(&viewers[0].1, &viewers[0].2)
        .unwrap();
}
