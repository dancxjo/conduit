//! Shared checked checkpoint Source, real selected residence and bounded Fore inputs.
#![allow(dead_code)]
use conduit_body::{Body, BodyPlan, BodyPlotPlan, ResidentPlot};
use conduit_core::*;
use conduit_std_host::body_execution::BodyForeOutputAdapter;
use conduit_std_host::todo_durable_resource::{CheckpointIdentity, MissingV2Disposition};
use conduit_std_host::{ExternalForeDelivery, ExternalForeInput};
use conduit_todo_plot::{TodoCommand, TodoState, STATE_MAX_BYTES};

pub(crate) fn planned_checkpoint() -> (conduit_std_host::StdHost, Plan, std::path::PathBuf) {
    planned_checkpoint_with_config(conduit_std_host::StdHostConfig {
        host_id: "host-a".into(),
        boot_id: "boot-a".into(),
        offer_generation: OfferGeneration(1),
    })
}

pub(crate) fn planned_checkpoint_with_config(
    config: conduit_std_host::StdHostConfig,
) -> (conduit_std_host::StdHost, Plan, std::path::PathBuf) {
    planned_checkpoint_with_version(config, [2; 32])
}

pub(crate) fn planned_checkpoint_with_version(
    config: conduit_std_host::StdHostConfig,
    version: [u8; 32],
) -> (conduit_std_host::StdHost, Plan, std::path::PathBuf) {
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
        version: ResourceVersionIdentity::from_digest(version),
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
    let host =
        conduit_std_host::StdHost::new_for_todo_checkpoint_once(config, &root, content).unwrap();
    let plan = checkpoint_plan(&host);
    (host, plan, root)
}

pub(crate) fn checkpoint_plan(host: &conduit_std_host::StdHost) -> Plan {
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
        &parse_syntax_document(include_str!(
            "../../../../plots/todo/checkpoint-once.conduit"
        )),
        &startup,
    )
    .unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "todo/checkpoint-once", &profile).unwrap();
    let expanded = &authored.expanded;
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
    conduit_planner::plan_expanded_authoring_with_activations(
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
    .unwrap()
}

#[derive(Default)]
pub(crate) struct CapturedFore(pub(crate) Vec<ExternalForeDelivery>);
impl BodyForeOutputAdapter for CapturedFore {
    fn deliver(&mut self, output: &ExternalForeDelivery) -> Result<(), String> {
        self.0.push(output.clone());
        Ok(())
    }
}

pub(crate) fn body_plan(plan: Plan) -> (conduit_body::Wake, BodyPlan) {
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

pub(crate) fn inputs() -> Vec<ExternalForeInput> {
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
pub(crate) fn checkpoint_identity() -> CheckpointIdentity {
    CheckpointIdentity {
        body: "body-1".into(),
        plot: "checked-plot-1".into(),
        workload: "revision-1".into(),
        missing_v2: MissingV2Disposition::StartNewList,
    }
}
