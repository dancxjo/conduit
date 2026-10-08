use conduit_body::{Body, BodyPlan, BodyPlotPlan, ResidentPlot};
use conduit_core::*;
use conduit_std_host::body_execution::{
    BodyForeExchange, BodyForeOutputAdapter, BodyRunRequest, TodoCheckpointSelection,
};
use conduit_std_host::todo_durable_resource::{
    CheckpointIdentity, MissingV2Disposition, SelectedTodoResidence, READ_OPERATION,
};
use conduit_std_host::{ExternalForeDelivery, ExternalForeInput, RunControl, ThreadTimer};
use conduit_todo_plot::{TodoCommand, TodoState, STATE_MAX_BYTES};

fn planned_checkpoint() -> (conduit_std_host::StdHost, Plan, std::path::PathBuf) {
    use conduit_plot::{
        check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
        KindSignature, ProfileCatalog, StartupCatalog,
    };
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    for kind in [
        conduit_todo_plot::todo_combine_kind(),
        conduit_todo_plot::todo_checkpoint_kind(),
    ] {
        startup
            .insert(KindSignature {
                kind: kind.kind_id.as_str().to_string(),
                startup_parameters: Vec::new(),
            })
            .unwrap();
        profile.insert_kind(kind).unwrap();
    }
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("../../../plots/todo/checkpoint-once.conduit")),
        &startup,
    )
    .unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "todo/checkpoint-once", &profile).unwrap();
    let expanded = &authored.expanded;
    let root = std::env::temp_dir().join(format!(
        "todo-checkpoint-plan-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let content = ResourceContentRequirement {
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
    };
    let host = conduit_std_host::StdHost::new_for_todo_checkpoint_once(
        conduit_std_host::StdHostConfig {
            host_id: "host-a".into(),
            boot_id: "boot-a".into(),
            offer_generation: OfferGeneration(1),
        },
        &root,
        content,
    )
    .unwrap();
    let advertisement = host.advertisement().clone();
    let checkpoint_offer = advertisement
        .capabilities
        .iter()
        .find(|offer| {
            offer.implementation.implementation_id.as_str()
                == conduit_std_offers::TODO_CHECKPOINT_IMPLEMENTATION
        })
        .unwrap();
    let requirement = &checkpoint_offer.authority_requirements[0];
    let grant = AuthorityGrant {
        grant_id: "grant/host-a/boot-a/checkpoint".into(),
        contract_id: requirement.contract_id.clone(),
        host_call_contract_id: requirement.host_call_contract_id.clone(),
        subject_kind: requirement.subject_kind.clone(),
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        capability_id: checkpoint_offer.capability_id.clone(),
    };
    let hosts = [advertisement];
    let placements = conduit_planner::default_expanded_placements(expanded, &hosts).unwrap();
    let connection_bases = std::collections::BTreeMap::new();
    let line_candidates = std::collections::BTreeMap::new();
    let boundary_limits = std::collections::BTreeMap::from([
        (
            conduit_planner::ForeBoundaryKey {
                direction: PortDirection::Input,
                front_port_id: port_id("current"),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: STATE_MAX_BYTES as u32,
            },
        ),
        (
            conduit_planner::ForeBoundaryKey {
                direction: PortDirection::Input,
                front_port_id: port_id("command"),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: conduit_todo_plot::COMMAND_MAX_BYTES as u32,
            },
        ),
        (
            conduit_planner::ForeBoundaryKey {
                direction: PortDirection::Output,
                front_port_id: port_id("committed"),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: STATE_MAX_BYTES as u32,
            },
        ),
    ]);
    let plan = conduit_planner::plan_expanded_authoring_with_activations(
        &checked,
        &authored,
        &profile,
        &conduit_plot::CanonicalBackCatalog::new(),
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
        &boundary_limits,
    )
    .unwrap();
    (host, plan, root)
}

#[test]
fn authored_plan_selects_exact_installed_backs() {
    let (_host, plan, root) = planned_checkpoint();
    let placements = &plan.fragments[0].placements;
    assert_eq!(placements.len(), 2);
    let combine = placements
        .iter()
        .find(|p| p.kind_id == conduit_todo_plot::todo_combine_kind().kind_id)
        .unwrap();
    assert_eq!(combine.capability_id.as_str(), "std-todo-combine-v1");
    assert_eq!(
        combine.execution_profile_id.as_str(),
        conduit_std_offers::TODO_COMBINE_PROFILE
    );
    let checkpoint = placements
        .iter()
        .find(|p| p.kind_id == conduit_todo_plot::todo_checkpoint_kind().kind_id)
        .unwrap();
    assert_eq!(
        checkpoint.implementation_id.as_str(),
        conduit_std_offers::TODO_CHECKPOINT_IMPLEMENTATION
    );
    assert_eq!(checkpoint.resources.len(), 1);
    assert_eq!(checkpoint.authority.len(), 1);
    std::fs::remove_dir_all(root).unwrap();
}

