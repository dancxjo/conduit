const CANDIDATE_WORKFLOWS = new Set([
  "candidate",
  "check",
  "book-and-creche-products",
  "book-pr-proof",
  "pages-deploy-pr-proof",
  "patchbay-debugger-pr-proof",
]);

const ACTIVE = new Set(["queued", "in_progress", "requested", "waiting", "pending"]);

export function supersededCandidateRuns(runs, pullNumber, currentHead) {
  return runs.filter((run) =>
    run.event === "pull_request"
    && CANDIDATE_WORKFLOWS.has(run.name)
    && ACTIVE.has(run.status)
    && run.head_sha !== currentHead
    && Array.isArray(run.pull_requests)
    && run.pull_requests.some(({ number }) => number === pullNumber));
}

export function duplicateCurrentCandidateRuns(runs, pullNumber, currentHead) {
  const retired = [];
  for (const workflow of CANDIDATE_WORKFLOWS) {
    const matching = runs
      .filter((run) => run.event === "pull_request"
        && run.name === workflow
        && run.head_sha === currentHead
        && Array.isArray(run.pull_requests)
        && run.pull_requests.some(({ number }) => number === pullNumber))
      .sort((left, right) => left.id - right.id);
    const active = matching.filter((run) => ACTIVE.has(run.status));
    // A completed receipt may be reusable, but canceling the only active run
    // replaces GitHub's stable required check with a cancelled conclusion.
    // Keep one current lifecycle run to publish the aggregate gate.
    retired.push(...active.slice(1));
  }
  return retired;
}

export function ancestorStackCandidateRuns(runs, members, tipNumber) {
  const ancestors = new Map(members
    .filter(({ number }) => number !== tipNumber)
    .map((member) => [member.head_sha, member.number]));
  return runs.filter((run) => {
    const pullNumber = ancestors.get(run.head_sha);
    return pullNumber !== undefined
      && run.event === "pull_request"
      && CANDIDATE_WORKFLOWS.has(run.name)
      && ACTIVE.has(run.status)
      && Array.isArray(run.pull_requests)
      && (run.pull_requests.length === 0 || run.pull_requests.some(({ number }) => number === pullNumber));
  });
}

