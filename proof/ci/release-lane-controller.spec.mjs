import assert from "node:assert/strict";
import test from "node:test";
import { decideReleaseLane } from "../../tools/ci/release-lane-controller.mjs";

const minute = 60_000;
const now = Date.parse("2026-09-08T16:00:00Z");

function run(id, status, ageMinutes, options = {}) {
  const createdAt = new Date(now - ageMinutes * minute).toISOString();
  return {
    id,
    headSha: options.headSha ?? `head-${id}`,
    status,
    conclusion: options.conclusion ?? null,
    createdAt,
    updatedAt: options.updatedAt ?? createdAt,
    explicitRepair: options.explicitRepair ?? false,
    jobs: options.jobs ?? (status === "in_progress" ? [{ id: id * 10, name: "check", status: "in_progress", conclusion: null, startedAt: createdAt }] : []),
    url: `https://example.test/runs/${id}`,
  };
}

function candidate(prNumber, attempt, options = {}) {
  return {
    prNumber,
    branch: `release/${options.snapshot ?? String(prNumber).padStart(40, "0")}`,
    headSha: options.headSha ?? attempt?.headSha ?? `head-${prNumber}`,
    createdAt: options.createdAt ?? attempt?.createdAt ?? new Date(now).toISOString(),
    attempts: options.attempts ?? (attempt ? [attempt] : []),
  };
}

function decide(candidates, escalations = []) {
  return decideReleaseLane({ now, candidates, escalations }, {
    noProgressMs: 15 * minute,
    maxElapsedMs: 45 * minute,
  });
}

test("A remains healthy while B queues", () => {
  const a = candidate(1, run(101, "in_progress", 5));
  const b = candidate(2, run(102, "queued", 1));
  const decision = decide([a, b]);
  assert.deepEqual(decision.actions.preserve.map(({ runId, state }) => [runId, state]), [[101, "running-healthy"], [102, "queued"]]);
  assert.deepEqual(decision.actions.cancel, []);
});

test("B is superseded when C queues behind healthy A", () => {
  const decision = decide([
    candidate(1, run(101, "in_progress", 5)),
    candidate(2, run(102, "queued", 2)),
    candidate(3, run(103, "queued", 1)),
  ]);
  assert.equal(decision.actions.preserve[0].runId, 101);
  assert.equal(decision.actions.preserve[1].runId, 103);
  assert.deepEqual(decision.actions.cancel.map(({ runId, state }) => [runId, state]), [[102, "cancelled-superseded"]]);
  assert.deepEqual(decision.actions.close.map(({ prNumber }) => prNumber), [2]);
});

test("passed A finalizes and queued C becomes eligible", () => {
  const decision = decide([
    candidate(1, run(101, "completed", 20, { conclusion: "success", jobs: [] })),
    candidate(3, run(103, "queued", 1)),
  ]);
  assert.equal(decision.actions.finalize[0].runId, 101);
  assert.equal(decision.actions.start.runId, 103);
});

test("decisive A failure cancels its remaining work and releases newest queued candidate", () => {
  const failed = run(101, "in_progress", 5, { jobs: [
    { id: 1001, name: "browser-proof", status: "completed", conclusion: "failure", completedAt: new Date(now - minute).toISOString() },
    { id: 1002, name: "firmware", status: "in_progress", conclusion: null, startedAt: new Date(now - 4 * minute).toISOString() },
  ] });
  const decision = decide([candidate(1, failed), candidate(3, run(103, "queued", 1))]);
  assert.deepEqual(decision.actions.cancel[0].failedJobs, [{ name: "browser-proof", id: 1001, url: null, conclusion: "failure" }]);
  assert.equal(decision.actions.cancel[0].state, "cancelled-bad");
  assert.equal(decision.actions.start.runId, 103);
});