#[derive(Default)]
struct CapturedFore(Vec<ExternalForeDelivery>);
impl BodyForeOutputAdapter for CapturedFore {
    fn deliver(&mut self, output: &ExternalForeDelivery) -> Result<(), String> {
        self.0.push(output.clone());
        Ok(())
    }
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

fn inputs() -> Vec<ExternalForeInput> {
    vec![
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
    ]
}
fn checkpoint_identity() -> CheckpointIdentity {
    CheckpointIdentity {
        body: "body-1".into(),
        plot: "checked-plot-1".into(),
        workload: "revision-1".into(),
        missing_v2: MissingV2Disposition::StartNewList,
    }
}

#[test]
fn public_body_play_publishes_before_committed_fore_delivery() {
    std::thread::Builder::new()
        .name("todo-checkpoint-body-proof".into())
        .stack_size(4 * 1024 * 1024)
        .spawn(public_body_play_proof)
        .unwrap()
        .join()
        .unwrap();
}

fn public_body_play_proof() {
    let (mut host, plan, root) = planned_checkpoint();
    let write = plan.fragments[0]
        .placements
        .iter()
        .find(|p| p.kind_id == conduit_todo_plot::todo_checkpoint_kind().kind_id)
        .unwrap()
        .clone();
    let (wake, body_plan) = body_plan(plan);
    let control = RunControl::default();
    let mut fore = CapturedFore::default();
    let mut output = Vec::new();
    let mut timer = ThreadTimer;
    let mut started = false;
    let report = host
        .run_body_plan_with_todo_checkpoint_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &body_plan,
                control: &control,
                keyboard: None,
            },
            BodyForeExchange {
                inputs: &inputs(),
                output: &mut fore,
            },
            TodoCheckpointSelection {
                root: &root,
                identity: checkpoint_identity(),
            },
            &mut output,
            &mut timer,
            |_, _| {
                started = true;
                Ok(())
            },
        )
        .unwrap();
    assert!(started);
    assert_eq!(report.terminal, TerminalDisposition::Completed);
    assert!(report.failure.is_none(), "{:?}", report.failure);
    assert_eq!(fore.0.len(), 1);
    let mut read = write;
    read.host_id = "host-b".into();
    read.boot_id = "boot-b".into();
    read.host_calls[0].contract_id = READ_OPERATION.into();
    read.host_calls[0].maximum_input_bytes = 0;
    read.host_calls[0].maximum_output_bytes = STATE_MAX_BYTES as u32;
    read.resources[0].content.as_mut().unwrap().contract.access = ResourceAccessMode::ReadPublished;
    read.resources[0]
        .content
        .as_mut()
        .unwrap()
        .contract
        .publication_slots = 0;
    read.resources[0].content.as_mut().unwrap().owner_host = read.host_id.clone();
    read.resources[0].content.as_mut().unwrap().owner_boot = read.boot_id.clone();
    read.authority[0].host_call_contract_id = READ_OPERATION.into();
    read.authority[0].host_id = read.host_id.clone();
    read.authority[0].boot_id = read.boot_id.clone();
    read.authority[0].grant_id = "grant/host-b/read".into();
    let reader = SelectedTodoResidence::prepare(&root, &read, checkpoint_identity()).unwrap();
    let recovered = reader.recover(&read.authority[0]).unwrap();
    assert_eq!(TodoState::decode_info(&fore.0[0].bytes).unwrap(), recovered);
    assert_eq!(recovered.items[0].text, "Buy milk");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn refused_play_start_leaves_residence_and_fore_untouched() {
    std::thread::Builder::new()
        .name("todo-checkpoint-start-refusal".into())
        .stack_size(4 * 1024 * 1024)
        .spawn(refused_start_proof)
        .unwrap()
        .join()
        .unwrap();
}

fn refused_start_proof() {
    let (mut host, plan, root) = planned_checkpoint();
    let (wake, body_plan) = body_plan(plan);
    let control = RunControl::default();
    let mut fore = CapturedFore::default();
    let result = host.run_body_plan_with_todo_checkpoint_to_with_start(
        BodyRunRequest {
            wake: &wake,
            plan: &body_plan,
            control: &control,
            keyboard: None,
        },
        BodyForeExchange {
            inputs: &inputs(),
            output: &mut fore,
        },
        TodoCheckpointSelection {
            root: &root,
            identity: checkpoint_identity(),
        },
        &mut Vec::new(),
        &mut ThreadTimer,
        |_, _| Err("start receipt refused".into()),
    );
    assert!(result.is_err());
    assert!(fore.0.is_empty());
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn rebound_residence_is_refused_before_play() {
    let (mut host, plan, root) = planned_checkpoint();
    let displaced = root.with_extension("displaced");
    std::fs::rename(&root, &displaced).unwrap();
    std::fs::create_dir(&root).unwrap();
    let (wake, body_plan) = body_plan(plan);
    let control = RunControl::default();
    let mut fore = CapturedFore::default();
    let result = host.run_body_plan_with_todo_checkpoint_to(
        BodyRunRequest {
            wake: &wake,
            plan: &body_plan,
            control: &control,
            keyboard: None,
        },
        BodyForeExchange {
            inputs: &inputs(),
            output: &mut fore,
        },
        TodoCheckpointSelection {
            root: &root,
            identity: checkpoint_identity(),
        },
        &mut Vec::new(),
        &mut ThreadTimer,
    );
    assert!(result.unwrap_err().contains("rebound"));
    assert!(fore.0.is_empty());
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(displaced).unwrap();
}
