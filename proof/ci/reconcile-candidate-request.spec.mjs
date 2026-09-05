import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { exactLaneDisposition, laneReceiptRunLocator, publishCandidateResults, resolveCandidateRequest, successfulAdmissionEvidence, successfulLaneEvidence } from "../../scripts/ci/reconcile-candidate-request.mjs";

const candidate = "b".repeat(40);
const base = "a".repeat(40);
const integration = "d".repeat(40);
const repository = "dancxjo/conduit";
const runUrl = (run, job = run) => `https://github.com/dancxjo/conduit/actions/runs/${run}/job/${job}`;
const response = (body, status = 200) => ({ ok: status >= 200 && status < 300, status, json: async () => body });
const pull = (head = candidate) => ({ state: "open", mergeable: true, merge_commit_sha: integration, head: { sha: head }, base: { sha: base, repo: { full_name: repository } } });

test("an authoritative empty workspace plan skips its matrix without weakening classifier failure", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/check.yml", import.meta.url), "utf8");
  assert.match(workflow, /needs\.classify\.outputs\.workspace_matrix != '\[\]'/);
  assert.match(workflow, /WORKSPACE_MATRIX: \$\{\{ needs\.classify\.outputs\.workspace_matrix \}\}/);
  assert.match(workflow, /if test "\$WORKSPACE_MATRIX" = '\[\]'; then require_result workspace "\$WORKSPACE_RESULT" skipped/);
  assert.match(workflow, /require_result classify "\$CLASSIFY_RESULT" success/);
});

test("inherits only exact successful GitHub Actions aggregate evidence", () => {
  const selected = successfulLaneEvidence([
    { id: 1, name: "check", status: "completed", conclusion: "failure", app: { slug: "github-actions" } },
    { id: 2, name: "check", status: "completed", conclusion: "success", app: { slug: "another-app" } },
    { id: 3, name: "check", status: "completed", conclusion: "success", details_url: null, app: { slug: "github-actions" } },
    { id: 4, name: "check", status: "completed", conclusion: "success", details_url: "https://example.test/4", app: { slug: "github-actions" } },
  ], "check");
  assert.equal(selected.id, 4);
});

test("locates aggregate receipts inside the single candidate controller run", () => {
  const checks = [
    { id: 10, name: "check / check", status: "completed", conclusion: "success", details_url: runUrl(700), app: { slug: "github-actions" } },
    { id: 11, name: "products / products-proof", status: "completed", conclusion: "success", details_url: runUrl(700), app: { slug: "github-actions" } },
  ];
  assert.equal(laneReceiptRunLocator(checks, "check").id, 10);
  assert.equal(laneReceiptRunLocator(checks, "products-proof").id, 11);
});

test("a failed aggregate run may locate successful child receipts but is not evidence", () => {
  const checks = [{
    id: 45, name: "products-proof", status: "completed", conclusion: "failure",
    details_url: "https://github.com/dancxjo/conduit/actions/runs/123/job/456",
    app: { slug: "github-actions" },
  }];
  assert.equal(successfulLaneEvidence(checks, "products-proof"), null);
  assert.equal(laneReceiptRunLocator(checks, "products-proof").id, 45);
});

test("exact proof receipts, not an aggregate name, determine lane inheritance", () => {
  const plan = {
    schema: "conduit.ci.reconciliation-plan/v1",
    integration_status: "clean",
    proofs: [
      { proof_id: "workspace.products", kind: "workspace", disposition: "inherited" },
      { proof_id: "machine.esp32-c3", kind: "machine", disposition: "execute" },
      { proof_id: "browser.tour", kind: "browser", disposition: "inherited" },
    ],
  };
  assert.deepEqual(exactLaneDisposition(plan, "check"), {
    inherited: false,
    inheritedProofIds: ["workspace.products"],
    executeProofIds: ["machine.esp32-c3"],
  });
  assert.deepEqual(exactLaneDisposition(plan, "products-proof"), {
    inherited: true,
    inheritedProofIds: ["browser.tour"],
    executeProofIds: [],
  });
});

test("exact lane reconciliation fails closed for absent and malformed proof evidence", () => {
  assert.throws(() => exactLaneDisposition({ schema: "unknown", integration_status: "clean", proofs: [] }, "check"), /unknown schema/);
  assert.throws(() => exactLaneDisposition({ schema: "conduit.ci.reconciliation-plan/v1", integration_status: "conflict", proofs: [] }, "check"), /incomplete/);
  assert.throws(() => exactLaneDisposition({
    schema: "conduit.ci.reconciliation-plan/v1", integration_status: "clean",
    proofs: [{ proof_id: "workspace.products", kind: "workspace", disposition: "green-enough" }],
  }, "check"), /malformed/);
  assert.deepEqual(exactLaneDisposition({
    schema: "conduit.ci.reconciliation-plan/v1", integration_status: "clean", proofs: [],
  }, "check"), { inherited: true, inheritedProofIds: [], executeProofIds: [] });
});