test("a changed repaired head gets a fresh attempt after failure", () => {
  const bad = candidate(1, run(101, "completed", 20, { conclusion: "failure", headSha: "bad-head", jobs: [] }), { headSha: "bad-head" });
  const repaired = candidate(1, run(102, "queued", 1, { headSha: "repaired-head" }), { headSha: "repaired-head" });
  const decision = decide([bad, repaired]);
  assert.equal(decision.classifications.find(({ runId }) => runId === 101).state, "running-bad");
  assert.equal(decision.actions.start.runId, 102);
});

test("healthy running work survives metadata and newer development observations", () => {
  const a = candidate(1, run(101, "in_progress", 5));
  const first = decide([a]);
  const second = decide([{ ...a, metadataRevision: 99, newerDevSha: "new-dev" }]);
  assert.equal(first.actions.preserve[0].runId, 101);
  assert.equal(second.actions.preserve[0].runId, 101);
  assert.deepEqual(second.actions.cancel, []);
});

test("three unstarted candidates collapse to the newest", () => {
  const decision = decide([
    candidate(1, run(101, "queued", 3)),
    candidate(2, run(102, "queued", 2)),
    candidate(3, run(103, "queued", 1)),
  ]);
  assert.equal(decision.actions.start.runId, 103);
  assert.deepEqual(decision.actions.cancel.map(({ runId }) => runId), [101, 102]);
});

test("an approval-gated unstarted candidate remains queued and may be superseded", () => {
  const approvalGated = run(101, "completed", 3, { conclusion: "action_required", jobs: [] });
  const decision = decide([
    candidate(1, approvalGated),
    candidate(2, run(102, "queued", 1)),
  ]);
  assert.equal(decision.classifications[0].state, "queued");
  assert.deepEqual(decision.actions.close.map(({ prNumber }) => prNumber), [1]);
  assert.equal(decision.actions.start.runId, 102);
});

test("stuck A escalates exactly once and retains only newest queued successor", () => {
  const a = candidate(1, run(101, "in_progress", 20, { updatedAt: new Date(now - 20 * minute).toISOString() }));
  const b = candidate(2, run(102, "queued", 2));
  const c = candidate(3, run(103, "queued", 1));
  const first = decide([a, b, c]);
  assert.equal(first.actions.escalate.length, 1);
  assert.equal(first.actions.escalate[0].key, "release-stuck:101");
  assert.deepEqual(first.actions.cancel.map(({ runId }) => runId), [102]);
  const repeated = decide([a, b, c], ["release-stuck:101"]);
  assert.deepEqual(repeated.actions.escalate, []);
});

test("known-bad head cannot restart without explicit repair action", () => {
  const prior = run(101, "completed", 20, { conclusion: "failure", headSha: "same", jobs: [] });
  const restart = run(102, "queued", 1, { headSha: "same" });
  const rejected = decide([candidate(1, restart, { headSha: "same", attempts: [prior, restart] })]);
  assert.equal(rejected.actions.cancel[0].reason, "known-bad-restart");
  const explicit = { ...restart, explicitRepair: true };
  const allowed = decide([candidate(1, explicit, { headSha: "same", attempts: [prior, explicit] })]);
  assert.equal(allowed.actions.start.runId, 102);
});

test("failure decisions preserve exact run head job and URL evidence", () => {
  const failed = run(101, "in_progress", 5, { jobs: [{
    id: 1001,
    name: "products / browser-proof-tour",
    status: "completed",
    conclusion: "failure",
    completedAt: new Date(now - minute).toISOString(),
    url: "https://example.test/jobs/1001",
  }] });
  const action = decide([candidate(7, failed, { headSha: failed.headSha })]).actions.cancel[0];
  assert.deepEqual({ prNumber: action.prNumber, headSha: action.headSha, runId: action.runId, runUrl: action.runUrl }, {
    prNumber: 7, headSha: "head-101", runId: 101, runUrl: "https://example.test/runs/101",
  });
  assert.deepEqual(action.failedJobs[0], {
    name: "products / browser-proof-tour", id: 1001, url: "https://example.test/jobs/1001", conclusion: "failure",
  });
});
