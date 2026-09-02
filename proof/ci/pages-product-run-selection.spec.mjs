import assert from "node:assert/strict";
import test from "node:test";

import { selectExactRun, selectExactSuccessfulRun } from "../../scripts/ci/pages-product-run-selection.mjs";

const head = "ef22d9e1b0f3d4cbc19fb35ac4e330f8dbf2b5dc";

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
