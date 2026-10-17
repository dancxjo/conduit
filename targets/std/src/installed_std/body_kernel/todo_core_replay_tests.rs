//! Replay the unchanged authored Todo through fresh installed Hosts and Bodies.
use super::*;
use crate::{body_execution::BodyRunRequest, StdHost, StdHostConfig};
use conduit_body::{Body, BodyPlan, ResidentPlot};
use conduit_core::{port_id, BootId, ConnectionTrack, HostId, OfferGeneration, SignId};
use conduit_todo_plot::{TodoCommand, TodoItem, TodoState};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::time::Duration;

const SOURCE: &str = include_str!("../../../../../plots/todo/live.conduit");

struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {}
}

#[derive(Default)]
struct States(Vec<Vec<u8>>);
impl BodyForeOutputAdapter for States {
    fn deliver(&mut self, output: &ExternalForeDelivery) -> Result<(), String> {
        assert_eq!(output.front_port_id, port_id("states"));
        if output.track == ConnectionTrack::Payload {
            self.0.push(output.bytes.clone());
        }
        Ok(())
    }
}

struct Replay {
    states: Vec<Vec<u8>>,
    terminal: TerminalDisposition,
    receipt: Value,
}

fn hash(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn commands() -> Vec<TodoCommand> {
    vec![
        TodoCommand::Add {
            text: "Milk".into(),
        },
        TodoCommand::Add {
            text: "Eggs".into(),
        },
        TodoCommand::Add {
            text: "Bread".into(),
        },
        TodoCommand::SetComplete {
            id: "task-1".into(),
            complete: true,
        },
        TodoCommand::SetComplete {
            id: "task-1".into(),
            complete: true,
        },
        TodoCommand::SetComplete {
            id: "task-1".into(),
            complete: false,
        },
        TodoCommand::Remove {
            id: "task-2".into(),
        },
    ]
}

fn expected(revision: u32, next_id: u32, items: &[(&str, &str, bool)]) -> TodoState {
    TodoState {
        title: "Groceries".into(),
        revision,
        next_id,
        items: items
            .iter()
            .map(|&(id, text, complete)| TodoItem {
                id: id.into(),
                text: text.into(),
                complete,
            })
            .collect(),
    }
}

fn golden_states() -> Vec<TodoState> {
    vec![
        expected(1, 2, &[("task-1", "Milk", false)]),
        expected(
            2,
            3,
            &[("task-1", "Milk", false), ("task-2", "Eggs", false)],
        ),
        expected(
            3,
            4,
            &[
                ("task-1", "Milk", false),
                ("task-2", "Eggs", false),
                ("task-3", "Bread", false),
            ],
        ),
        expected(
            4,
            4,
            &[
                ("task-1", "Milk", true),
                ("task-2", "Eggs", false),
                ("task-3", "Bread", false),
            ],
        ),
        expected(
            4,
            4,
            &[
                ("task-1", "Milk", true),
                ("task-2", "Eggs", false),
                ("task-3", "Bread", false),
            ],
        ),
        expected(
            5,
            4,
            &[
                ("task-1", "Milk", false),
                ("task-2", "Eggs", false),
                ("task-3", "Bread", false),
            ],
        ),
        expected(
            6,
            4,
            &[("task-1", "Milk", false), ("task-3", "Bread", false)],
        ),
    ]
}

fn run(label: &str, actions: &[TodoCommand]) -> Replay {
    let initial = TodoState::new("Groceries".into()).unwrap();
    let mut host = StdHost::new_for_todo_scan(
        StdHostConfig {
            host_id: HostId::from(format!("host/todo-replay/{label}")),
            boot_id: BootId::from(format!("boot/todo-replay/{label}")),
            offer_generation: OfferGeneration(1),
        },
        &initial,
        64,
    )
    .unwrap();
    let plan = crate::flow_activation::authored_todo_plan_on_host(host.advertisement(), 64);
    assert!(conduit_core::verify_plan(&plan));
    let conduit_core::PlannedActivationEntry::Scan(scan) = &plan.activations[0] else {
        panic!("authored Todo must select its bounded scan");
    };
    assert_eq!(scan.initial_accumulator, initial.encode_info().unwrap());
    assert_eq!(scan.limits.maximum_items, 64);
    assert!(scan
        .selected_plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .all(|placement| placement.host_calls.is_empty()
            && placement.resources.is_empty()
            && placement.authority.is_empty()));
    let body = Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        SignId::from(format!("sign/todo-replay/{label}/born")),
    )
    .unwrap();
    let body_id = body.body_id.clone();
    let wake = body
        .wake(1, SignId::from(format!("sign/todo-replay/{label}/wake")))
        .unwrap()
        .1;
    let body_plan = BodyPlan::seal(
        &wake,
        vec![BodyPlotPlan {
            plot: ResidentPlot::new(
                plan.source_document_id.clone(),
                plan.checked_plot_id.clone(),
            ),
            plan,
        }],
    )
    .unwrap();
    let inputs: Vec<_> = actions
        .iter()
        .map(|command| ExternalForeInput {
            front_port_id: port_id("commands"),
            track: ConnectionTrack::Payload,
            bytes: command.encode_info().unwrap(),
        })
        .collect();
    let mut states = States::default();
    let report = host
        .run_body_plan_with_fore_to(
            BodyRunRequest {
                wake: &wake,
                plan: &body_plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            &inputs,
            true,
            &mut states,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap();
    assert_eq!(report.play.plan_id, body_plan.plan_id);
    assert_eq!(report.play.body_id, body_id);
    assert!(report.cleanup_failure.is_none());
    let signs = report.scan_child_signs.unwrap();
    assert_eq!(signs.len(), actions.len());
    for (index, sign) in signs.iter().enumerate() {
        assert_eq!(sign.parent_active_play_id, report.play.active_play_id);
        assert_eq!(usize::from(sign.invocation), index);
        assert!(!sign.events.is_empty());
    }
    let values: Vec<_> = states
        .0
        .iter()
        .map(|bytes| {
            let state = TodoState::decode_info(bytes).unwrap();
            assert_eq!(state.encode_info().unwrap(), *bytes);
            json!({"sha256":hash(bytes), "revision":state.revision, "next_id":state.next_id,
            "title":state.title, "items":state.items.iter().map(|item| json!({
                "id":item.id, "text":item.text, "complete":item.complete,
            })).collect::<Vec<_>>()})
        })
        .collect();
    let receipt = json!({"body_id":body_id,
        "source_document_id":body_plan.plots[0].plan.source_document_id,
        "checked_plot_id":body_plan.plots[0].plan.checked_plot_id,
        "plan_id":body_plan.plan_id, "play":report.play,
        "terminal":report.terminal, "failure":report.failure,
        "terminal_sign":report.terminal_sign,
        "initial_state_sha256":hash(&initial.encode_info().unwrap()),
        "command_sha256":inputs.iter().map(|input| hash(&input.bytes)).collect::<Vec<_>>(),
        "child_invocations":signs.len(), "states":values});
    Replay {
        states: states.0,
        terminal: report.terminal,
        receipt,
    }
}

#[test]
fn todo_core_replay_retains_exact_state_digests_across_two_fresh_hosts() {
    let actions = commands();
    let first = run("first", &actions);
    let second = run("second", &actions);
    assert_eq!(first.terminal, TerminalDisposition::Completed);
    assert_eq!(second.terminal, TerminalDisposition::Completed);
    assert_eq!(first.states.len(), 7);
    assert_eq!(first.states, second.states);
    assert_ne!(first.receipt["body_id"], second.receipt["body_id"]);
    assert_eq!(
        first.receipt["source_document_id"],
        second.receipt["source_document_id"]
    );
    for (bytes, expected) in first.states.iter().zip(golden_states()) {
        assert_eq!(TodoState::decode_info(bytes).unwrap(), expected);
    }
    assert_eq!(first.states[3], first.states[4]); // Repeated completion is idempotent.
    println!(
        "TODO_CORE_REPLAY_RECEIPT={}",
        json!({
        "schema":"conduit.test/todo-core-replay@1", "proof_class":"executable-hosted",
        "state_info_kind":conduit_todo_plot::TODO_STATE_INFO_ID,
        "command_info_kind":conduit_todo_plot::TODO_COMMAND_INFO_ID,
        "state_maximum_bytes":conduit_todo_plot::STATE_MAX_BYTES,
        "command_maximum_bytes":conduit_todo_plot::COMMAND_MAX_BYTES,
        "source_sha256":hash(SOURCE.as_bytes()), "first":first.receipt, "second":second.receipt,
            "limits":"Two independent fresh-Body replay fixtures; no persistence, Masks or Host-change continuity claim",
        })
    );
}

#[test]
fn todo_core_replay_refuses_bad_commands_without_state_for_the_refused_command() {
    assert_eq!(
        TodoCommand::Add {
            text: String::new()
        }
        .encode_info(),
        Err(conduit_todo_plot::TodoRefusal::InvalidText)
    );
    for (label, bad) in [
        (
            "missing-item",
            TodoCommand::Remove {
                id: "task-99".into(),
            },
        ),
        (
            "missing-completion-target",
            TodoCommand::SetComplete {
                id: "task-99".into(),
                complete: true,
            },
        ),
    ] {
        let mut actions = commands()[..3].to_vec();
        actions.push(bad);
        let result = run(label, &actions);
        assert!(matches!(
            result.terminal,
            TerminalDisposition::Failed { .. }
        ));
        assert_eq!(result.states.len(), 3);
        assert!(result.receipt["failure"].is_string());
        for (bytes, expected) in result.states.iter().zip(golden_states()) {
            assert_eq!(TodoState::decode_info(bytes).unwrap(), expected);
        }
        println!("TODO_CORE_REFUSAL_RECEIPT={}", result.receipt);
    }
}
