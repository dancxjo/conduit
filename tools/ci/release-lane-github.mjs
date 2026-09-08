#!/usr/bin/env node
import { appendFile } from "node:fs/promises";
import { decideReleaseLane } from "./release-lane-controller.mjs";

const apply = process.argv.includes("--apply");
const repository = process.env.GITHUB_REPOSITORY;
const token = process.env.GH_TOKEN || process.env.GITHUB_TOKEN;
if (!repository?.includes("/")) throw new Error("GITHUB_REPOSITORY must be owner/name");
if (!token) throw new Error("GH_TOKEN or GITHUB_TOKEN is required");

const headers = {
  Accept: "application/vnd.github+json",
  Authorization: `Bearer ${token}`,
  "X-GitHub-Api-Version": "2022-11-28",
  "User-Agent": "conduit-release-lane-controller",
};

async function api(path, options = {}) {
  const response = await fetch(`https://api.github.com/repos/${repository}${path}`, {
    ...options,
    headers: { ...headers, ...(options.headers ?? {}) },
  });
  if (!response.ok) {
    const body = await response.text();
    throw new Error(`${options.method ?? "GET"} ${path}: ${response.status} ${body}`);
  }
  if (response.status === 204) return null;
  return response.json();
}

function body(value) {
  return { body: JSON.stringify(value), headers: { "Content-Type": "application/json" } };
}

async function jobsFor(run) {
  const response = await api(`/actions/runs/${run.id}/jobs?per_page=100&filter=latest`);
  return response.jobs.map((job) => ({
    id: job.id,
    name: job.name,
    status: job.status,
    conclusion: job.conclusion,
    startedAt: job.started_at,
    completedAt: job.completed_at,
    url: job.html_url,
  }));
}

async function observe() {
  const [pulls, runResponse, issues] = await Promise.all([
    api("/pulls?state=open&base=main&per_page=100&sort=created&direction=asc"),
    api("/actions/workflows/promotion.yml/runs?per_page=100"),
    api("/issues?state=all&labels=release-liveness%2Frepair-needed&per_page=100"),
  ]);
  const releases = pulls.filter((pull) =>
    pull.head.repo?.full_name === repository && /^release\/[0-9a-f]{40}$/.test(pull.head.ref),
  );
  const releaseBranches = new Set(releases.map((pull) => pull.head.ref));
  const relevantRuns = runResponse.workflow_runs.filter((run) =>
    run.event === "pull_request" && releaseBranches.has(run.head_branch),
  );
  const jobLists = await Promise.all(relevantRuns.map(jobsFor));
  const runs = relevantRuns.map((run, index) => ({
    id: run.id,
    headSha: run.head_sha,
    status: run.status,
    conclusion: run.conclusion,
    createdAt: run.created_at,
    updatedAt: run.updated_at,
    explicitRepair: (run.run_attempt ?? 1) > 1,
    jobs: jobLists[index],
    url: run.html_url,
  }));

  const candidates = [];
  for (const pull of releases) {
    const branchRuns = runs.filter((run) => relevantRuns.find(({ id }) => id === run.id)?.head_branch === pull.head.ref);
    const heads = new Set([...branchRuns.map((run) => run.headSha), pull.head.sha]);
    for (const headSha of heads) {
      candidates.push({
        prNumber: pull.number,
        branch: pull.head.ref,
        headSha,
        createdAt: branchRuns.find((run) => run.headSha === headSha)?.createdAt ?? pull.created_at,
        attempts: branchRuns.filter((run) => run.headSha === headSha),
      });
    }
  }
  const escalations = issues
    .filter((issue) => !issue.pull_request)
    .flatMap((issue) => [...(issue.body ?? "").matchAll(/<!-- conduit-release-liveness:([^ ]+) -->/g)].map((match) => match[1]));
  return { now: Date.now(), candidates, escalations, pulls: releases };
}

async function commentOnce(prNumber, marker, message) {
  const comments = await api(`/issues/${prNumber}/comments?per_page=100`);
  if (comments.some((comment) => comment.body?.includes(marker))) return;
  await api(`/issues/${prNumber}/comments`, {
    method: "POST",
    ...body({ body: `${marker}\n${message}` }),
  });
}

function evidenceLines(action) {
  const jobs = action.failedJobs?.length
    ? action.failedJobs.map((job) => `- ${job.name}: ${job.url ?? `job ${job.id}`}`).join("\n")
    : "- No decisive job failure was reported; inspect the run liveness context.";
  return `Release PR: #${action.prNumber}\nExact head: \`${action.headSha}\`\nRun: ${action.runUrl ?? action.runId}\nRelevant jobs:\n${jobs}`;
}

async function ensureLivenessLabel() {
  try {
    await api("/labels/release-liveness%2Frepair-needed");
  } catch (error) {
    if (!String(error).includes(" 404 ")) throw error;
    await api("/labels", {
      method: "POST",
      ...body({
        name: "release-liveness/repair-needed",
        color: "B60205",
        description: "A bounded release-lane watchdog needs repair attention",
      }),
    });
  }
}

async function enact(snapshot, decision) {
  for (const action of decision.actions.cancel) {
    if (action.runId) {
      try {
        await api(`/actions/runs/${action.runId}/cancel`, { method: "POST" });
      } catch (error) {
        if (!String(error).includes(" 409 ")) throw error;
      }
    }
    await commentOnce(
      action.prNumber,
      `<!-- conduit-release-${action.state}:${action.runId ?? action.headSha} -->`,
      `Release lane classified this attempt as **${action.state}** (${action.reason}).\n\n${evidenceLines(action)}`,
    );
  }

  for (const action of decision.actions.close) {
    const pull = snapshot.pulls.find((candidate) => candidate.number === action.prNumber);
    if (!pull || pull.head.sha !== action.headSha) continue;
    await commentOnce(
      action.prNumber,
      `<!-- conduit-release-superseded:${action.headSha} -->`,
      `This release never materially started and a newer queued release superseded it. Exact head: \`${action.headSha}\`.`,
    );
    await api(`/pulls/${action.prNumber}`, { method: "PATCH", ...body({ state: "closed" }) });
  }

  for (const action of decision.actions.escalate) {
    await ensureLivenessLabel();
    await api("/issues", {
      method: "POST",
      ...body({
        title: `[release-liveness] Promotion ${action.runId} is stuck`,
        labels: ["release-liveness/repair-needed"],
        body: `<!-- conduit-release-liveness:${action.key} -->\nThe bounded release watchdog classified this promotion as **stuck** (${action.reason}). It remains the lane owner; no replacement train was started.\n\n${evidenceLines(action)}`,
      }),
    });
  }
}

const snapshot = await observe();
const decision = decideReleaseLane(snapshot);
if (apply) await enact(snapshot, decision);
const serialized = `${JSON.stringify(decision, null, 2)}\n`;
process.stdout.write(serialized);
if (process.env.GITHUB_STEP_SUMMARY) {
  await appendFile(process.env.GITHUB_STEP_SUMMARY, `## Release lane decision\n\n\`\`\`json\n${serialized}\`\`\`\n`);
}
