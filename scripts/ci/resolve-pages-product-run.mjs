#!/usr/bin/env node

import { appendFile, readFile } from "node:fs/promises";

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
if (!pull.merged_at || !/^[0-9a-f]{40}$/.test(pull.merge_commit_sha ?? "")
  || !/^[0-9a-f]{40}$/.test(pull.head?.sha ?? "")) {
  throw new Error(`pull request #${requestedNumber} is not an exact merged source`);
}

const query = new URLSearchParams({ event: "pull_request", head_sha: pull.head.sha, status: "success", per_page: "100" });
const runs = await api(`/repos/${repository}/actions/workflows/executable-book-pages.yml/runs?${query}`);
const run = runs.workflow_runs.find((candidate) =>
  candidate.conclusion === "success"
  && candidate.head_sha === pull.head.sha
  && candidate.pull_requests?.some((item) => item.number === requestedNumber));
if (!run) throw new Error(`no successful exact-head Pages product run admits pull request #${requestedNumber}`);

await appendFile(output, [
  `run_id=${run.id}`,
  `merge_commit=${pull.merge_commit_sha}`,
  `source_head=${pull.head.sha}`,
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
