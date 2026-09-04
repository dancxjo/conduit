#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const EXACT_SHA = /^[0-9a-f]{40}$/;

export async function retireMergedPullBranch({ pull, repository, defaultBranch, request = fetch }) {
  if (!pull?.merged || !EXACT_SHA.test(pull?.head?.sha ?? "")) {
    return { disposition: "retained", reason: "pull-not-merged" };
  }
  if (pull?.head?.repo?.full_name !== repository || pull?.base?.repo?.full_name !== repository) {
    return { disposition: "retained", reason: "non-repository-branch" };
  }
  const branch = pull.head.ref;
  const target = pull.base.ref;
  if (!validRef(branch) || !validRef(target) || branch === defaultBranch || target !== defaultBranch) {
    throw new Error("merged branch retirement is outside the default-branch contract");
  }

  const dependents = await listDependents(request, repository, branch);
  const retargeted = [];
  for (const dependent of dependents) {
    if (!Number.isSafeInteger(dependent?.number) || !EXACT_SHA.test(dependent?.head?.sha ?? "")) {
      throw new Error("dependent pull request identity is malformed");
    }
    const response = await request(api(repository, `/pulls/${dependent.number}`), jsonRequest("PATCH", { base: target }));
    if (!response.ok) throw new Error(`retarget pull request #${dependent.number} refused with HTTP ${response.status}`);
    const updated = await response.json();
    if (updated?.base?.ref !== target || updated?.head?.sha !== dependent.head.sha) {
      throw new Error(`retarget pull request #${dependent.number} changed candidate identity or retained the old base`);
    }
    retargeted.push({ number: dependent.number, head_sha: dependent.head.sha });
  }

  const remaining = await listDependents(request, repository, branch);
  if (remaining.length !== 0) throw new Error("dependent pull requests remain on the retiring branch");

  const deletion = await request(api(repository, `/git/refs/heads/${encodeRef(branch)}`), jsonRequest("DELETE"));
  if (!deletion.ok) {
    throw new Error(`delete merged branch ${branch} refused with HTTP ${deletion.status}`);
  }
  return {
    disposition: "deleted",
    branch,
    target,
    retargeted,
  };
}

async function listDependents(request, repository, branch) {
  const pulls = [];
  for (let page = 1; page <= 10; page += 1) {
    const query = new URLSearchParams({ state: "open", base: branch, per_page: "100", page: String(page) });
    const response = await request(`${api(repository, "/pulls")}?${query}`, jsonRequest("GET"));
    if (!response.ok) throw new Error(`list dependents refused with HTTP ${response.status}`);
    const body = await response.json();
    if (!Array.isArray(body)) throw new Error("dependent pull response is malformed");
    pulls.push(...body);
    if (body.length < 100) break;
  }
  return pulls;
}

function jsonRequest(method, body) {
  const headers = {
    Accept: "application/vnd.github+json",
    Authorization: `Bearer ${process.env.GITHUB_TOKEN ?? ""}`,
    "X-GitHub-Api-Version": "2022-11-28",
  };
  return body === undefined ? { method, headers } : { method, headers, body: JSON.stringify(body) };
}

function api(repository, path) {
  if (!/^[^/]+\/[^/]+$/.test(repository ?? "")) throw new Error("repository identity is malformed");
  return `https://api.github.com/repos/${repository}${path}`;
}

function validRef(value) {
  return typeof value === "string" && value.length > 0 && value.length <= 255
    && !value.startsWith("/") && !value.endsWith("/") && !value.includes("..") && !/[~^:?*[\\\s]/.test(value);
}

function encodeRef(value) {
  return value.split("/").map(encodeURIComponent).join("/");
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const event = JSON.parse(await readFile(process.env.GITHUB_EVENT_PATH, "utf8"));
  const result = await retireMergedPullBranch({
    pull: event.pull_request,
    repository: process.env.GITHUB_REPOSITORY ?? "",
    defaultBranch: event.repository?.default_branch,
  });
  process.stdout.write(`${JSON.stringify({ schema: "conduit.ci.merged-branch-retirement/v1", ...result }, null, 2)}\n`);
}
