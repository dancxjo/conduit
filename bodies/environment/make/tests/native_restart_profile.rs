extern crate alloc;
#[path = "../../../../architecture/core/tests/common/sealed_state.rs"]
mod core_fixture;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_restart_profile.rs"]
mod profile;
use conduit_body::{BodyPlotPlan, SealedBodyLifecycleSession};
use conduit_core as c;
use serde::Deserialize;
#[derive(Deserialize)]
struct ReadEvidence(
    #[serde(deserialize_with = "profile::deserialize_evidence")]
    conduit_body::BodyBiographyEvidence,
);
#[derive(Deserialize)]
struct ReadArchives(
    #[serde(deserialize_with = "profile::deserialize_archives")]
    Vec<conduit_body::BodyBiographyArchiveSegment>,
);
fn installed() -> (
    SealedBodyLifecycleSession,
    Vec<BodyPlotPlan>,
    c::HostAdvertisement,
) {
    let sign = c::bind_sign(
        &"construction/host".into(),
        &"construction/boot".into(),
        None,
        1,
    );
    let image =
        conduit_environment_make::EnvironmentBodyImage::prepare_mask_from_sign(&sign).unwrap();
    let fragment = core_fixture::fragment();
    let host = c::HostAdvertisement {
        declared_name: None,
        protocol_version: c::PROTOCOL_VERSION,
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        offer_generation: fragment.offer_generation,
        profile: "fixture/root".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let evidence = image.body.evidence().clone();
    let plans = evidence
        .body
        .workset
        .plots()
        .iter()
        .map(|resident| {
            let mut fragment = core_fixture::fragment();
            fragment.source_document_id = resident.source_document_id.clone();
            fragment.checked_plot_id = resident.checked_plot_id.clone();
            fragment.expanded_plot_id =
                format!("fixture/expanded/{}", resident.checked_plot_id.as_str()).into();
            BodyPlotPlan {
                plot: resident.clone(),
                plan: core_fixture::seal(fragment),
            }
        })
        .collect();
    (
        conduit_environment_runtime::body_image::open_installed_image_sealed(evidence, &host)
            .unwrap(),
        plans,
        host,
    )
}

fn parity(evidence: &conduit_body::BodyBiographyEvidence) {
    let canonical = serde_json::to_vec(evidence).unwrap();
    let profile = serde_json::to_vec(&profile::EvidenceRef::from(evidence)).unwrap();
    assert_eq!(canonical, profile);
    assert_eq!(
        serde_json::from_slice::<ReadEvidence>(&canonical)
            .unwrap()
            .0,
        *evidence
    );
}
#[test]
fn complete_two_plot_play_and_restart_preserve_every_field() {
    let (mut owner, plans, host) = installed();
    parity(owner.evidence());
    owner.propose(plans, &host.host_id, &host.boot_id).unwrap();
    let realization = owner.realization().unwrap();
    let play = conduit_body::BodyPlayIdentity::bind(&realization.plan, 1);
    let wake = realization
        .wake
        .body_plan_ready(
            &realization.plan,
            c::bind_sign(&host.host_id, &host.boot_id, Some(&play.active_play_id), 0).sign_id,
        )
        .unwrap()
        .body_play_started(
            &realization.plan,
            &play,
            c::bind_sign(&host.host_id, &host.boot_id, Some(&play.active_play_id), 1).sign_id,
        )
        .unwrap();
    owner
        .started(&host.host_id, &host.boot_id, play, wake)
        .unwrap();
    parity(owner.evidence());
    let restored = SealedBodyLifecycleSession::resume_here(
        owner.evidence().clone(),
        &host.host_id,
        &"boot/new".into(),
    )
    .unwrap();
    parity(restored.evidence());
    let archive = conduit_body::BodyBiographyArchiveSegment {
        schema: "conduit.body/biography-archive-segment@2".into(),
        body_id: owner.evidence().body_id.clone(),
        ordinal: 1,
        previous_digest: None,
        records: owner.evidence().records.clone(),
        wakes: owner.evidence().wakes.clone(),
        body_events: owner.evidence().body.events.clone(),
        membership_events: owner.evidence().membership.events.clone(),
        digest: [13; 32],
    };
    assert_eq!(
        serde_json::to_vec(&archive).unwrap(),
        serde_json::to_vec(&profile::ArchiveRef::from(&archive)).unwrap()
    );
    let bytes = serde_json::to_vec(&vec![archive.clone()]).unwrap();
    assert_eq!(
        serde_json::from_slice::<ReadArchives>(&bytes).unwrap().0,
        vec![archive]
    );
    assert!(profile::ensure_no_holds(owner.evidence(), &[]).is_ok());
}
#[test]
fn held_payload_refused_before_even_parsing_its_value() {
    // Use a valid real shape up to hold so malformed rich content cannot mask refusal.
    let (mut owner, plans, host) = installed();
    owner.propose(plans, &host.host_id, &host.boot_id).unwrap();
    let realization = owner.realization().unwrap();
    let wake = realization
        .wake
        .body_plan_ready(&realization.plan, "profile/plan-ready".into())
        .unwrap();
    let mut evidence = owner.evidence().clone();
    evidence.wakes.push(wake);
    let canonical = String::from_utf8(serde_json::to_vec(&evidence).unwrap()).unwrap();
    assert!(canonical.contains("\"hold\":null"));
    let held = canonical.replacen("\"hold\":null", "\"hold\":{\"plan\":INVALID}", 1);
    let error = serde_json::from_slice::<ReadEvidence>(held.as_bytes())
        .err()
        .unwrap()
        .to_string();
    assert!(
        error.contains("native restart HOLD is unsupported"),
        "{error}"
    );
    let missing = canonical.replace(",\"hold\":null", "");
    assert_eq!(
        serde_json::from_slice::<ReadEvidence>(missing.as_bytes())
            .unwrap()
            .0,
        evidence
    );
    let unknown = canonical.replacen("\"schema\":", "\"unrecognized\":0,\"schema\":", 1);
    assert!(serde_json::from_slice::<ReadEvidence>(unknown.as_bytes()).is_err());
}
#[test]
fn nonempty_hold_refused_on_both_live_and_archived_save_paths() {
    let (mut owner, plans, host) = installed();
    owner.propose(plans, &host.host_id, &host.boot_id).unwrap();
    let wake_id = owner.realization().unwrap().wake.wake_id.clone();
    let mut evidence = owner.evidence().clone();
    let workset = evidence.body.workset.clone();
    let held = conduit_body::PlanHold {
        plan: core_fixture::seal(core_fixture::fragment()),
        basis: conduit_body::PlanningBasis::new(vec!["profile/basis".into()]).unwrap(),
        policy: conduit_body::HoldPolicy {
            reason: "profile/reason".into(),
            source: "profile/source".into(),
            release_authority: conduit_body::HoldReleaseAuthority::new("profile/release".into()),
            hold_replacement_plan: false,
        },
    };
    let wake = conduit_body::Wake {
        wake_id,
        body_id: evidence.body_id.clone(),
        workset,
        workload_revision: 0,
        wake_sequence: 2,
        lifecycle: conduit_body::WakeLifecycle::Held,
        plans: vec![conduit_body::WakePlan {
            plan_id: held.plan.plan_id.clone(),
            active_play_id: None,
            state: conduit_body::WakePlanState::Held,
            hold: Some(held),
        }],
        rejections: vec![],
        sign_ids: vec![],
        events: vec![],
    };
    evidence.wakes.push(wake.clone());
    assert_eq!(
        profile::ensure_no_holds(&evidence, &[]),
        Err("restart-hold-unsupported")
    );
    assert!(serde_json::to_vec(&profile::EvidenceRef::from(&evidence)).is_err());
    evidence.wakes.clear();
    let archive = conduit_body::BodyBiographyArchiveSegment {
        schema: "conduit.body/biography-archive-segment@2".into(),
        body_id: evidence.body_id.clone(),
        ordinal: 1,
        previous_digest: None,
        records: evidence.records.clone(),
        wakes: vec![wake],
        body_events: vec![],
        membership_events: vec![],
        digest: [0; 32],
    };
    assert_eq!(
        profile::ensure_no_holds(&evidence, core::slice::from_ref(&archive)),
        Err("restart-hold-unsupported")
    );
    assert!(serde_json::to_vec(&profile::ArchiveRef::from(&archive)).is_err());
    let encoded = serde_json::to_vec(&vec![archive]).unwrap();
    assert!(serde_json::from_slice::<ReadArchives>(&encoded).is_err());
}
