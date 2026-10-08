use conduit_body::{Body, BodyPlan, BodyPlotPlan, ResidentPlot};
use conduit_core::*;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    CanonicalBackCatalog, KindSignature, ProfileCatalog, StartupCatalog,
};
use conduit_std_host::body_execution::{BodyForeOutputAdapter, BodyRunRequest};
use conduit_std_host::todo_durable_resource::CheckpointIdentity;
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
    let (source, entry, host_id, boot_id) = match mode {
        Mode::Write => (
            include_str!("../../../plots/todo/checkpoint-once.conduit"),
            "todo/checkpoint-once",
            "host-a",
            "boot-a",
        ),
        Mode::Read => (
            include_str!("../../../plots/todo/checkpoint-restore.conduit"),
            "todo/checkpoint-restore",
            "host-b",
            "boot-b",
        ),
    };
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, entry, &profile).unwrap();
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
        grant_id: format!("grant/{host_id}/{boot_id}/checkpoint").into(),
        contract_id: requirement.contract_id.clone(),
        host_call_contract_id: requirement.host_call_contract_id.clone(),
        subject_kind: requirement.subject_kind.clone(),
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        capability_id: offer.capability_id.clone(),
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
    .unwrap();
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
    (host, plan)
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
            &inputs,
            &mut fore,
            root,
            identity(),
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
    restore_with_identity(root, version, identity())
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
            root,
            checkpoint,
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
