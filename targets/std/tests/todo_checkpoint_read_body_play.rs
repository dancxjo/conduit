use conduit_body::{Body, BodyPlan, BodyPlotPlan, ResidentPlot};
use conduit_core::*;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    CanonicalBackCatalog, KindSignature, ProfileCatalog, StartupCatalog,
};
use conduit_std_host::body_execution::{
    BodyForeExchange, BodyForeOutputAdapter, BodyRunRequest, TodoCheckpointSelection,
};
use conduit_std_host::todo_durable_resource::{CheckpointIdentity, MissingV2Disposition};
use conduit_std_host::todo_durable_resource::{Refusal, SelectedTodoResidence};
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, RunControl, StdHost, StdHostConfig, ThreadTimer,
};
use conduit_todo_plot::{TodoCommand, TodoState, COMMAND_MAX_BYTES, STATE_MAX_BYTES};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy)]
enum Mode {
    Write,
    Read,
}
fn root() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "todo-read-body-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&path).unwrap();
    path
}
fn identity() -> CheckpointIdentity {
    CheckpointIdentity {
        body: "body-1".into(),
        plot: "checked-todo-1".into(),
        workload: "todo-list-1".into(),
        missing_v2: MissingV2Disposition::StartNewList,
    }
}
fn read_identity() -> CheckpointIdentity {
    CheckpointIdentity {
        plot: "checked-todo-restore-2".into(),
        missing_v2: MissingV2Disposition::InspectLegacyWritePlot("checked-todo-1".into()),
        ..identity()
    }
}
fn content(mode: Mode, version: u8) -> ResourceContentRequirement {
    ResourceContentRequirement {
        identity: ResourceSemanticIdentity::from_digest([1; 32]),
        version: ResourceVersionIdentity::from_digest([version; 32]),
        content_profile: kind_id("conduit.todo/checkpoint-envelope@1"),
        maximum_bytes: conduit_std_offers::TODO_CHECKPOINT_MAX_BYTES,
        maximum_items: 1,
        retention: ResourceRetention::ExternalDurable,
        sharing: ResourceSharing::SingleWriterPublished,
        access: match mode {
            Mode::Write => ResourceAccessMode::WriteCandidatePublish,
            Mode::Read => ResourceAccessMode::ReadPublished,
        },
        generation_slots: 1,
        reader_leases: 1,
        publication_slots: match mode {
            Mode::Write => 1,
            Mode::Read => 0,
        },
        sensitive: false,
    }
}
fn planned(mode: Mode, root: &Path, version: u8) -> (StdHost, Plan) {
    let (host_id, boot_id) = match mode {
        Mode::Write => ("host-a", "boot-a"),
        Mode::Read => ("host-b", "boot-b"),
    };
    let config = StdHostConfig {
        host_id: host_id.into(),
        boot_id: boot_id.into(),
        offer_generation: OfferGeneration(1),
    };
    let host = match mode {
        Mode::Write => StdHost::new_for_todo_checkpoint_once(config, root, content(mode, version)),
        Mode::Read => StdHost::new_for_todo_checkpoint_read(config, root, content(mode, version)),
    }
    .unwrap();
    planned_on_host(mode, host, version)
}
fn planned_on_host(mode: Mode, host: StdHost, version: u8) -> (StdHost, Plan) {
    let plan = plan_on_host(mode, &host, version, None).unwrap();
    (host, plan)
}
fn plan_on_host(
    mode: Mode,
    host: &StdHost,
    version: u8,
    grant_capability: Option<CapabilityId>,
) -> Result<Plan, String> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    for kind in [
        conduit_todo_plot::todo_combine_kind(),
        conduit_todo_plot::todo_checkpoint_kind(),
        conduit_todo_plot::todo_checkpoint_read_kind(),
    ] {
        startup
            .insert(KindSignature {
                kind: kind.kind_id.as_str().to_string(),
                startup_parameters: Vec::new(),
            })
            .unwrap();
        profile.insert_kind(kind).unwrap();
    }
    let (source, entry) = match mode {
        Mode::Write => (
            include_str!("../../../plots/todo/checkpoint-once.conduit"),
            "todo/checkpoint-once",
        ),
        Mode::Read => (
            include_str!("../../../plots/todo/checkpoint-restore.conduit"),
            "todo/checkpoint-restore",
        ),
    };
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, entry, &profile).unwrap();
    let advertisement = host.advertisement().clone();
    let implementation = match mode {
        Mode::Write => conduit_std_offers::TODO_CHECKPOINT_IMPLEMENTATION,
        Mode::Read => conduit_std_offers::TODO_CHECKPOINT_READ_IMPLEMENTATION,
    };
    let offer = advertisement
        .capabilities
        .iter()
        .find(|offer| offer.implementation.implementation_id.as_str() == implementation)
        .unwrap();
    let requirement = &offer.authority_requirements[0];
    let grant = AuthorityGrant {
        grant_id: format!(
            "grant/{}/{}/checkpoint",
            advertisement.host_id.as_str(),
            advertisement.boot_id.as_str()
        )
        .into(),
        contract_id: requirement.contract_id.clone(),
        host_call_contract_id: requirement.host_call_contract_id.clone(),
        subject_kind: requirement.subject_kind.clone(),
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        capability_id: grant_capability.unwrap_or_else(|| offer.capability_id.clone()),
    };
    let hosts = [advertisement];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let mut limits = BTreeMap::new();
    let mut boundary = |direction, name, bytes| {
        limits.insert(
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: port_id(name),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: bytes,
            },
        );
    };
    match mode {
        Mode::Write => {
            boundary(PortDirection::Input, "current", STATE_MAX_BYTES as u32);
            boundary(PortDirection::Input, "command", COMMAND_MAX_BYTES as u32);
            boundary(PortDirection::Output, "committed", STATE_MAX_BYTES as u32);
        }
        Mode::Read => boundary(PortDirection::Output, "restored", STATE_MAX_BYTES as u32),
    }
    let connection_bases = BTreeMap::new();
    let line_candidates = BTreeMap::new();
    let plan = conduit_planner::plan_expanded_authoring_with_activations(
        &checked,
        &authored,
        &profile,
        &CanonicalBackCatalog::new(),
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &connection_bases,
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: STATE_MAX_BYTES as u32,
            authority_grants: &[grant],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &limits,
    )
    .map_err(|error| format!("{error:?}"))?;
    if let Mode::Read = mode {
        let [placement] = plan.fragments[0].placements.as_slice() else {
            panic!("restore must select exactly one read Gear");
        };
        assert_eq!(
            placement.implementation_id.as_str(),
            conduit_std_offers::TODO_CHECKPOINT_READ_IMPLEMENTATION
        );
        assert_eq!(placement.host_calls.len(), 1);
        assert_eq!(placement.authority.len(), 1);
        assert_eq!(placement.resources.len(), 1);
        let selected = &placement.resources[0].content.as_ref().unwrap().contract;
        assert_eq!(selected.access, ResourceAccessMode::ReadPublished);
        assert_eq!(
            selected.version,
            ResourceVersionIdentity::from_digest([version; 32])
        );
    }
    Ok(plan)
}
fn body_plan(plan: Plan) -> (conduit_body::Wake, BodyPlan) {
    let resident = ResidentPlot::new(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
    );
    let body = Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        SignId::from("sign/born"),
    )
    .unwrap();
    let wake = body.wake(1, SignId::from("sign/wake")).unwrap().1;
    let body_plan = BodyPlan::seal(
        &wake,
        vec![BodyPlotPlan {
            plot: resident,
            plan,
        }],
    )
    .unwrap();
    (wake, body_plan)
}
#[derive(Default)]
struct CapturedFore(Vec<ExternalForeDelivery>);
impl BodyForeOutputAdapter for CapturedFore {
    fn deliver(&mut self, output: &ExternalForeDelivery) -> Result<(), String> {
        self.0.push(output.clone());
        Ok(())
    }
}
fn seed(root: &Path) {
    let (mut host, plan) = planned(Mode::Write, root, 2);
    let (wake, body_plan) = body_plan(plan);
    let control = RunControl::default();
    let inputs = [
        ExternalForeInput {
            front_port_id: port_id("current"),
            track: ConnectionTrack::Payload,
            bytes: TodoState::new("Groceries".into())
                .unwrap()
                .encode_info()
                .unwrap(),
        },
        ExternalForeInput {
            front_port_id: port_id("command"),
            track: ConnectionTrack::Payload,
            bytes: TodoCommand::Add {
                text: "Buy milk".into(),
            }
            .encode_info()
            .unwrap(),
        },
    ];
    let mut fore = CapturedFore::default();
    let report = host
        .run_body_plan_with_todo_checkpoint_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &body_plan,
                control: &control,
                keyboard: None,
            },
            BodyForeExchange {
                inputs: &inputs,
                output: &mut fore,
            },
            TodoCheckpointSelection {
                root,
                identity: identity(),
            },
            &mut Vec::new(),
            &mut ThreadTimer,
            |_, _| Ok(()),
        )
        .unwrap();
    assert_eq!(report.terminal, TerminalDisposition::Completed);
    assert_eq!(fore.0.len(), 1);
}
fn restore(
    root: &Path,
    version: u8,
) -> (
    conduit_std_host::body_execution::BodyRunReport,
    CapturedFore,
) {
    restore_with_identity(root, version, read_identity())
}
fn restore_with_identity(
    root: &Path,
    version: u8,
    checkpoint: CheckpointIdentity,
) -> (
    conduit_std_host::body_execution::BodyRunReport,
    CapturedFore,
) {
    let (mut host, plan) = planned(Mode::Read, root, version);
    let (wake, body_plan) = body_plan(plan);
    let control = RunControl::default();
    let mut fore = CapturedFore::default();
    let report = host
        .run_body_plan_with_todo_checkpoint_read_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &body_plan,
                control: &control,
                keyboard: None,
            },
            &mut fore,
            TodoCheckpointSelection {
                root,
                identity: checkpoint,
            },
            &mut Vec::new(),
            &mut ThreadTimer,
            |_, _| Ok(()),
        )
        .unwrap();
    (report, fore)
}
fn on_body_stack(test: fn()) {
    std::thread::Builder::new()
        .name("todo-read-body-proof".into())
        .stack_size(4 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}
#[test]
fn idle_offer_transition_preserves_host_play_sequence_and_refuses_stale_plans() {
    on_body_stack(|| {
        let root = root();
        let (mut host, first_plan) = planned(Mode::Write, &root, 2);
        let before_refusal = host.advertisement().clone();
        let mut other_identity = content(Mode::Read, 2);
        other_identity.identity = ResourceSemanticIdentity::from_digest([9; 32]);
        assert!(host
            .transition_todo_checkpoint_offer(&root, other_identity)
            .is_err());
        assert_eq!(host.advertisement(), &before_refusal);
        let old_write_capability = first_plan.fragments[0]
            .placements
            .iter()
            .find(|placement| {
                placement.implementation_id.as_str()
                    == conduit_std_offers::TODO_CHECKPOINT_IMPLEMENTATION
            })
            .unwrap()
            .capability_id
            .clone();
        let first_state = TodoState::new("Groceries".into())
            .unwrap()
            .encode_info()
            .unwrap();
        let write = |host: &mut StdHost, plan: Plan, state: Vec<u8>, text: &str| {
            let (wake, body_plan) = body_plan(plan);
            let control = RunControl::default();
            let inputs = [
                ExternalForeInput {
                    front_port_id: port_id("current"),
                    track: ConnectionTrack::Payload,
                    bytes: state,
                },
                ExternalForeInput {
                    front_port_id: port_id("command"),
                    track: ConnectionTrack::Payload,
                    bytes: TodoCommand::Add { text: text.into() }
                        .encode_info()
                        .unwrap(),
                },
            ];
            let mut fore = CapturedFore::default();
            let report = host
                .run_body_plan_with_todo_checkpoint_to_with_start(
                    BodyRunRequest {
                        wake: &wake,
                        plan: &body_plan,
                        control: &control,
                        keyboard: None,
                    },
                    BodyForeExchange {
                        inputs: &inputs,
                        output: &mut fore,
                    },
                    TodoCheckpointSelection {
                        root: &root,
                        identity: CheckpointIdentity {
                            plot: format!("checked-todo-write/{text}"),
                            ..identity()
                        },
                    },
                    &mut Vec::new(),
                    &mut ThreadTimer,
                    |_, _| Ok(()),
                )
                .unwrap();
            assert_eq!(report.terminal, TerminalDisposition::Completed);
            assert_eq!(fore.0.len(), 1);
            (report, fore.0.remove(0).bytes)
        };
        let stale_write = first_plan.clone();
        let (first, committed) = write(&mut host, first_plan, first_state, "Buy milk");
        let old_generation = host.advertisement().offer_generation;
        host.transition_todo_checkpoint_offer(&root, content(Mode::Read, 2))
            .unwrap();
        assert!(host.advertisement().offer_generation > old_generation);
        let (wake, stale_body) = body_plan(stale_write);
        let mut fore = CapturedFore::default();
        let stale_inputs = [
            ExternalForeInput {
                front_port_id: port_id("current"),
                track: ConnectionTrack::Payload,
                bytes: committed.clone(),
            },
            ExternalForeInput {
                front_port_id: port_id("command"),
                track: ConnectionTrack::Payload,
                bytes: TodoCommand::Add {
                    text: "stale".into(),
                }
                .encode_info()
                .unwrap(),
            },
        ];
        assert!(host
            .run_body_plan_with_todo_checkpoint_to_with_start(
                BodyRunRequest {
                    wake: &wake,
                    plan: &stale_body,
                    control: &RunControl::default(),
                    keyboard: None
                },
                BodyForeExchange {
                    inputs: &stale_inputs,
                    output: &mut fore,
                },
                TodoCheckpointSelection {
                    root: &root,
                    identity: identity(),
                },
                &mut Vec::new(),
                &mut ThreadTimer,
                |_, _| Ok(()),
            )
            .is_err());
        assert!(fore.0.is_empty());
        let (mut host, read_plan) = planned_on_host(Mode::Read, host, 2);
        let stale_read = read_plan.clone();
        let (wake, read_body_plan) = body_plan(read_plan);
        let mut restored = CapturedFore::default();
        let read = host
            .run_body_plan_with_todo_checkpoint_read_to_with_start(
                BodyRunRequest {
                    wake: &wake,
                    plan: &read_body_plan,
                    control: &RunControl::default(),
                    keyboard: None,
                },
                &mut restored,
                TodoCheckpointSelection {
                    root: &root,
                    identity: identity(),
                },
                &mut Vec::new(),
                &mut ThreadTimer,
                |_, _| Ok(()),
            )
            .unwrap();
        assert_eq!(read.terminal, TerminalDisposition::Completed);
        assert_eq!(restored.0.len(), 1);
        assert_eq!(restored.0[0].bytes, committed);
        host.transition_todo_checkpoint_offer(&root, content(Mode::Write, 3))
            .unwrap();
        assert!(plan_on_host(Mode::Write, &host, 3, Some(old_write_capability)).is_err());
        let (wake, stale_body) = body_plan(stale_read);
        let mut fore = CapturedFore::default();
        assert!(host
            .run_body_plan_with_todo_checkpoint_read_to_with_start(
                BodyRunRequest {
                    wake: &wake,
                    plan: &stale_body,
                    control: &RunControl::default(),
                    keyboard: None
                },
                &mut fore,
                TodoCheckpointSelection {
                    root: &root,
                    identity: identity(),
                },
                &mut Vec::new(),
                &mut ThreadTimer,
                |_, _| Ok(()),
            )
            .is_err());
        assert!(fore.0.is_empty());
        let (mut host, second_plan) = planned_on_host(Mode::Write, host, 3);
        let (second, _) = write(
            &mut host,
            second_plan,
            restored.0.remove(0).bytes,
            "Buy bread",
        );
        assert_ne!(first.play.active_play_id, read.play.active_play_id);
        assert_ne!(first.play.active_play_id, second.play.active_play_id);
        assert_ne!(read.play.active_play_id, second.play.active_play_id);
        assert_ne!(first.terminal_sign.sign_id, second.terminal_sign.sign_id);
        std::fs::remove_dir_all(root).unwrap();
    });
}
#[test]
fn second_host_restores_exact_published_state_through_fore() {
    on_body_stack(|| {
        let root = root();
        seed(&root);
        let (report, fore) = restore(&root, 2);
        assert_eq!(report.terminal, TerminalDisposition::Completed);
        assert!(report.failure.is_none());
        assert_eq!(fore.0.len(), 1);
        let restored = TodoState::decode_info(&fore.0[0].bytes).unwrap();
        assert_eq!(restored.items[0].text, "Buy milk");
        std::fs::remove_dir_all(root).unwrap();
    });
}
#[test]
fn read_body_play_transfers_large_admitted_state_and_refuses_oversize_checkpoint() {
    on_body_stack(|| {
        for (title, count, text) in [
            ("Groceries".to_owned(), 4, "Long-list item".to_owned()),
            ("L".repeat(64), 20, "I".repeat(72)),
        ] {
            let root = root();
            let (_, plan) = planned(Mode::Write, &root, 2);
            let placement = plan.fragments[0]
                .placements
                .iter()
                .find(|placement| {
                    placement.implementation_id.as_str()
                        == conduit_std_offers::TODO_CHECKPOINT_IMPLEMENTATION
                })
                .unwrap();
            let residence = SelectedTodoResidence::prepare(&root, placement, identity()).unwrap();
            let mut state = TodoState::new(title).unwrap();
            for _ in 0..count {
                state = state
                    .apply(&TodoCommand::Add { text: text.clone() })
                    .unwrap();
                residence.commit(&placement.authority[0], &state).unwrap();
            }
            let bytes = state.encode_info().unwrap();
            assert!(bytes.len() > 100);
            if count == 20 {
                assert_eq!(bytes.len(), STATE_MAX_BYTES);
            }
            let (report, fore) = restore(&root, 2);
            assert_eq!(report.terminal, TerminalDisposition::Completed);
            assert_eq!(fore.0.len(), 1);
            assert_eq!(fore.0[0].bytes, bytes);

            if count == 20 {
                let candidate = std::fs::read_dir(&root)
                    .unwrap()
                    .map(|entry| entry.unwrap().path())
                    .find(|path| path.extension().is_some_and(|ext| ext == "checkpoint"))
                    .unwrap();
                let mut oversized = std::fs::read(&candidate).unwrap();
                oversized.push(0);
                std::fs::write(candidate, oversized).unwrap();
                let (refused, fore) = restore(&root, 2);
                assert_ne!(refused.terminal, TerminalDisposition::Completed);
                assert!(fore.0.is_empty());
            }
            std::fs::remove_dir_all(root).unwrap();
        }
    });
}
#[test]
fn old_plot_keyed_checkpoint_requires_explicit_migration() {
    on_body_stack(|| {
        let root = root();
        let (host, plan) = planned(Mode::Read, &root, 2);
        let placement = &plan.fragments[0].placements[0];
        let selected = placement.resources[0].content.as_ref().unwrap();
        let checkpoint = read_identity();
        let old_write_plot = "checked-todo-1".to_string();
        let mut key = Vec::new();
        key.extend_from_slice(&selected.contract.identity.digest());
        for part in [&checkpoint.body, &old_write_plot, &checkpoint.workload] {
            key.extend_from_slice(&(part.len() as u16).to_le_bytes());
            key.extend_from_slice(part.as_bytes());
        }
        let legacy = semantic_digest("conduit.todo/checkpoint-namespace@1", &key);
        let namespace = legacy
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        std::fs::write(root.join(format!("{namespace}.current")), [0u8; 36]).unwrap();
        let residence = SelectedTodoResidence::prepare(&root, placement, checkpoint).unwrap();
        assert_eq!(
            residence.recover(&placement.authority[0]),
            Err(Refusal::MigrationRequired)
        );
        drop(host);
        std::fs::remove_dir_all(root).unwrap();
    });
}
#[test]
fn fresh_v2_publication_requires_explicit_new_list_choice() {
    on_body_stack(|| {
        let root = root();
        let (_, plan) = planned(Mode::Write, &root, 2);
        let placement = plan.fragments[0]
            .placements
            .iter()
            .find(|placement| {
                placement.implementation_id.as_str()
                    == conduit_std_offers::TODO_CHECKPOINT_IMPLEMENTATION
            })
            .unwrap();
        let checkpoint = CheckpointIdentity {
            missing_v2: MissingV2Disposition::Refuse,
            ..identity()
        };
        let residence = SelectedTodoResidence::prepare(&root, placement, checkpoint).unwrap();
        let first = TodoState::new("Groceries".into())
            .unwrap()
            .apply(&TodoCommand::Add {
                text: "Buy milk".into(),
            })
            .unwrap();
        assert_eq!(
            residence.commit(&placement.authority[0], &first),
            Err(Refusal::Missing)
        );
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        std::fs::remove_dir_all(root).unwrap();
    });
}
#[test]
fn selected_resource_body_and_list_mismatch_refuse_before_state_output() {
    on_body_stack(|| {
        let root = root();
        seed(&root);
        let (_, plan) = planned(Mode::Read, &root, 2);
        let placement = &plan.fragments[0].placements[0];
        let mut wrong_body = read_identity();
        wrong_body.body = "other-body".into();
        let residence = SelectedTodoResidence::prepare(&root, placement, wrong_body).unwrap();
        assert_eq!(
            residence.recover(&placement.authority[0]),
            Err(Refusal::Missing)
        );
        let mut wrong_key = read_identity();
        wrong_key.workload = "other-list".into();
        let residence = SelectedTodoResidence::prepare(&root, placement, wrong_key).unwrap();
        assert_eq!(
            residence.recover(&placement.authority[0]),
            Err(Refusal::Missing)
        );
        let wrong_content = ResourceContentRequirement {
            identity: ResourceSemanticIdentity::from_digest([9; 32]),
            ..content(Mode::Read, 2)
        };
        let host = StdHost::new_for_todo_checkpoint_read(
            StdHostConfig {
                host_id: "host-b".into(),
                boot_id: "boot-b".into(),
                offer_generation: OfferGeneration(1),
            },
            &root,
            wrong_content,
        )
        .unwrap();
        let plan = plan_on_host(Mode::Read, &host, 2, None).unwrap();
        let placement = &plan.fragments[0].placements[0];
        let residence = SelectedTodoResidence::prepare(&root, placement, read_identity()).unwrap();
        assert_eq!(
            residence.recover(&placement.authority[0]),
            Err(Refusal::Missing)
        );
        std::fs::remove_dir_all(root).unwrap();
    });
}
#[test]
fn missing_stale_and_corrupt_versions_never_emit_state() {
    on_body_stack(|| {
        let root = root();
        let (missing, fore) = restore(&root, 2);
        assert_ne!(missing.terminal, TerminalDisposition::Completed);
        assert!(fore.0.is_empty());
        seed(&root);
        let (stale, fore) = restore(&root, 3);
        assert_ne!(stale.terminal, TerminalDisposition::Completed);
        assert!(fore.0.is_empty());
        let mut other_namespace = identity();
        other_namespace.workload = "other-todo-list".into();
        let (mismatch, fore) = restore_with_identity(&root, 2, other_namespace);
        assert_ne!(mismatch.terminal, TerminalDisposition::Completed);
        assert!(fore.0.is_empty());
        let mut other_body = read_identity();
        other_body.body = "body-2".into();
        let (mismatch, fore) = restore_with_identity(&root, 2, other_body);
        assert_ne!(mismatch.terminal, TerminalDisposition::Completed);
        assert!(fore.0.is_empty());
        let candidate = std::fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| path.extension().is_some_and(|ext| ext == "checkpoint"))
            .unwrap();
        std::fs::write(candidate, b"corrupt").unwrap();
        let (corrupt, fore) = restore(&root, 2);
        assert_ne!(corrupt.terminal, TerminalDisposition::Completed);
        assert!(fore.0.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    });
}
