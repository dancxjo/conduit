import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { publishCandidateResults, resolveAndPublishInherited, resolveCandidateRequest, successfulLaneEvidence } from "../../scripts/ci/reconcile-candidate-request.mjs";

const candidate = "b".repeat(40);
const base = "a".repeat(40);
const repository = "dancxjo/conduit";
const response = (body, status = 200) => ({ ok: status >= 200 && status < 300, status, json: async () => body });
const pull = (head = candidate) => ({ state: "open", head: { sha: head }, base: { sha: base, repo: { full_name: repository } } });

test("an authoritative empty workspace plan skips its matrix without weakening classifier failure", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/check.yml", import.meta.url), "utf8");
  assert.match(workflow, /needs\.classify\.outputs\.workspace_matrix != '\[\]'/);
  assert.match(workflow, /WORKSPACE_MATRIX: \$\{\{ needs\.classify\.outputs\.workspace_matrix \}\}/);
  assert.match(workflow, /if test "\$WORKSPACE_MATRIX" = '\[\]'; then\s+test "\$WORKSPACE_RESULT" = skipped/);
  assert.match(workflow, /test "\$CLASSIFY_RESULT" = success/);
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

test("same candidate inherits successful checks after the base moves", async () => {
  const result = await resolveCandidateRequest({ repository, pullNumber: 2426, candidateSha: candidate, token: "test", request: async (url) => url.endsWith("/pulls/2426") ? response(pull()) : response({ check_runs: [
    { id: 7, name: "check", status: "completed", conclusion: "success", details_url: "https://example.test/7", app: { slug: "github-actions" } },
    { id: 8, name: "products-proof", status: "completed", conclusion: "failure", details_url: "https://example.test/8", app: { slug: "github-actions" } },
  ] }) });
  assert.equal(result.baseSha, base);
  assert.deepEqual(result.lanes.check, { checkRunId: 7, detailsUrl: "https://example.test/7" });
  assert.equal(result.lanes["products-proof"], null);
});

test("stale candidate and another target repository fail before evidence lookup", async () => {
  let requests = 0;
  await assert.rejects(() => resolveCandidateRequest({ repository, pullNumber: 7, candidateSha: candidate, token: "test", request: async () => { requests += 1; return response(pull("c".repeat(40))); } }), /not the current head/);
  assert.equal(requests, 1);
  await assert.rejects(() => resolveCandidateRequest({ repository, pullNumber: 7, candidateSha: candidate, token: "test", request: async () => response({ ...pull(), base: { sha: base, repo: { full_name: "other/repository" } } }) }), /target repository/);
});

test("refuses admission without publishing a sticky failed gate", async () => {
  const bodies = [];
  await assert.rejects(() => publishCandidateResults({ repository, candidateSha: candidate,
    checkInherited: true, checkResult: "skipped", checkEvidenceUrl: "https://example.test/check",
    productsInherited: false, productsResult: "failure", productsEvidenceUrl: "",
    runUrl: "https://example.test/reconcile", token: "test",
    request: async (_url, options) => { bodies.push(JSON.parse(options.body)); return response({ id: bodies.length + 100 }, 201); },
  }), /refuse admission.*products-proof=failure/);
  assert.equal(bodies.length, 0);
});

test("a published successful reconciliation is reusable by an identical later request", async () => {
  const result = await resolveCandidateRequest({ repository, pullNumber: 7, candidateSha: candidate, token: "test", request: async (url) => url.endsWith("/pulls/7") ? response(pull()) : response({ check_runs: [
    { id: 91, name: "check", status: "completed", conclusion: "success", details_url: "https://example.test/reconcile", app: { slug: "github-actions" } },
    { id: 92, name: "products-proof", status: "completed", conclusion: "success", details_url: "https://example.test/reconcile", app: { slug: "github-actions" } },
  ] }) });
  assert.ok(result.lanes.check);
  assert.ok(result.lanes["products-proof"]);
});

test("an all-inherited request publishes admission without allocating a report job", async () => {
  const posts = [];
  const result = await resolveAndPublishInherited({
    repository, pullNumber: 7, candidateSha: candidate,
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
  assert.equal(result.published.length, 1);
  assert.deepEqual(posts.map(({ name }) => name), ["admission"]);
  assert.ok(posts.every(({ conclusion }) => conclusion === "success"));
});

test("partial evidence remains fail-closed for execute then report", async () => {
  let posts = 0;
  const result = await resolveAndPublishInherited({
    repository, pullNumber: 7, candidateSha: candidate,
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
  assert.equal(result.published, null);
  assert.equal(posts, 0);
  assert.ok(result.resolved.lanes.check);
  assert.equal(result.resolved.lanes["products-proof"], null);
});
