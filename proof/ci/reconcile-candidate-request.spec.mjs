import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { laneInputsUnchanged, publishCandidateResults, resolveCandidateRequest, successfulAdmissionEvidence, successfulLaneEvidence } from "../../scripts/ci/reconcile-candidate-request.mjs";

const candidate = "b".repeat(40);
const base = "a".repeat(40);
const integration = "d".repeat(40);
const repository = "dancxjo/conduit";
const response = (body, status = 200) => ({ ok: status >= 200 && status < 300, status, json: async () => body });
const pull = (head = candidate) => ({ state: "open", mergeable: true, merge_commit_sha: integration, head: { sha: head }, base: { sha: base, repo: { full_name: repository } } });
const unchangedPlan = {
  pages_products_required: false, full_fallback: false,
  ci_controller_proofs: [], repository_command_proofs: [],
  esp32_required: false, browser_required: false, conduitos_required: false,
  conduitos_aarch64_product_required: false,
  workspace_shards: { lint: false, "test-products": false },
};

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

test("aggregate evidence is reusable only when its actual lane inputs are unchanged", () => {
  assert.equal(laneInputsUnchanged(unchangedPlan, "check"), true);
  assert.equal(laneInputsUnchanged(unchangedPlan, "products-proof"), true);
  assert.equal(laneInputsUnchanged({ ...unchangedPlan, workspace_shards: { lint: true } }, "check"), false);
  assert.equal(laneInputsUnchanged({ ...unchangedPlan, browser_required: true }, "check"), false);
  assert.equal(laneInputsUnchanged({ ...unchangedPlan, pages_products_required: true }, "products-proof"), false);
  assert.equal(laneInputsUnchanged({ ...unchangedPlan, full_fallback: true }, "products-proof"), false);
  assert.throws(() => laneInputsUnchanged({ ...unchangedPlan, workspace_shards: { lint: "false" } }, "check"), /malformed/);
});

test("a green aggregate cannot inherit across related integration movement", async () => {
  const relatedPlan = { ...unchangedPlan, workspace_shards: { lint: false, "test-products": true } };
  const result = await resolveCandidateRequest({
    repository, pullNumber: 7, candidateSha: candidate, baseSha: base,
    integrationSha: integration, reconciliationPlan: relatedPlan, token: "test",
    request: async (url) => url.endsWith("/pulls/7") ? response(pull()) : response({ check_runs: [
      { id: 91, name: "check", status: "completed", conclusion: "success", details_url: "https://example.test/check", app: { slug: "github-actions" } },
      { id: 92, name: "products-proof", status: "completed", conclusion: "success", details_url: "https://example.test/products", app: { slug: "github-actions" } },
    ] }),
  });
  assert.equal(result.lanes.check, null);
  assert.ok(result.lanes["products-proof"]);
  assert.deepEqual(result.laneInputsUnchanged, { check: false, "products-proof": true });
});

test("same candidate inherits successful checks after the base moves", async () => {
  const result = await resolveCandidateRequest({ repository, pullNumber: 2426, candidateSha: candidate, baseSha: base, integrationSha: integration, reconciliationPlan: unchangedPlan, token: "test", request: async (url) => url.endsWith("/pulls/2426") ? response(pull()) : response({ check_runs: [
    { id: 7, name: "check", status: "completed", conclusion: "success", details_url: "https://example.test/7", app: { slug: "github-actions" } },
    { id: 8, name: "products-proof", status: "completed", conclusion: "failure", details_url: "https://example.test/8", app: { slug: "github-actions" } },
  ] }) });
  assert.equal(result.baseSha, base);
  assert.equal(result.integrationSha, integration);
  assert.deepEqual(result.lanes.check, { checkRunId: 7, detailsUrl: "https://example.test/7" });
  assert.equal(result.lanes["products-proof"], null);
});

test("stale candidate and another target repository fail before evidence lookup", async () => {
  let requests = 0;
  await assert.rejects(() => resolveCandidateRequest({ repository, pullNumber: 7, candidateSha: candidate, baseSha: base, integrationSha: integration, reconciliationPlan: unchangedPlan, token: "test", request: async () => { requests += 1; return response(pull("c".repeat(40))); } }), /not the current head/);
  assert.equal(requests, 1);
  await assert.rejects(() => resolveCandidateRequest({ repository, pullNumber: 7, candidateSha: candidate, baseSha: base, integrationSha: integration, reconciliationPlan: unchangedPlan, token: "test", request: async () => response({ ...pull(), base: { sha: base, repo: { full_name: "other/repository" } } }) }), /target repository/);
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
  const relatedPlan = { ...unchangedPlan, full_fallback: true, pages_products_required: true };
  const result = await resolveCandidateRequest({ repository, pullNumber: 7, candidateSha: candidate, baseSha: base, integrationSha: integration, reconciliationPlan: relatedPlan, token: "test", request: async (url) => url.endsWith("/pulls/7") ? response(pull()) : response({ check_runs: [
    { id: 93, name: "admission-evidence", status: "completed", conclusion: "success", external_id: exact, details_url: "https://example.test/reconcile", app: { slug: "github-actions" } },
  ] }) });
  assert.deepEqual(result.lanes.check, { checkRunId: 93, detailsUrl: "https://example.test/reconcile" });
  assert.deepEqual(result.lanes["products-proof"], result.lanes.check);
  assert.deepEqual(result.laneInputsUnchanged, { check: true, "products-proof": true });
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
    repository, pullNumber: 7, candidateSha: candidate, baseSha: base, integrationSha: integration, reconciliationPlan: unchangedPlan,
    runUrl: "https://example.test/reconcile", token: "test",
    request: async (url, options = {}) => {
      if (options.method === "POST") {
        posts.push(JSON.parse(options.body));
        return response({ id: 200 + posts.length }, 201);
      }
      if (url.endsWith("/pulls/7")) return response(pull());
      return response({ check_runs: [
        { id: 91, name: "check", status: "completed", conclusion: "success", details_url: "https://example.test/check", app: { slug: "github-actions" } },
        { id: 92, name: "products-proof", status: "completed", conclusion: "success", details_url: "https://example.test/products", app: { slug: "github-actions" } },
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

test("partial evidence remains fail-closed for execute then report", async () => {
  let posts = 0;
  const result = await resolveCandidateRequest({
    repository, pullNumber: 7, candidateSha: candidate, baseSha: base, integrationSha: integration, reconciliationPlan: unchangedPlan,
    runUrl: "https://example.test/reconcile", token: "test",
    request: async (url, options = {}) => {
      if (options.method === "POST") {
        posts += 1;
        return response({ id: 201 }, 201);
      }
      if (url.endsWith("/pulls/7")) return response(pull());
      return response({ check_runs: [
        { id: 91, name: "check", status: "completed", conclusion: "success", details_url: "https://example.test/check", app: { slug: "github-actions" } },
      ] });
    },
  });
  assert.equal(posts, 0);
  assert.ok(result.lanes.check);
  assert.equal(result.lanes["products-proof"], null);
});
