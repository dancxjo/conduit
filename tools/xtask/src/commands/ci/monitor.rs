#![cfg_attr(test, allow(dead_code))]

use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    process::Command,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const MAX_BACKOFF_SECONDS: u64 = 900;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct WorkflowRun {
    id: u64,
    status: String,
    conclusion: Option<String>,
    html_url: String,
}

#[derive(Debug, Deserialize)]
struct WorkflowRuns {
    workflow_runs: Vec<WorkflowRun>,
}

#[derive(Debug, Eq, PartialEq)]
enum Observation {
    Runs(Vec<WorkflowRun>),
    Limited {
        retry_after: Option<u64>,
        reset_epoch: Option<u64>,
    },
    Transient(String),
}

pub fn run(
    repository: &str,
    tracked_runs: &[u64],
    interval_seconds: u64,
    max_requests: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    validate(repository, tracked_runs, interval_seconds, max_requests)?;
    let tracked = tracked_runs.iter().copied().collect::<BTreeSet<_>>();
    let mut failures = 0_u32;

    for request in 1..=max_requests {
        let observation = request_runs(repository)?;
        let observation_failed = match &observation {
            Observation::Runs(runs) => tracked
                .iter()
                .any(|tracked_id| !runs.iter().any(|run| run.id == *tracked_id)),
            Observation::Limited { .. } | Observation::Transient(_) => true,
        };
        let now = unix_seconds();
        let (done, delay) = next_step(&tracked, observation, interval_seconds, failures, now)?;
        if done {
            return Ok(());
        }

        failures = if observation_failed {
            failures.saturating_add(1)
        } else {
            0
        };
        require_request_remaining(request, max_requests, tracked_runs)?;
        eprintln!(
            "next observation in {delay}s (request {request}/{max_requests}); tracked runs remain unchanged"
        );
        thread::sleep(Duration::from_secs(delay));
    }
    unreachable!("validated request budget is nonzero")
}

fn require_request_remaining(
    request: u32,
    max_requests: u32,
    tracked: &[u64],
) -> Result<(), String> {
    if request < max_requests {
        return Ok(());
    }
    Err(format!(
        "monitor request budget exhausted after {request} batched observations; run identities {tracked:?} remain nonterminal or unobserved"
    ))
}

fn validate(
    repository: &str,
    tracked_runs: &[u64],
    interval_seconds: u64,
    max_requests: u32,
) -> Result<(), String> {
    if repository.split_once('/').is_none() {
        return Err("--repo must use owner/name form".into());
    }
    if tracked_runs.is_empty() || tracked_runs.contains(&0) {
        return Err("at least one nonzero run ID is required".into());
    }
    if interval_seconds == 0 {
        return Err("--interval-seconds must be at least 1".into());
    }
    if max_requests == 0 {
        return Err("--max-requests must be at least 1".into());
    }
    Ok(())
}

fn request_runs(repository: &str) -> Result<Observation, Box<dyn std::error::Error>> {
    let output = Command::new("gh")
        .args([
            "api",
            "--include",
            "-H",
            "X-GitHub-Api-Version: 2022-11-28",
            &format!("repos/{repository}/actions/runs?per_page=100"),
        ])
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Ok(parse_response(output.status.success(), &stdout, &stderr))
}

fn parse_response(success: bool, stdout: &str, stderr: &str) -> Observation {
    let headers = stdout
        .lines()
        .take_while(|line| !line.trim_start().starts_with('{'))
        .collect::<Vec<_>>();
    let retry_after = header(&headers, "retry-after").and_then(|value| value.parse().ok());
    let reset_epoch = header(&headers, "x-ratelimit-reset").and_then(|value| value.parse().ok());
    let status_limited = stdout
        .lines()
        .rfind(|line| line.starts_with("HTTP/"))
        .is_some_and(|line| line.contains(" 403 ") || line.contains(" 429 "));
    let text_limited =
        stderr.contains("rate limit") || stderr.contains("HTTP 403") || stderr.contains("HTTP 429");
    if status_limited || text_limited {
        return Observation::Limited {
            retry_after,
            reset_epoch,
        };
    }
    if !success {
        return Observation::Transient(stderr.trim().to_owned());
    }
    let Some(body_start) = stdout.find('{') else {
        return Observation::Transient("GitHub response omitted JSON".into());
    };
    match serde_json::from_str::<WorkflowRuns>(&stdout[body_start..]) {
        Ok(response) => Observation::Runs(response.workflow_runs),
        Err(error) => Observation::Transient(format!("invalid GitHub response: {error}")),
    }
}

fn header<'a>(headers: &'a [&str], name: &str) -> Option<&'a str> {
    headers.iter().rev().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.eq_ignore_ascii_case(name).then_some(value.trim())
    })
}

