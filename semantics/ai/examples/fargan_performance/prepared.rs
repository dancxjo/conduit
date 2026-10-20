//! Paired full-frame preparation lifetime observations; one production kernel.
use super::{allocation, artifact, frame, plan, runtime, shared, state, timing, RefCell};
use serde_json::json;
#[path = "prepared_conformance.rs"]
mod conformance;
use std::{path::Path, time::Instant};
fn observe(
    run: impl FnOnce(&mut dyn FnMut(), &mut dyn FnMut()) -> Option<runtime::EncodedResultAndTiming>,
) -> Result<(runtime::EncodedResultAndTiming, serde_json::Value), Box<dyn std::error::Error>> {
    let wall = Instant::now();
    let cpu = timing::cpu();
    let prep = RefCell::new(None);
    let play = RefCell::new(None);
    let play_start = RefCell::new(None);
    let decode_start = RefCell::new(None);
    allocation::start();
    let result = run(
        &mut || {
            let counts = allocation::finish();
            *prep.borrow_mut() = Some((timing::ns(wall.elapsed()), counts));
            *play_start.borrow_mut() = Some(Instant::now());
            allocation::start();
        },
        &mut || {
            let counts = allocation::finish();
            *play.borrow_mut() = Some((timing::ns(play_start.borrow().unwrap().elapsed()), counts));
            *decode_start.borrow_mut() = Some(Instant::now());
            allocation::start();
        },
    );
    let decoded = allocation::finish();
    let decode_ns = timing::ns(decode_start.borrow().ok_or("missing Play end")?.elapsed());
    let whole_ns = timing::ns(wall.elapsed());
    let cpu_ns = cpu.zip(timing::cpu()).map(|(a, b)| b - a);
    let (prep_ns, prep_alloc) = prep.into_inner().ok_or("missing Play start")?;
    let (play_ns, play_alloc) = play.into_inner().ok_or("missing Play end")?;
    assert_eq!(
        play_alloc.allocations + play_alloc.reallocations,
        0,
        "bounded Play"
    );
    Ok((
        result.ok_or("no complete result")?,
        json!({"whole_call_wall_ns":whole_ns,"whole_call_process_cpu_ns":cpu_ns,
        "pre_play":{"wall_ns":prep_ns,"allocator":prep_alloc},"play":{"wall_ns":play_ns,"allocator":play_alloc},
        "output_decode":{"wall_ns":decode_ns,"allocator":decoded}}),
    ))
}
struct FrameKey<'a> {
    repetition: u32,
    index: usize,
    profile: &'a str,
    plan_sha: &'a str,
}
fn retain_frame(
    output: &Path,
    raw: &mut artifact::Raw,
    key: FrameKey<'_>,
    result: &runtime::EncodedResultAndTiming,
    metrics: serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let FrameKey {
        repetition,
        index,
        profile,
        plan_sha,
    } = key;
    let signs = super::sign_text::render(&result.signs);
    let signs_file = format!("{profile}-{repetition}-{index}.signs.txt");
    std::fs::write(output.join(&signs_file), &signs)?;
    let values = result
        .outputs
        .iter()
        .map(|(n, v)| (n, v.canonical_bytes().unwrap()))
        .collect::<Vec<_>>();
    let values_file = format!("{profile}-{repetition}-{index}.outputs.json");
    artifact::json(&output.join(&values_file), &values)?;
    raw.row(&json!({"phase":"paired-fargan-frame","profile":profile,"repetition":repetition,"frame":index,"plan_sha256":plan_sha,
        "nodes":result.nodes,"cords":result.cords,"metrics":metrics,"steps":result.work.steps,"numeric_steps":result.work.numeric_steps,
        "expression_calls":result.work.expression_calls,"signs":result.signs.len(),"signs_file":signs_file,"signs_sha256":artifact::digest(signs.as_bytes()),
        "outputs_file":values_file,"outputs_sha256":artifact::digest(&std::fs::read(output.join(&values_file))?),
        "owner_preparation_ns":timing::ns(result.preparation),"kernel_play_ns":timing::ns(result.execution),
        "canonical_intermediates_avoided":result.work.compact_values,
        "canonical_bytes_avoided":result.work.compact_logical_bytes,
        "canonical_values":result.work.canonical_values,"canonical_bytes":result.work.canonical_bytes,
        "native_values":result.work.compact_values,"native_bytes":result.work.compact_bytes,
        "logical_invocations":result.work.logical_invocations,"physical_decisions":result.work.physical_decisions,
        "local_expression_calls":result.work.local_expression_calls,
        "kernel_step_ns":timing::ns(result.work.kernel_steps),
        "local_expression_ns_included_in_kernel":timing::ns(result.work.local_expressions),
        "hosted_expression_ns_outside_kernel":timing::ns(result.work.expressions),
        "compact_binding_bytes":result.work.compact_binding_bytes,"scheduler_inline_bytes":result.work.scheduler_bytes}))?;
    Ok(())
}
pub(super) fn run(
    output: &Path,
    repetitions: u32,
    schema: &plan::SourceSchema,
    resources: &shared::Resources,
    oracle: &frame::Oracle,
    raw: &mut artifact::Raw,
    compact_local: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut fresh_walls = Vec::new();
    let mut retained_walls = Vec::new();
    let mut previous_sign_count = 0;
    for repetition in 0..repetitions {
        let (plan, cold) =
            timing::measure(|| plan::prepare_entry_plan(schema, &plan::source(), frame::ENTRY));
        raw.row(&json!({"phase":"cold-check-expand-plan","repetition":repetition,"metrics":cold}))?;
        artifact::json(
            &output.join(format!("plan-repetition-{repetition}.json")),
            &plan,
        )?;
        let plan_sha = artifact::digest(&serde_json::to_vec(&plan)?);
        let candidate = if compact_local {
            let (candidate, cold) = timing::measure(|| {
                plan::prepare_entry_plan_profile(
                    schema,
                    &plan::source(),
                    frame::ENTRY,
                    plan::NumericProfile::CompactDagLocalExpressions,
                )
            });
            raw.row(&json!({"phase":"candidate-check-expand-plan","repetition":repetition,"metrics":cold}))?;
            conformance::same_source(&plan, &candidate);
            candidate
        } else {
            plan.clone()
        };
        artifact::json(
            &output.join(format!("candidate-plan-repetition-{repetition}.json")),
            &candidate,
        )?;
        let candidate_sha = artifact::digest(&serde_json::to_vec(&candidate)?);
        let mut state = oracle.state.clone();
        let mut history = oracle.history;
        let mut previous = oracle.period;
        let first = &oracle.cases[0];
        let (initial, initial_input) = timing::measure(|| {
            frame::inputs(
                schema,
                &first.features,
                &history,
                first.period,
                previous,
                &state,
            )
        });
        raw.row(&json!({"phase":"initial-input-encoding","repetition":repetition,"metrics":initial_input}))?;
        let (mut retained, preparation) = timing::measure(|| {
            runtime::session::PreparedTrajectory::new(
                candidate,
                resources,
                initial,
                oracle.cases.len(),
            )
        });
        raw.row(&json!({"phase":"cold-prepared-trajectory","repetition":repetition,"metrics":preparation,"invocation_reservation":oracle.cases.len(),"sign_slots_per_invocation":32768}))?;
        let address = retained.kernel_address();
        let (mut pcm_error, mut state_error, mut history_error) = (0f32, 0f32, 0f32);
        for (index, case) in oracle.cases.iter().enumerate() {
            let (input, encoding) = timing::measure(|| {
                frame::inputs(
                    schema,
                    &case.features,
                    &history,
                    case.period,
                    previous,
                    &state,
                )
            });
            raw.row(&json!({"phase":"frame-input-encoding","repetition":repetition,"frame":index,"metrics":encoding}))?;
            if index > 0 {
                let a = retained.active();
                let next = conduit_core::bind_active_play(
                    &a.plan_id,
                    &a.host_id,
                    &a.boot_id,
                    index as u64,
                );
                let (old, rearm) =
                    timing::measure(|| retained.begin_next(next, input.clone()).unwrap());
                assert_eq!(old.signs.events().count(), previous_sign_count);
                raw.row(&json!({"phase":"prepared-rearm","repetition":repetition,"frame":index,"metrics":rearm,"previous_signs_transferred":previous_sign_count}))?;
                // Old evidence remains owned by this returned log until this
                // explicit boundary drops it; full raw events are already retained.
                let ((), release) = timing::measure(|| drop(old));
                raw.row(&json!({"phase":"previous-evidence-release","repetition":repetition,"frame":index,"metrics":release}))?;
            }
            let ((fresh_plan, fresh_input), binding) =
                timing::measure(|| (plan.clone(), input.clone()));
            raw.row(&json!({"phase":"fresh-call-bindings","repetition":repetition,"frame":index,"metrics":binding}))?;
            let fresh_run = |before: &mut dyn FnMut(), after: &mut dyn FnMut()| {
                runtime::run_entry_plan_observed_at_sequence(
                    fresh_plan,
                    resources,
                    fresh_input,
                    runtime::ExecutionMode::Normal,
                    index as u64,
                    before,
                    after,
                )
            };
            let retained_run = |before: &mut dyn FnMut(), after: &mut dyn FnMut()| {
                retained.run_observed(runtime::ExecutionMode::Normal, before, after)
            };
            let ((fresh, fresh_metrics), (actual, retained_metrics)) = if index.is_multiple_of(2) {
                (observe(fresh_run)?, observe(retained_run)?)
            } else {
                let r = observe(retained_run)?;
                let f = observe(fresh_run)?;
                (f, r)
            };
            assert_eq!(actual.outputs, fresh.outputs, "paired frame {index}");
            conformance::compare(&fresh, &actual, index, compact_local);
            previous_sign_count = actual.signs.len();
            assert_eq!(retained.kernel_address(), address);
            fresh_walls.push(fresh_metrics["whole_call_wall_ns"].as_u64().unwrap());
            retained_walls.push(retained_metrics["whole_call_wall_ns"].as_u64().unwrap());
            let (inspection, inspection_metrics) =
                timing::measure(|| -> Result<(), Box<dyn std::error::Error>> {
                    retain_frame(
                        output,
                        raw,
                        FrameKey {
                            repetition,
                            index,
                            profile: "fresh",
                            plan_sha: &plan_sha,
                        },
                        &fresh,
                        fresh_metrics,
                    )?;
                    retain_frame(
                        output,
                        raw,
                        FrameKey {
                            repetition,
                            index,
                            profile: if compact_local {
                                "prepared-compact-local"
                            } else {
                                "prepared"
                            },
                            plan_sha: &candidate_sha,
                        },
                        &actual,
                        retained_metrics,
                    )?;
                    Ok(())
                });
            inspection?;
            raw.row(&json!({"phase":"paired-evidence-reconstruction-and-write","repetition":repetition,"frame":index,"metrics":inspection_metrics}))?;
            for (a, b) in frame::pcm(&actual).iter().zip(case.pcm) {
                pcm_error = pcm_error.max((a - b).abs());
            }
            let ((next_state, next_history, active), acceptance) = timing::measure(|| {
                let names: Vec<_> = actual.outputs.keys().cloned().collect();
                let active = retained.active().clone();
                assert!(retained.accept_outputs(&active, &names));
                let next_history: [f32; 128] = state::floats(&actual.outputs["next_history"])
                    .try_into()
                    .unwrap();
                (
                    state::State::from_result(&actual.value),
                    next_history,
                    active,
                )
            });
            raw.row(&json!({"phase":"output-acceptance-state-advance","repetition":repetition,"frame":index,"metrics":acceptance}))?;
            state = next_state;
            history = next_history;
            previous = case.period;
            for (a, b) in state.flattened().iter().zip(case.next.flattened()) {
                state_error = state_error.max((a - b).abs());
            }
            for (a, b) in history.iter().zip(case.history) {
                history_error = history_error.max((a - b).abs());
            }
            raw.row(&json!({"phase":"paired-frame-conformance","repetition":repetition,"frame":index,"active":active,"outputs_equal":true,"all_signs_equal_with_request_generation_offset":(!compact_local).then_some(index),"evidence_comparison":if compact_local { "per-node-ordered-value-transactions" } else { "full-events-request-generation-offset" },"request_generation_offset":index,
                "pcm_max_abs_error":pcm_error,"active_state_max_abs_error":state_error,"conditioning_history_max_abs_error":history_error}))?;
            let ((), disposal) = timing::measure(|| drop((fresh, actual)));
            raw.row(&json!({"phase":"paired-frame-result-release","repetition":repetition,"frame":index,"metrics":disposal}))?;
        }
        assert!(pcm_error < 0.002 && state_error < 0.05 && history_error < 0.001);
        let ((), disposal) = timing::measure(|| drop(retained));
        raw.row(&json!({"phase":"prepared-trajectory-release","repetition":repetition,"metrics":disposal}))?;
    }
    artifact::json(
        &output.join("summary.json"),
        &json!({"schema":if compact_local { "conduit.fargan-compact-local-performance@1" } else { "conduit.fargan-prepared-performance@1" },"repetitions":repetitions,"frames_per_repetition":24,"prepared_final_disposal_measured":true,"compact_local":compact_local,
        "fresh_whole_call":timing::distribution(&fresh_walls),"prepared_whole_call":timing::distribution(&retained_walls),"source_sha256":artifact::digest(plan::source().as_bytes()),
        "scope":"Paired full authored frame; production prepared kernel and owners, fixture acceptance controller; explicit compact pointwise DAG/local expression candidate when selected; not ordinary Body integration, realtime or intelligibility proof.",
        "accounting":"Cold schema/model common; baseline and selected candidate Plan preparation disclosed separately; cold trajectory includes all evidence reservations; rearm includes staging clone; old evidence release and complete output acceptance/state advance separately measured; whole frame includes output decoding; fresh binding clones and final prepared-owner/session destruction separately disclosed. No hardware memory-traffic claim; canonical and native successful store counters reported per profile. Separate Plan IDs and raw full event logs retained; different physical schedules compare ordered per-node value transactions."}),
    )?;
    Ok(())
}