test("a green aggregate is only a receipt locator across related integration movement", async () => {
  const result = await resolveCandidateRequest({
    repository, pullNumber: 7, candidateSha: candidate, baseSha: base,
    integrationSha: integration, token: "test",
    request: async (url) => url.endsWith("/pulls/7") ? response(pull()) : response({ check_runs: [
      { id: 91, name: "check", status: "completed", conclusion: "success", details_url: runUrl(901), app: { slug: "github-actions" } },
      { id: 92, name: "products-proof", status: "completed", conclusion: "success", details_url: runUrl(902), app: { slug: "github-actions" } },
    ] }),
  });
  assert.deepEqual(result.lanes.check, { checkRunId: 91, detailsUrl: runUrl(901), workflowRunId: 901 });
  assert.ok(result.lanes["products-proof"]);
});

test("same candidate inherits successful checks after the base moves", async () => {
  const result = await resolveCandidateRequest({ repository, pullNumber: 2426, candidateSha: candidate, baseSha: base, integrationSha: integration, token: "test", request: async (url) => url.endsWith("/pulls/2426") ? response(pull()) : response({ check_runs: [
    { id: 7, name: "check", status: "completed", conclusion: "success", details_url: runUrl(701), app: { slug: "github-actions" } },
    { id: 8, name: "products-proof", status: "completed", conclusion: "failure", details_url: runUrl(801), app: { slug: "github-actions" } },
  ] }) });
  assert.equal(result.baseSha, base);
  assert.equal(result.integrationSha, integration);
  assert.deepEqual(result.lanes.check, { checkRunId: 7, detailsUrl: runUrl(701), workflowRunId: 701 });
  assert.deepEqual(result.lanes["products-proof"], { checkRunId: 8, detailsUrl: runUrl(801), workflowRunId: 801 });
});

test("aggregate checks expose only their workflow run as a receipt locator", async () => {
  const result = await resolveCandidateRequest({
    repository, pullNumber: 7, candidateSha: candidate, baseSha: base,
    integrationSha: integration, token: "test",
    request: async (url) => url.endsWith("/pulls/7") ? response(pull()) : response({ check_runs: [{
      id: 77, name: "check", status: "completed", conclusion: "success",
      details_url: "https://github.com/dancxjo/conduit/actions/runs/123456789/job/987654321",
      app: { slug: "github-actions" },
    }] }),
  });
  assert.equal(result.lanes.check.workflowRunId, 123456789);
});

test("stale candidate and another target repository fail before evidence lookup", async () => {
  let requests = 0;
  await assert.rejects(() => resolveCandidateRequest({ repository, pullNumber: 7, candidateSha: candidate, baseSha: base, integrationSha: integration, token: "test", request: async () => { requests += 1; return response(pull("c".repeat(40))); } }), /not the current head/);
  assert.equal(requests, 1);
  await assert.rejects(() => resolveCandidateRequest({ repository, pullNumber: 7, candidateSha: candidate, baseSha: base, integrationSha: integration, token: "test", request: async () => response({ ...pull(), base: { sha: base, repo: { full_name: "other/repository" } } }) }), /target repository/);
});

test("refuses admission without publishing a sticky failed gate", async () => {
  const bodies = [];
  await assert.rejects(() => publishCandidateResults({ repository, candidateSha: candidate,
    baseSha: base, integrationSha: integration,
    checkInherited: true, checkResult: "skipped", checkEvidenceUrl: "https://example.test/check",
    productsInherited: false, productsResult: "failure", productsEvidenceUrl: "",
    runUrl: "https://example.test/reconcile", token: "test",
    request: async (_url, options) => { bodies.push(JSON.parse(options.body)); return response({ id: bodies.length + 100 }, 201); },
  }), /refuse admission.*products-proof=failure/);
  assert.equal(bodies.length, 0);
});

test("a published successful reconciliation is reusable by an identical later request", async () => {
  const exact = `conduit.current-controller-reconciliation/v3:${candidate}:${base}:${integration}`;
  const result = await resolveCandidateRequest({ repository, pullNumber: 7, candidateSha: candidate, baseSha: base, integrationSha: integration, token: "test", request: async (url) => url.endsWith("/pulls/7") ? response(pull()) : response({ check_runs: [
    { id: 93, name: "admission-evidence", status: "completed", conclusion: "success", external_id: exact, details_url: "https://example.test/reconcile", app: { slug: "github-actions" } },
  ] }) });
  assert.deepEqual(result.lanes.check, { checkRunId: 93, detailsUrl: "https://example.test/reconcile", workflowRunId: null });
  assert.deepEqual(result.lanes["products-proof"], result.lanes.check);
  assert.equal(result.published, true);
});

