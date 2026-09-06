#!/usr/bin/env node

import { appendFile, writeFile } from "node:fs/promises";

const EXACT_SHA = /^[0-9a-f]{40,64}$/;

function validPull(pull, repository) {
  return Number.isSafeInteger(pull?.number)
    && pull.number > 0
    && pull.state === "open"
    && typeof pull?.user?.login === "string"
    && pull.user.login.length > 0
    && typeof pull?.head?.ref === "string"
    && pull.head.ref.length > 0
    && EXACT_SHA.test(pull.head.sha ?? "")
    && pull?.head?.repo?.full_name === repository
    && typeof pull?.base?.ref === "string"
    && pull.base.ref.length > 0
    && pull?.base?.repo?.full_name === repository;
}

export function classifyPullStack(pulls, pullNumber, candidateSha, repository) {
  if (!Array.isArray(pulls) || !Number.isSafeInteger(pullNumber) || pullNumber < 1
    || !EXACT_SHA.test(candidateSha ?? "") || !/^[^/]+\/[^/]+$/.test(repository ?? "")) {
    throw new Error("stack classification inputs are malformed");
  }
  const current = pulls.find((pull) => pull?.number === pullNumber);
  if (!validPull(current, repository) || current.head.sha !== candidateSha) {
    throw new Error("current pull request is not an exact open repository candidate");
  }
  const open = pulls.filter((pull) => validPull(pull, repository));
  const byHead = new Map();
  for (const pull of open) {
    const prior = byHead.get(pull.head.ref);
    if (prior) return independent(current, "ambiguous-head-ref");
    byHead.set(pull.head.ref, pull);
  }

  const owner = current.user.login;
  let root = current;
  const ancestors = [];
  const seen = new Set([current.number]);
  while (byHead.has(root.base.ref)) {
    const parent = byHead.get(root.base.ref);
    if (parent.user.login !== owner || seen.has(parent.number)) {
      return independent(current, parent.user.login === owner ? "ancestry-cycle" : "multiple-owners");
    }
    ancestors.unshift(parent);
    seen.add(parent.number);
    root = parent;
  }

  const descendants = [];
  let tip = current;
  while (true) {
    const children = open.filter((pull) => pull.base.ref === tip.head.ref);
    if (children.length === 0) break;
    if (children.length !== 1) return independent(current, "conflicting-siblings");
    const child = children[0];
    if (child.user.login !== owner || seen.has(child.number)) {
      return independent(current, child.user.login === owner ? "ancestry-cycle" : "multiple-owners");
    }
    descendants.push(child);
    seen.add(child.number);
    tip = child;
  }

  const chain = [...ancestors, current, ...descendants];
  return {
    role: current.number === tip.number ? "tip" : "intermediate",
    reason: current.number === tip.number ? "immutable-cumulative-tip" : "covered-by-cumulative-tip",
    owner,
    root_pr: chain[0].number,
    tip_pr: tip.number,
    tip_sha: tip.head.sha,
    pull_numbers: chain.map((pull) => pull.number),
    members: chain.map((pull) => ({ number: pull.number, head_ref: pull.head.ref, head_sha: pull.head.sha })),
    candidate_sha: current.head.sha,
  };
}

function independent(current, reason) {
  return {
    role: "independent",
    reason,
    owner: current.user.login,
    root_pr: current.number,
    tip_pr: current.number,
    tip_sha: current.head.sha,
    pull_numbers: [current.number],
    members: [{ number: current.number, head_ref: current.head.ref, head_sha: current.head.sha }],
    candidate_sha: current.head.sha,
  };
}

export async function resolvePullStack({ repository, pullNumber, candidateSha, token, request = fetch }) {
  if (!token) throw new Error("GitHub token is required");
  const pulls = [];
  const headers = {
    Accept: "application/vnd.github+json",
    Authorization: `Bearer ${token}`,
    "X-GitHub-Api-Version": "2022-11-28",
  };
  for (let page = 1; page <= 10; page += 1) {
    const response = await request(`https://api.github.com/repos/${repository}/pulls?state=open&per_page=100&page=${page}`, { headers });
    if (!response.ok) throw new Error(`list open pull requests failed: HTTP ${response.status}`);
    const body = await response.json();
    if (!Array.isArray(body)) throw new Error("open pull request response is malformed");
    pulls.push(...body);
    if (body.length < 100) break;
  }
  return classifyPullStack(pulls, pullNumber, candidateSha, repository);
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const result = await resolvePullStack({
    repository: process.env.GITHUB_REPOSITORY ?? "",
    pullNumber: Number(process.env.CONDUIT_PR_NUMBER),
    candidateSha: process.env.CONDUIT_CANDIDATE_SHA ?? "",
    token: process.env.GH_TOKEN ?? "",
  });
  const document = { schema: "conduit.ci.pr-stack-role/v1", ...result };
  if (process.env.CONDUIT_STACK_ROLE_OUT) {
    await writeFile(process.env.CONDUIT_STACK_ROLE_OUT, `${JSON.stringify(document, null, 2)}\n`);
  }
  if (process.env.GITHUB_OUTPUT) {
    await appendFile(process.env.GITHUB_OUTPUT, [
      `role=${result.role}`,
      `reason=${result.reason}`,
      `root_pr=${result.root_pr}`,
      `tip_pr=${result.tip_pr}`,
      `tip_sha=${result.tip_sha}`,
      `pull_numbers=${JSON.stringify(result.pull_numbers)}`,
      "",
    ].join("\n"));
  }
  if (process.env.GITHUB_STEP_SUMMARY) {
    await appendFile(process.env.GITHUB_STEP_SUMMARY,
      `## PR stack role\n\n- Candidate: \`${result.candidate_sha}\`\n- Role: \`${result.role}\`\n- Chain: \`${result.pull_numbers.map((number) => `#${number}`).join(" -> ")}\`\n- Expensive proof tip: \`#${result.tip_pr}\` at \`${result.tip_sha}\`\n- Reason: ${result.reason}\n`);
  }
  process.stdout.write(`${JSON.stringify(document, null, 2)}\n`);
}
