import assert from "node:assert/strict";
import test from "node:test";

import {
  ancestorStackCandidateRuns,
  duplicateCurrentCandidateRuns,
  retireSupersededCandidates,
  supersededCandidateRuns,
} from "../../scripts/ci/retire-superseded-candidates.mjs";

const current = "b".repeat(40);
const old = "a".repeat(40);
const headRef = "codex/fixture-head";

const currentPull = () => ({ ok: true, json: async () => ({ head: { sha: current, ref: headRef } }) });

test("retires only another head of the same pull lifecycle", () => {
  const runs = [
    { id: 1, event: "pull_request", name: "check", status: "in_progress", head_sha: old, pull_requests: [{ number: 7 }] },
    { id: 2, event: "pull_request", name: "check", status: "queued", head_sha: current, pull_requests: [{ number: 7 }] },
    { id: 3, event: "pull_request", name: "check", status: "in_progress", head_sha: old, pull_requests: [{ number: 8 }] },
    { id: 4, event: "pull_request_target", name: "tour-and-creche-pages", status: "in_progress", head_sha: old, pull_requests: [{ number: 7 }] },
    { id: 5, event: "pull_request", name: "unknown", status: "in_progress", head_sha: old, pull_requests: [{ number: 7 }] },
    { id: 6, event: "pull_request", name: "check", status: "completed", head_sha: old, pull_requests: [{ number: 7 }] },
  ];
  assert.deepEqual(supersededCandidateRuns(runs, 7, current).map(({ id }) => id), [1]);
});

test("duplicate current-head events retain the established run and retire later copies", () => {
  const duplicates = [1, 2].map((id) => ({
    id, event: "pull_request", name: "check", status: "in_progress", head_sha: current, pull_requests: [{ number: 7 }],
  }));
  assert.deepEqual(supersededCandidateRuns(duplicates, 7, current), []);
  assert.deepEqual(duplicateCurrentCandidateRuns(duplicates, 7, current).map(({ id }) => id), [2]);
});

test("a cumulative tip retires active expensive runs for exact ancestor candidates only", () => {
  const members = [
    { number: 5, head_ref: "victor/one", head_sha: "1".repeat(40) },
    { number: 6, head_ref: "victor/two", head_sha: "2".repeat(40) },
    { number: 7, head_ref: "victor/tip", head_sha: current },
  ];
  const runs = [
    { id: 1, event: "pull_request", name: "check", status: "in_progress", head_sha: members[0].head_sha, pull_requests: [{ number: 5 }] },
    { id: 2, event: "pull_request", name: "tour-and-creche-products", status: "queued", head_sha: members[1].head_sha, pull_requests: [] },
    { id: 3, event: "pull_request", name: "check", status: "queued", head_sha: current, pull_requests: [{ number: 7 }] },
    { id: 4, event: "pull_request", name: "check", status: "completed", head_sha: members[0].head_sha, pull_requests: [{ number: 5 }] },
    { id: 5, event: "pull_request", name: "check", status: "queued", head_sha: members[0].head_sha, pull_requests: [{ number: 99 }] },
  ];
  assert.deepEqual(ancestorStackCandidateRuns(runs, members, 7).map(({ id }) => id), [1, 2]);
});

test("the unified candidate controller is retired by immutable head identity", () => {
  const runs = [
    { id: 1, event: "pull_request", name: "candidate", status: "in_progress", head_sha: old, pull_requests: [{ number: 7 }] },
    { id: 2, event: "pull_request", name: "candidate", status: "in_progress", head_sha: current, pull_requests: [{ number: 7 }] },
  ];
  assert.deepEqual(supersededCandidateRuns(runs, 7, current).map(({ id }) => id), [1]);
});

test("historical success never retires the sole active aggregate-gate run", () => {
  const runs = [
    { id: 1, event: "pull_request", name: "check", status: "completed", conclusion: "success", head_sha: current, pull_requests: [{ number: 7 }] },
    { id: 2, event: "pull_request", name: "check", status: "queued", head_sha: current, pull_requests: [{ number: 7 }] },
    { id: 3, event: "pull_request", name: "book-pr-proof", status: "queued", head_sha: current, pull_requests: [{ number: 7 }] },
  ];
  assert.deepEqual(duplicateCurrentCandidateRuns(runs, 7, current), []);
});

test("historical success still deduplicates multiple active runs to one survivor", () => {
  const runs = [
    { id: 1, event: "pull_request", name: "check", status: "completed", conclusion: "success", head_sha: current, pull_requests: [{ number: 7 }] },
    { id: 2, event: "pull_request", name: "check", status: "in_progress", head_sha: current, pull_requests: [{ number: 7 }] },
    { id: 3, event: "pull_request", name: "check", status: "queued", head_sha: current, pull_requests: [{ number: 7 }] },
  ];
  assert.deepEqual(duplicateCurrentCandidateRuns(runs, 7, current).map(({ id }) => id), [3]);
});

