#!/usr/bin/env node

import { appendFile, readFile } from "node:fs/promises";
import { resolveMergedPullSource, selectExactRun, selectExactSuccessfulRun } from "./pages-product-run-selection.mjs";

const event = JSON.parse(await readFile(process.env.GITHUB_EVENT_PATH, "utf8"));
const repository = process.env.GITHUB_REPOSITORY;
const token = process.env.GITHUB_TOKEN;
const output = process.env.GITHUB_OUTPUT;
if (!repository || !token || !output) throw new Error("GitHub workflow context is incomplete");

const requestedNumber = event.pull_request?.number ?? Number(event.inputs?.pr_number);
if (!Number.isSafeInteger(requestedNumber) || requestedNumber <= 0) {
  throw new Error("a merged pull request number is required");
}
const pull = await api(`/repos/${repository}/pulls/${requestedNumber}`);
const source = await resolveMergedPullSource(
  pull,
  (commit) => api(`/repos/${repository}/git/commits/${commit}`),
).catch((error) => {
  throw new Error(`pull request #${requestedNumber} cannot provide merged-tree provenance: ${error.message}`);
});

const query = new URLSearchParams({ event: "pull_request", head_sha: source.sourceHead, per_page: "100" });
const attempts = boundedInteger(process.env.CONDUIT_PRODUCT_RUN_ATTEMPTS, 80, 1, 120);
const intervalMilliseconds = boundedInteger(process.env.CONDUIT_PRODUCT_RUN_INTERVAL_MS, 15_000, 0, 60_000);
let run;
for (let attempt = 1; attempt <= attempts; attempt += 1) {
  const runs = await api(`/repos/${repository}/actions/workflows/executable-book-pages.yml/runs?${query}`);
  const candidate = selectExactRun(runs.workflow_runs, source.sourceHead, requestedNumber);
  run = selectExactSuccessfulRun(runs.workflow_runs, source.sourceHead, requestedNumber);
  if (run) break;
  if (candidate?.status === "completed") {
    throw new Error(`exact-head Pages product run ${candidate.id} concluded ${candidate.conclusion ?? "without a conclusion"}`);
  }
  if (attempt < attempts) await new Promise((resolve) => setTimeout(resolve, intervalMilliseconds));
}
if (!run) throw new Error(`exact-head Pages product run did not complete successfully for pull request #${requestedNumber} within the bounded wait`);

await appendFile(output, [
  `run_id=${run.id}`,
  `merge_commit=${source.mergeCommit}`,
  `source_head=${source.sourceHead}`,
  `source_tree=${source.sourceTree}`,
  `integration_base=${source.integrationBase}`,
  `integration_tree=${source.integrationTree}`,
  `pr_number=${requestedNumber}`,
  "",
].join("\n"));

async function api(endpoint) {
  const response = await fetch(`https://api.github.com${endpoint}`, {
    headers: {
      Accept: "application/vnd.github+json",
      Authorization: `Bearer ${token}`,
      "X-GitHub-Api-Version": "2022-11-28",
    },
  });
  if (!response.ok) throw new Error(`GitHub API ${endpoint} refused with ${response.status}`);
  return response.json();
}

function boundedInteger(value, fallback, minimum, maximum) {
  if (value === undefined) return fallback;
  const number = Number(value);
  if (!Number.isSafeInteger(number) || number < minimum || number > maximum) {
    throw new Error("Pages product run wait configuration is outside its admitted bound");
  }
  return number;
}
