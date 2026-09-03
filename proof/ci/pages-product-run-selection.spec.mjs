import assert from "node:assert/strict";
import test from "node:test";

import { resolveMergedPullSource, selectExactRun, selectExactSuccessfulRun } from "../../scripts/ci/pages-product-run-selection.mjs";

const head = "ef22d9e1b0f3d4cbc19fb35ac4e330f8dbf2b5dc";
const merged = "7dccbe822271ef1ac8a0fd49e7cc37add40a943c";
const headTree = "27a9e0f98339fce62fc8360095b2f1ea0d5a9b92";
const mergedTree = "bd48b1d15eab5989a6b452ff23e0b47cde8c67a7";

test("selects the successful exact-head run when GitHub omits PR associations", () => {
  const admitted = { id: 33662354458, status: "completed", conclusion: "success", head_sha: head, pull_requests: [] };
  assert.equal(selectExactSuccessfulRun([
    { id: 1, status: "completed", conclusion: "failure", head_sha: head },
    { id: 2, status: "completed", conclusion: "success", head_sha: "0".repeat(40) },
    admitted,
  ], head), admitted);
});

test("refuses non-successful, stale, and malformed candidates", () => {
  assert.equal(selectExactSuccessfulRun([{ status: "completed", conclusion: "failure", head_sha: head }], head), undefined);
  assert.equal(selectExactSuccessfulRun([{ status: "completed", conclusion: "success", head_sha: "0".repeat(40) }], head), undefined);
  assert.equal(selectExactSuccessfulRun([], "not-an-identity"), undefined);
});

test("exposes queued and failed exact-head runs without admitting them", () => {
  const queued = { id: 7, status: "queued", conclusion: null, head_sha: head };
  const failed = { id: 8, status: "completed", conclusion: "failure", head_sha: head };
  assert.equal(selectExactRun([queued], head), queued);
  assert.equal(selectExactRun([failed], head), failed);
  assert.equal(selectExactSuccessfulRun([queued], head), undefined);
  assert.equal(selectExactSuccessfulRun([failed], head), undefined);
});

test("keeps an exact head run bound to the requested pull when associations are present", () => {
  const otherPull = { id: 9, status: "completed", conclusion: "success", head_sha: head, pull_requests: [{ number: 2198 }] };
  const requestedPull = { id: 10, status: "completed", conclusion: "success", head_sha: head, pull_requests: [{ number: 2199 }] };
  assert.equal(selectExactSuccessfulRun([otherPull, requestedPull], head, 2199), requestedPull);
  assert.equal(selectExactRun([otherPull], head, 2199), undefined);
});

test("derives carrier provenance from the actual merged commit tree after a concurrent base advance", async () => {
  const requested = [];
  const source = await resolveMergedPullSource({
    merged_at: "2026-09-02T00:00:00Z",
    merge_commit_sha: merged,
    head: { sha: head },
  }, async (commit) => {
    requested.push(commit);
    return { sha: commit, tree: { sha: mergedTree } };
  });
  assert.deepEqual(requested, [merged]);
  assert.deepEqual(source, { mergeCommit: merged, sourceHead: head, sourceTree: mergedTree });
  assert.notEqual(source.sourceTree, headTree);
});

test("refuses an unmerged pull or a merged commit without an exact tree", async () => {
  await assert.rejects(() => resolveMergedPullSource({ merge_commit_sha: merged, head: { sha: head } }, async () => ({ tree: { sha: mergedTree } })), /not an exact merged source/);
  await assert.rejects(() => resolveMergedPullSource({ merged_at: "now", merge_commit_sha: merged, head: { sha: head } }, async () => ({ tree: { sha: headTree.slice(1) } })), /no exact merged tree/);
});