fn next_step(
    tracked: &BTreeSet<u64>,
    observation: Observation,
    interval: u64,
    failures: u32,
    now: u64,
) -> Result<(bool, u64), String> {
    match observation {
        Observation::Runs(runs) => {
            let by_id = runs
                .into_iter()
                .map(|run| (run.id, run))
                .collect::<BTreeMap<_, _>>();
            let missing = tracked
                .iter()
                .filter(|id| !by_id.contains_key(id))
                .copied()
                .collect::<Vec<_>>();
            if !missing.is_empty() {
                eprintln!(
                    "tracked runs absent from the batch: {missing:?}; no terminal state inferred"
                );
                return Ok((false, backoff(interval, failures, tracked)));
            }
            let mut all_terminal = true;
            for id in tracked {
                let run = &by_id[id];
                println!(
                    "run {} status={} conclusion={} {}",
                    run.id,
                    run.status,
                    run.conclusion.as_deref().unwrap_or("-"),
                    run.html_url
                );
                all_terminal &= run.status == "completed";
            }
            Ok((all_terminal, interval))
        }
        Observation::Limited {
            retry_after,
            reset_epoch,
        } => {
            let header_delay =
                retry_after.or_else(|| reset_epoch.map(|reset| reset.saturating_sub(now)));
            Ok((
                false,
                header_delay
                    .unwrap_or_else(|| backoff(interval, failures, tracked))
                    .clamp(1, MAX_BACKOFF_SECONDS),
            ))
        }
        Observation::Transient(message) => {
            eprintln!("transient observation failure: {message}; no terminal state inferred");
            Ok((false, backoff(interval, failures, tracked)))
        }
    }
}

fn backoff(interval: u64, failures: u32, tracked: &BTreeSet<u64>) -> u64 {
    let exponent = failures.min(3);
    let base = interval
        .saturating_mul(1_u64 << exponent)
        .min(MAX_BACKOFF_SECONDS);
    let jitter_bound = (base / 10).max(1);
    let seed = tracked.iter().fold(failures as u64, |value, id| value ^ id);
    base.saturating_add(seed % jitter_bound)
        .min(MAX_BACKOFF_SECONDS)
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracked() -> BTreeSet<u64> {
        [10, 20].into_iter().collect()
    }

    #[test]
    fn missing_delay_and_budget_are_refused() {
        assert!(validate("dancxjo/conduit", &[10], 0, 1).is_err());
        assert!(validate("dancxjo/conduit", &[10], 1, 0).is_err());
    }

    #[test]
    fn continuously_nonterminal_observation_cannot_exceed_request_budget() {
        let mut requests = 0;
        loop {
            requests += 1;
            if require_request_remaining(requests, 3, &[10]).is_err() {
                break;
            }
        }
        assert_eq!(requests, 3);
    }

    #[test]
    fn secondary_limit_uses_bounded_exponential_backoff() {
        let (_, delay) = next_step(
            &tracked(),
            Observation::Limited {
                retry_after: None,
                reset_epoch: None,
            },
            120,
            2,
            1_000,
        )
        .unwrap();
        assert!((480..=528).contains(&delay));
    }

    #[test]
    fn primary_exhaustion_honors_reset_without_unbounded_sleep() {
        let (_, delay) = next_step(
            &tracked(),
            Observation::Limited {
                retry_after: None,
                reset_epoch: Some(5_000),
            },
            120,
            0,
            4_000,
        )
        .unwrap();
        assert_eq!(delay, MAX_BACKOFF_SECONDS);
    }

    #[test]
    fn transient_failure_retains_identity_and_backs_off() {
        let (done, delay) = next_step(
            &tracked(),
            Observation::Transient("network unavailable".into()),
            120,
            1,
            0,
        )
        .unwrap();
        assert!(!done);
        assert!(delay >= 240);
    }

    #[test]
    fn eventual_terminal_state_requires_every_tracked_run() {
        let running = WorkflowRun {
            id: 10,
            status: "in_progress".into(),
            conclusion: None,
            html_url: "https://example/10".into(),
        };
        let terminal = WorkflowRun {
            id: 20,
            status: "completed".into(),
            conclusion: Some("success".into()),
            html_url: "https://example/20".into(),
        };
        assert!(
            !next_step(
                &tracked(),
                Observation::Runs(vec![running.clone(), terminal.clone()]),
                120,
                0,
                0,
            )
            .unwrap()
            .0
        );
        let completed = WorkflowRun {
            status: "completed".into(),
            conclusion: Some("failure".into()),
            ..running
        };
        assert!(
            next_step(
                &tracked(),
                Observation::Runs(vec![completed, terminal]),
                120,
                0,
                0,
            )
            .unwrap()
            .0
        );
    }

    #[test]
    fn absent_run_is_never_inferred_terminal() {
        let only_one = WorkflowRun {
            id: 10,
            status: "completed".into(),
            conclusion: Some("success".into()),
            html_url: "https://example/10".into(),
        };
        assert!(
            !next_step(&tracked(), Observation::Runs(vec![only_one]), 120, 0, 0,)
                .unwrap()
                .0
        );
    }

    #[test]
    fn response_parser_retains_rate_headers() {
        let parsed = parse_response(
            false,
            "HTTP/2 429 Too Many Requests\nretry-after: 37\nx-ratelimit-reset: 99\n\n{}",
            "gh: HTTP 429",
        );
        assert_eq!(
            parsed,
            Observation::Limited {
                retry_after: Some(37),
                reset_epoch: Some(99)
            }
        );
    }
}
