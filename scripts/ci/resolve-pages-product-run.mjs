#!/usr/bin/env node

import { appendFile, readFile } from "node:fs/promises";
import { hasRetainedArtifact, productCarrierRuns, resolveExactMainSource, resolveMergedPullSource, selectExactRun, selectExactSuccessfulRun } from "./pages-product-run-selection.mjs";

const event = JSON.parse(await readFile(process.env.GITHUB_EVENT_PATH, "utf8"));
const repository = process.env.GITHUB_REPOSITORY;
const token = process.env.GITHUB_TOKEN;
const output = process.env.GITHUB_OUTPUT;
if (!repository || !token || !output) throw new Error("GitHub workflow context is incomplete");

const requestedMain = event.inputs?.main_sha?.trim() ?? "";
const eventPullNumber = event.pull_request?.number;
const rawInputPullNumber = event.inputs?.pr_number;
const inputPullNumber = rawInputPullNumber === undefined || rawInputPullNumber === ""
  || rawInputPullNumber === 0 || rawInputPullNumber === "0"
  ? undefined : Number(rawInputPullNumber);
const requestedNumber = eventPullNumber ?? inputPullNumber;
if (requestedMain && requestedNumber !== undefined) {
  throw new Error("request exactly one Pages source: a merged pull request or current main");
}

let source;
let runId = "";
let carrierPresent = false;
let directMain = false;
if (requestedMain) {
  const repositoryDocument = await api(`/repos/${repository}`);
  if (repositoryDocument.default_branch !== "main") throw new Error("Pages current-main admission requires main to be the default branch");
  const reference = await api(`/repos/${repository}/git/ref/heads/main`);
  source = await resolveExactMainSource(
    requestedMain,
    reference?.object?.sha,
    (commit) => api(`/repos/${repository}/git/commits/${commit}`),
  );
  directMain = true;
} else {
  if (!Number.isSafeInteger(requestedNumber) || requestedNumber <= 0) {
    throw new Error("a merged pull request number or exact current main commit is required");
  }
  const pull = await api(`/repos/${repository}/pulls/${requestedNumber}`);
  source = await resolveMergedPullSource(
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
    const runs = await api(`/repos/${repository}/actions/runs?${query}`);
    const carriers = productCarrierRuns(runs.workflow_runs);
    const candidate = selectExactRun(carriers, source.sourceHead, requestedNumber);
    run = selectExactSuccessfulRun(carriers, source.sourceHead, requestedNumber);
    if (run) break;
    if (candidate?.status === "completed") {
      throw new Error(`exact-head Pages product run ${candidate.id} concluded ${candidate.conclusion ?? "without a conclusion"}`);
    }
    if (attempt < attempts) await new Promise((resolve) => setTimeout(resolve, intervalMilliseconds));
  }
  if (!run) throw new Error(`exact-head Pages product run did not complete successfully for pull request #${requestedNumber} within the bounded wait`);
  runId = String(run.id);
  const artifacts = await api(`/repos/${repository}/actions/runs/${runId}/artifacts?per_page=100`);
  carrierPresent = hasRetainedArtifact(artifacts.artifacts, "conduit-pages-carrier");
}

await appendFile(output, [
  `run_id=${runId}`,
  `carrier_present=${carrierPresent}`,
  `merge_commit=${source.mergeCommit}`,
  `source_head=${source.sourceHead}`,
  `source_tree=${source.sourceTree}`,
  `integration_base=${source.integrationBase}`,
  `integration_tree=${source.integrationTree}`,
  `pr_number=${requestedNumber ?? ""}`,
  `direct_main=${directMain}`,
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