test("cancels each exact superseded run and reports immutable provenance", async () => {
  const requests = [];
  const request = async (url, options = {}) => {
    requests.push([url, options.method ?? "GET"]);
    if (url.endsWith("/pulls/7")) return currentPull();
    if ((options.method ?? "GET") === "GET" && url.includes("actions/runs?")) {
      return { ok: true, json: async () => ({ workflow_runs: [
        { id: 41, event: "pull_request", name: "check", status: "queued", head_sha: old, pull_requests: [{ number: 7 }] },
        { id: 42, event: "pull_request", name: "check", status: "queued", head_sha: old, pull_requests: [{ number: 9 }] },
      ] }) };
    }
    if ((options.method ?? "GET") === "GET") return { ok: true, json: async () => ({ status: "completed" }) };
    return { ok: true, status: 202 };
  };
  const retired = await retireSupersededCandidates({ repository: "dancxjo/conduit", pullNumber: 7, currentHead: current, headRef, token: "test", request, pause: async () => {} });
  assert.deepEqual(retired, { eventStatus: "current", retired: [{ id: 41, name: "check", head_sha: old, retirement_reason: "superseded-head" }] });
  assert.match(requests.find(([url]) => url.includes("actions/runs?"))[0], /branch=codex%2Ffixture-head/);
  assert.equal(requests.filter(([, method]) => method === "POST").length, 1);
  assert.match(requests.find(([, method]) => method === "POST")[0], /\/runs\/41\/cancel$/);
});

test("force-cancels only the same superseded run when normal cancellation lingers", async () => {
  const posts = [];
  const request = async (url, options = {}) => {
    if ((options.method ?? "GET") === "POST") {
      posts.push(url);
      return { ok: true, status: 202 };
    }
    if (url.endsWith("/pulls/7")) return currentPull();
    if (url.includes("actions/runs?")) return { ok: true, json: async () => ({ workflow_runs: [
      { id: 51, event: "pull_request", name: "check", status: "in_progress", head_sha: old, pull_requests: [{ number: 7 }] },
    ] }) };
    return { ok: true, json: async () => ({ status: "in_progress" }) };
  };
  await retireSupersededCandidates({ repository: "dancxjo/conduit", pullNumber: 7, currentHead: current, headRef, token: "test", request, pause: async () => {} });
  assert.deepEqual(posts.map((url) => url.split("/").at(-1)), ["cancel", "force-cancel"]);
  assert.ok(posts.every((url) => url.includes("/runs/51/")));
});

test("resolves omitted run associations from immutable head history", async () => {
  const posts = [];
  const request = async (url, options = {}) => {
    if ((options.method ?? "GET") === "POST") {
      posts.push(url);
      return { ok: true, status: 202 };
    }
    if (url.endsWith("/pulls/7")) return currentPull();
    if (url.includes("actions/runs?")) return { ok: true, json: async () => ({ workflow_runs: [
      { id: 61, event: "pull_request", name: "check", status: "queued", head_sha: old, pull_requests: [] },
    ] }) };
    if (url.includes(`/commits/${old}/pulls`)) return { ok: true, json: async () => [{ number: 7 }] };
    return { ok: true, json: async () => ({ status: "completed" }) };
  };
  const retired = await retireSupersededCandidates({ repository: "dancxjo/conduit", pullNumber: 7, currentHead: current, headRef, token: "test", request, pause: async () => {} });
  assert.deepEqual(retired.retired.map(({ id }) => id), [61]);
  assert.equal(posts.length, 1);
});

test("other pull associations and API failures fail closed", async () => {
  assert.deepEqual(supersededCandidateRuns([
    { id: 1, event: "pull_request", name: "check", status: "queued", head_sha: old, pull_requests: [] },
  ], 7, current), []);
  await assert.rejects(() => retireSupersededCandidates({
    repository: "dancxjo/conduit", pullNumber: 7, currentHead: current, headRef, token: "test",
    request: async () => ({ ok: false, status: 403 }),
  }), /HTTP 403/);
  await assert.rejects(() => retireSupersededCandidates({
    repository: "dancxjo/conduit", pullNumber: 7, currentHead: current, headRef: "bad\nref", token: "test",
    request: async () => { throw new Error("must not request"); },
  }), /bounded branch identity/);
});

test("a stale retirement event cannot cancel the newer candidate", async () => {
  const requests = [];
  const request = async (url, options = {}) => {
    requests.push([url, options.method ?? "GET"]);
    if (url.endsWith("/pulls/7")) {
      return { ok: true, json: async () => ({ head: { sha: current, ref: headRef } }) };
    }
    throw new Error("stale event must stop before listing or canceling runs");
  };
  const result = await retireSupersededCandidates({
    repository: "dancxjo/conduit",
    pullNumber: 7,
    currentHead: old,
    headRef,
    token: "test",
    request,
  });
  assert.deepEqual(result, { eventStatus: "stale", retired: [] });
  assert.deepEqual(requests, [["https://api.github.com/repos/dancxjo/conduit/pulls/7", "GET"]]);
});