export async function retireSupersededCandidates({ repository, pullNumber, currentHead, headRef, token, request = fetch, pause = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds)), retireStackAncestors = false }) {
  if (!/^[^/]+\/[^/]+$/.test(repository)) throw new Error("repository must be owner/name");
  if (!Number.isSafeInteger(pullNumber) || pullNumber < 1) throw new Error("pull number must be positive");
  if (!/^[0-9a-f]{40,64}$/.test(currentHead)) throw new Error("current head must be an exact Git identity");
  if (typeof headRef !== "string" || headRef.length === 0 || headRef.length > 255 || /[\u0000-\u001f\u007f]/.test(headRef)) {
    throw new Error("head ref must be a bounded branch identity");
  }
  if (!token) throw new Error("GitHub token is required");

  const headers = {
    Accept: "application/vnd.github+json",
    Authorization: `Bearer ${token}`,
    "X-GitHub-Api-Version": "2022-11-28",
  };
  const pullResponse = await request(`https://api.github.com/repos/${repository}/pulls/${pullNumber}`, { headers });
  if (!pullResponse.ok) throw new Error(`resolve current pull head failed: HTTP ${pullResponse.status}`);
  const pull = await pullResponse.json();
  if (pull?.head?.sha !== currentHead || pull?.head?.ref !== headRef) {
    return { eventStatus: "stale", retired: [] };
  }
  const runs = [];
  for (let page = 1; page <= 10; page += 1) {
    const query = new URLSearchParams({
      event: "pull_request",
      branch: headRef,
      per_page: "100",
      page: String(page),
    });
    const response = await request(`https://api.github.com/repos/${repository}/actions/runs?${query}`, { headers });
    if (!response.ok) throw new Error(`list candidate runs failed: HTTP ${response.status}`);
    const body = await response.json();
    if (!Array.isArray(body.workflow_runs)) throw new Error("candidate run response is malformed");
    runs.push(...body.workflow_runs);
    if (body.workflow_runs.length < 100) break;
  }

  const retired = supersededCandidateRuns(runs, pullNumber, currentHead)
    .map((run) => ({ ...run, retirement_reason: "superseded-head" }));
  retired.push(...duplicateCurrentCandidateRuns(runs, pullNumber, currentHead)
    .map((run) => ({ ...run, retirement_reason: "duplicate-current-head" })));
  const unresolved = runs.filter((run) =>
    run.event === "pull_request"
    && CANDIDATE_WORKFLOWS.has(run.name)
    && ACTIVE.has(run.status)
    && run.head_sha !== currentHead
    && Array.isArray(run.pull_requests)
    && run.pull_requests.length === 0);
  for (const run of unresolved) {
    const response = await request(`https://api.github.com/repos/${repository}/commits/${run.head_sha}/pulls`, { headers });
    if (!response.ok) throw new Error(`resolve run ${run.id} pull lifecycle failed: HTTP ${response.status}`);
    const pulls = await response.json();
    if (!Array.isArray(pulls)) throw new Error(`run ${run.id} pull lifecycle response is malformed`);
    if (pulls.some(({ number }) => number === pullNumber)) {
      retired.push({ ...run, retirement_reason: "superseded-head" });
    }
  }
  if (retireStackAncestors) {
    const stack = await resolvePullStack({ repository, pullNumber, candidateSha: currentHead, token, request });
    if (stack.role === "tip" && stack.members.length > 1) {
      const recent = [];
      for (let page = 1; page <= 10; page += 1) {
        const query = new URLSearchParams({ event: "pull_request", per_page: "100", page: String(page) });
        const response = await request(`https://api.github.com/repos/${repository}/actions/runs?${query}`, { headers });
        if (!response.ok) throw new Error(`list stack candidate runs failed: HTTP ${response.status}`);
        const body = await response.json();
        if (!Array.isArray(body.workflow_runs)) throw new Error("stack candidate run response is malformed");
        recent.push(...body.workflow_runs);
        if (body.workflow_runs.length < 100) break;
      }
      retired.push(...ancestorStackCandidateRuns(recent, stack.members, stack.tip_pr)
        .map((run) => ({ ...run, retirement_reason: `covered-by-stack-tip-${stack.tip_pr}` })));
    }
  }
  const uniqueRetired = [...new Map(retired.map((run) => [run.id, run])).values()];
  for (const run of uniqueRetired) {
    const response = await request(`https://api.github.com/repos/${repository}/actions/runs/${run.id}/cancel`, {
      method: "POST",
      headers,
    });
    if (!response.ok && response.status !== 409) {
      throw new Error(`cancel run ${run.id} failed: HTTP ${response.status}`);
    }
  }
  if (uniqueRetired.length > 0) await pause(5_000);
  for (const run of uniqueRetired) {
    const statusResponse = await request(`https://api.github.com/repos/${repository}/actions/runs/${run.id}`, { headers });
    if (!statusResponse.ok) throw new Error(`inspect cancelled run ${run.id} failed: HTTP ${statusResponse.status}`);
    const status = await statusResponse.json();
    if (ACTIVE.has(status.status)) {
      const forced = await request(`https://api.github.com/repos/${repository}/actions/runs/${run.id}/force-cancel`, {
        method: "POST",
        headers,
      });
      if (!forced.ok && forced.status !== 409) {
        throw new Error(`force-cancel run ${run.id} failed: HTTP ${forced.status}`);
      }
    }
  }
  return {
    eventStatus: "current",
    retired: uniqueRetired.map(({ id, name, head_sha, retirement_reason }) => ({ id, name, head_sha, retirement_reason })),
  };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const result = await retireSupersededCandidates({
    repository: process.env.GITHUB_REPOSITORY ?? "",
    pullNumber: Number(process.env.CONDUIT_PR_NUMBER),
    currentHead: process.env.CONDUIT_CANDIDATE_SHA ?? "",
    headRef: process.env.CONDUIT_HEAD_REF ?? "",
    token: process.env.GH_TOKEN ?? "",
    retireStackAncestors: true,
  });
  process.stdout.write(`${JSON.stringify({
    schema: "conduit.ci.superseded-candidates/v1",
    current_head: process.env.CONDUIT_CANDIDATE_SHA,
    event_status: result.eventStatus,
    retired: result.retired,
  }, null, 2)}\n`);
}
import { resolvePullStack } from "./classify-pr-stack.mjs";
