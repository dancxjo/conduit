use super::*;
use conduit_core::{
    kind_id, BootId, HostId, OfferGeneration, ResourceAccessMode, ResourceContentRequirement,
    ResourceRetention, ResourceSemanticIdentity, ResourceSharing, ResourceVersionIdentity,
};
use conduit_std_host::todo_durable_resource::MissingV2Disposition;
use conduit_std_host::{StdHost, StdHostConfig};

const SOURCE: &str = include_str!("../../../../plots/todo/checkpoint-once.conduit");

fn selected_host(root: &Path) -> StdHost {
    StdHost::new_for_todo_checkpoint_once(
        StdHostConfig {
            host_id: HostId::from("host/todo-owner-test"),
            boot_id: BootId::from("boot/todo-owner-test"),
            offer_generation: OfferGeneration(1),
        },
        root,
        ResourceContentRequirement {
            identity: ResourceSemanticIdentity::from_digest([1; 32]),
            version: ResourceVersionIdentity::from_digest([2; 32]),
            content_profile: kind_id("conduit.todo/checkpoint-envelope@1"),
            maximum_bytes: conduit_std_offers::TODO_CHECKPOINT_MAX_BYTES,
            maximum_items: 1,
            retention: ResourceRetention::ExternalDurable,
            sharing: ResourceSharing::SingleWriterPublished,
            access: ResourceAccessMode::WriteCandidatePublish,
            generation_slots: 1,
            reader_leases: 1,
            publication_slots: 1,
            sensitive: false,
        },
    )
    .unwrap()
}

fn fixture() -> (
    Owner,
    crate::plot_source::CanonicalSource,
    conduit_plot::ExpandedAuthoringPlot,
    AuthorityGrant,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let state_root = std::env::temp_dir().join(super::super::super::super::fresh_identity(
        "owner-todo-checkpoint",
        "first-action",
    ));
    let checkpoint_root = state_root.join("checkpoint");
    std::fs::create_dir_all(&checkpoint_root).unwrap();
    let installation = super::super::super::super::Installation {
        schema: super::super::super::super::INSTALL_SCHEMA.into(),
        host_id: "host/todo-owner-test".into(),
        release_source_identity: "source/test".into(),
        release_bundle_sha256: super::super::super::super::digest(b"bundle/test"),
        product_executable: "fixture-unused".into(),
        body_state: None,
        joined_body_state: None,
        selected_speech: None,
        selected_model: None,
    };
    super::super::super::super::write_json_atomic(
        &state_root.join("installation.json"),
        &installation,
    )
    .unwrap();
    let source = crate::plot_source::parse(SOURCE).unwrap();
    let plot = source.expand_entry_for_authoring().unwrap();
    assert_eq!(plot.expanded.name, "todo/checkpoint-once");
    let host = selected_host(&checkpoint_root);
    let ad = host.advertisement();
    let offer = ad
        .capabilities
        .iter()
        .find(|offer| {
            offer.implementation.implementation_id.as_str()
                == conduit_std_offers::TODO_CHECKPOINT_IMPLEMENTATION
        })
        .unwrap();
    let requirement = &offer.authority_requirements[0];
    let grant = AuthorityGrant {
        grant_id: "grant/owner/checkpoint".into(),
        contract_id: requirement.contract_id.clone(),
        host_call_contract_id: requirement.host_call_contract_id.clone(),
        subject_kind: requirement.subject_kind.clone(),
        host_id: ad.host_id.clone(),
        boot_id: ad.boot_id.clone(),
        capability_id: offer.capability_id.clone(),
    };
    let resident = ResidentPlot::new(
        plot.expanded.source_document_id.clone(),
        plot.expanded.checked_plot_id.clone(),
    );
    let mut owner = Owner::open(host, resident, None, "Groceries").unwrap();
    owner.set_resident_plot_name(&plot).unwrap();
    owner.persist(&state_root).unwrap();
    (owner, source, plot, grant, state_root, checkpoint_root)
}

#[test]
fn todo_face_refuses_a_pre_play_contribution() {
    let (owner, _, _, _, state_root, _) = fixture();
    let initial = TodoState::new("Groceries".into()).unwrap();
    assert!(owner.project_face(Some((&initial, true))).is_err());
    std::fs::remove_dir_all(state_root).unwrap();
}