test("admission reuse is fail-closed for another integration or contract", () => {
  const checks = [{
    id: 93, name: "admission-evidence", status: "completed", conclusion: "success",
    external_id: `conduit.current-controller-reconciliation/v3:${candidate}:${base}:${integration}`,
    details_url: "https://example.test/reconcile", app: { slug: "github-actions" },
  }];
  assert.equal(successfulAdmissionEvidence(checks, candidate, base, "e".repeat(40)), null);
  assert.equal(successfulAdmissionEvidence([
    { ...checks[0], external_id: `conduit.current-controller-reconciliation/v2:${candidate}:${base}:${integration}` },
  ], candidate, base, integration), null);
});

test("all-inherited evidence still publishes through the stable admission job", async () => {
  const posts = [];
  const result = await resolveCandidateRequest({
    repository, pullNumber: 7, candidateSha: candidate, baseSha: base, integrationSha: integration,
    runUrl: "https://example.test/reconcile", token: "test",
    request: async (url, options = {}) => {
      if (options.method === "POST") {
        posts.push(JSON.parse(options.body));
        return response({ id: 200 + posts.length }, 201);
      }
      if (url.endsWith("/pulls/7")) return response(pull());
      return response({ check_runs: [
        { id: 91, name: "check", status: "completed", conclusion: "success", details_url: runUrl(901), app: { slug: "github-actions" } },
        { id: 92, name: "products-proof", status: "completed", conclusion: "success", details_url: runUrl(902), app: { slug: "github-actions" } },
      ] });
    },
  });
  assert.equal(posts.length, 0);
  assert.ok(result.lanes.check);
  assert.ok(result.lanes["products-proof"]);
  const published = await publishCandidateResults({
    repository, candidateSha: candidate, baseSha: base, integrationSha: integration,
    checkInherited: true, checkResult: "skipped", checkEvidenceUrl: result.lanes.check.detailsUrl,
    productsInherited: true, productsResult: "skipped", productsEvidenceUrl: result.lanes["products-proof"].detailsUrl,
    runUrl: "https://example.test/reconcile", token: "test",
    request: async (_url, options) => {
      posts.push(JSON.parse(options.body));
      return response({ id: 201 }, 201);
    },
  });
  assert.equal(published[0].name, "admission-evidence");
  assert.deepEqual(posts.map(({ name }) => name), ["admission-evidence"]);
  assert.ok(posts.every(({ conclusion }) => conclusion === "success"));
  assert.match(posts[0].output.summary, new RegExp(`Base: ${base}`));
  assert.match(posts[0].output.summary, new RegExp(`Prospective integration: ${integration}`));
});

test("trusted dispatch publishes the exact required PR-head gate", async () => {
  const posts = [];
  const published = await publishCandidateResults({
    repository, candidateSha: candidate, baseSha: base, integrationSha: integration,
    checkInherited: true, checkResult: "skipped", checkEvidenceUrl: "https://example.test/check",
    productsInherited: true, productsResult: "skipped", productsEvidenceUrl: "https://example.test/products",
    runUrl: "https://example.test/reconcile", publishRequiredAdmission: true, token: "test",
    request: async (_url, options) => {
      posts.push(JSON.parse(options.body));
      return response({ id: 300 + posts.length }, 201);
    },
  });
  assert.deepEqual(published.map(({ name }) => name), ["admission-evidence", "admission"]);
  assert.equal(posts[1].head_sha, candidate);
  assert.equal(posts[1].external_id, `conduit.required-admission/v1:${candidate}:${base}:${integration}`);
  assert.equal(posts[1].conclusion, "success");
});

test("partial evidence remains fail-closed for execute then report", async () => {
  let posts = 0;
  const result = await resolveCandidateRequest({
    repository, pullNumber: 7, candidateSha: candidate, baseSha: base, integrationSha: integration,
    runUrl: "https://example.test/reconcile", token: "test",
    request: async (url, options = {}) => {
      if (options.method === "POST") {
        posts += 1;
        return response({ id: 201 }, 201);
      }
      if (url.endsWith("/pulls/7")) return response(pull());
      return response({ check_runs: [
        { id: 91, name: "check", status: "completed", conclusion: "success", details_url: runUrl(901), app: { slug: "github-actions" } },
      ] });
    },
  });
  assert.equal(posts, 0);
  assert.ok(result.lanes.check);
  assert.equal(result.lanes["products-proof"], null);
});
