import assert from "node:assert/strict";
import test from "node:test";

import { retireMergedPullBranch } from "../../scripts/ci/retire-merged-pr-branch.mjs";

const repository = "dancxjo/conduit";
const parentHead = "a".repeat(40);
const childHead = "b".repeat(40);
const pull = {
  merged: true,
  head: { ref: "agent/parent", sha: parentHead, repo: { full_name: repository } },
  base: { ref: "main", repo: { full_name: repository } },
};

test("retargets exact dependent heads, rescans, then deletes the parent branch", async () => {
  const calls = [];
  let listed = 0;
  const result = await retireMergedPullBranch({
    pull,
    repository,
    defaultBranch: "main",
    request: async (url, init) => {
      calls.push([url, init.method, init.body]);
      if (url.includes("/pulls?") && listed++ === 0) return response([{ number: 42, head: { sha: childHead } }]);
      if (url.includes("/pulls?")) return response([]);
      if (url.endsWith("/pulls/42")) return response({ base: { ref: "main" }, head: { sha: childHead } });
      if (url.endsWith("/actions/workflows/reconcile-candidate.yml/dispatches")) return response(undefined, 204);
      if (url.endsWith("/git/refs/heads/agent/parent")) return response(undefined, 204);
      throw new Error(`unexpected ${init.method} ${url}`);
    },
  });
  assert.deepEqual(result.retargeted, [{ number: 42, head_sha: childHead }]);
  assert.deepEqual(calls.map((call) => call[1]), ["GET", "PATCH", "POST", "GET", "DELETE"]);
  assert.equal(JSON.parse(calls[1][2]).base, "main");
  assert.deepEqual(JSON.parse(calls[2][2]), {
    ref: "main",
    inputs: { pr_number: "42", candidate_sha: childHead },
  });
});

test("a failed or identity-changing retarget retains the parent branch", async () => {
  const methods = [];
  await assert.rejects(() => retireMergedPullBranch({
    pull,
    repository,
    defaultBranch: "main",
    request: async (url, init) => {
      methods.push(init.method);
      if (url.includes("/pulls?")) return response([{ number: 42, head: { sha: childHead } }]);
      return response({ base: { ref: "main" }, head: { sha: "c".repeat(40) } });
    },
  }), /changed candidate identity/);
  assert.deepEqual(methods, ["GET", "PATCH"]);
});

test("a refused reconciliation dispatch retains the parent branch", async () => {
  const methods = [];
  await assert.rejects(() => retireMergedPullBranch({
    pull,
    repository,
    defaultBranch: "main",
    request: async (url, init) => {
      methods.push(init.method);
      if (url.includes("/pulls?")) return response([{ number: 42, head: { sha: childHead } }]);
      if (url.endsWith("/pulls/42")) return response({ base: { ref: "main" }, head: { sha: childHead } });
      if (url.endsWith("/actions/workflows/reconcile-candidate.yml/dispatches")) return response(undefined, 403);
      throw new Error(`unexpected ${init.method} ${url}`);
    },
  }), /dispatch reconciliation.*HTTP 403/);
  assert.deepEqual(methods, ["GET", "PATCH", "POST"]);
});

test("unmerged and fork branches are retained without mutation", async () => {
  const request = async () => { throw new Error("must not call GitHub"); };
  assert.equal((await retireMergedPullBranch({ pull: { ...pull, merged: false }, repository, defaultBranch: "main", request })).reason, "pull-not-merged");
  const fork = { ...pull, head: { ...pull.head, repo: { full_name: "elsewhere/fork" } } };
  assert.equal((await retireMergedPullBranch({ pull: fork, repository, defaultBranch: "main", request })).reason, "non-repository-branch");
});

function response(body, status = 200) {
  return { ok: status >= 200 && status < 300, status, json: async () => body };
}