#[test]
fn first_caller_supplied_action_commits_under_retained_body_before_ack() {
    let (mut owner, source, plot, grant, state_root, checkpoint_root) = fixture();
    let body_id = owner.session.evidence().body_id.clone();
    owner.plan_checkpoint_once(&source, &plot, &grant).unwrap();
    let current = TodoState::new("Groceries".into()).unwrap();
    let committed = owner
        .execute_checkpoint_once(
            &state_root,
            &checkpoint_root,
            CheckpointIdentity {
                body: body_id.as_str().into(),
                plot: plot.expanded.checked_plot_id.as_str().into(),
                workload: "todo-list".into(),
                missing_v2: MissingV2Disposition::StartNewList,
            },
            &current,
            &TodoCommand::Add {
                text: "Buy milk".into(),
            },
            5000,
        )
        .unwrap();
    assert_eq!(committed.revision, 1);
    assert_eq!(committed.items[0].text, "Buy milk");
    assert_eq!(owner.session.evidence().body_id, body_id);
    assert!(owner.last_execution.as_ref().unwrap()["terminal_sign"]["active_play_id"].is_string());
    assert!(owner.last_execution.as_ref().unwrap()["committed_fore_sha256"].is_string());
    assert!(owner.session.realization().is_none());
    assert!(owner
        .execute_checkpoint_once(
            &state_root,
            &checkpoint_root,
            CheckpointIdentity {
                body: body_id.as_str().into(),
                plot: plot.expanded.checked_plot_id.as_str().into(),
                workload: "todo-list".into()
                missing_v2: MissingV2Disposition::StartNewList,
            },
            &committed,
            &TodoCommand::Add {
                text: "Eggs".into()
            },
            5000,
        )
        .is_err());
    std::fs::remove_dir_all(state_root).unwrap();
}

#[test]
fn foreign_checkpoint_namespace_refuses_before_play_or_publication() {
    let (mut owner, source, plot, grant, state_root, checkpoint_root) = fixture();
    owner.plan_checkpoint_once(&source, &plot, &grant).unwrap();
    let current = TodoState::new("Groceries".into()).unwrap();
    let error = owner
        .execute_checkpoint_once(
            &state_root,
            &checkpoint_root,
            CheckpointIdentity {
                body: "different-body".into(),
                plot: plot.expanded.checked_plot_id.as_str().into(),
                workload: "todo-list".into(),
                missing_v2: MissingV2Disposition::StartNewList,
            },
            &current,
            &TodoCommand::Add {
                text: "Milk".into(),
            },
            5000,
        )
        .unwrap_err();
    assert!(error.contains("namespace"));
    assert!(owner.session.realization().unwrap().play.is_none());
    assert!(std::fs::read_dir(&checkpoint_root)
        .unwrap()
        .next()
        .is_none());
    std::fs::remove_dir_all(state_root).unwrap();
}

#[test]
fn stale_current_and_foreign_grant_refuse_before_play() {
    let (mut owner, source, plot, grant, state_root, checkpoint_root) = fixture();
    let mut foreign = grant.clone();
    foreign.boot_id = BootId::from("boot/foreign");
    assert!(owner
        .plan_checkpoint_once(&source, &plot, &foreign)
        .is_err());
    assert!(owner.session.realization().is_none());
    owner.plan_checkpoint_once(&source, &plot, &grant).unwrap();
    let mut stale = TodoState::new("Groceries".into()).unwrap();
    stale.revision = 1;
    let body = owner.session.evidence().body_id.as_str().to_string();
    assert!(owner
        .execute_checkpoint_once(
            &state_root,
            &checkpoint_root,
            CheckpointIdentity {
                body,
                plot: plot.expanded.checked_plot_id.as_str().into(),
                workload: "todo-list".into(),
                missing_v2: MissingV2Disposition::StartNewList,
            },
            &stale,
            &TodoCommand::Add {
                text: "Milk".into()
            },
            5000,
        )
        .is_err());
    assert!(owner.session.realization().unwrap().play.is_none());
    assert!(std::fs::read_dir(&checkpoint_root)
        .unwrap()
        .next()
        .is_none());
    std::fs::remove_dir_all(state_root).unwrap();
}
