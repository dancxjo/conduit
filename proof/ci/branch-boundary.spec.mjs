import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { test } from "node:test";
import { validateBoundary } from "../../tools/ci/branch-boundary.mjs";

test("feature candidates enter dev through focused admission", () => {
  assert.deepEqual(validateBoundary("candidate", {
    CONDUIT_EVENT_NAME: "pull_request",
    CONDUIT_BASE_REF: "dev",
  }), { mode: "candidate", admission: "focused", target: "dev" });
  assert.throws(() => validateBoundary("candidate", {
    CONDUIT_EVENT_NAME: "pull_request",
    CONDUIT_BASE_REF: "main",
  }), /candidate base must be dev/);
});

test("only an exact same-repository frozen dev snapshot can promote to main", () => {
  const head = "0123456789abcdef0123456789abcdef01234567";
  const valid = {
    CONDUIT_EVENT_NAME: "pull_request",
    CONDUIT_BASE_REF: "main",
    CONDUIT_HEAD_REF: `promote/${head}`,
    CONDUIT_HEAD_SHA: head,
    CONDUIT_HEAD_REPOSITORY: "dancxjo/conduit",
    CONDUIT_REPOSITORY: "dancxjo/conduit",
  };
  assert.equal(validateBoundary("promotion", valid).admission, "exhaustive");
  assert.equal(validateBoundary("promotion", valid).snapshot, head);
  assert.throws(() => validateBoundary("promotion", { ...valid, CONDUIT_HEAD_REF: "dev" }), /promotion snapshot ref/);
  assert.throws(() => validateBoundary("promotion", { ...valid, CONDUIT_HEAD_REF: "promote/different" }), /promotion snapshot ref/);
  assert.throws(() => validateBoundary("promotion", { ...valid, CONDUIT_HEAD_SHA: "0123456" }), /full lowercase Git commit SHA/);
  assert.throws(() => validateBoundary("promotion", { ...valid, CONDUIT_HEAD_REPOSITORY: "fork/conduit" }), /promotion repository/);
});

test("workflow topology keeps fast development separate from stable promotion", () => {
  const candidate = readFileSync(".github/workflows/candidate.yml", "utf8");
  const integration = readFileSync(".github/workflows/dev-integration.yml", "utf8");
  const promotion = readFileSync(".github/workflows/promotion.yml", "utf8");
  const deploy = readFileSync(".github/workflows/tour-pages-deploy.yml", "utf8");
  assert.match(candidate, /branches: \[dev\]/);
  assert.match(integration, /branches: \[dev\]/);
  assert.match(promotion, /branches: \[main\]/);
  assert.match(promotion, /full_suite: true/g);
  const check = readFileSync(".github/workflows/check.yml", "utf8");
  const classification = check.split("      - name: Classify exact change set\n")[1]
    .split("      - name:")[0];
  assert.match(classification, /CONDUIT_FULL_SUITE: \$\{\{ inputs.full_suite \}\}/);
  assert.match(check, /docs_only: \$\{\{ steps.slice.outputs.docs_only \|\| steps.changes.outputs.docs_only \}\}/);
  assert.match(deploy, /startsWith\(github\.event\.pull_request\.head\.ref, 'promote\/'\)/);
  const closedTrigger = deploy.split("  pull_request_target:\n")[1].split("  workflow_dispatch:")[0];
  assert.match(closedTrigger, /types: \[closed\]/);
  assert.match(closedTrigger, /branches: \[main\]/);
  // A std-only or documentation-only promotion still has a proven carrier.
  assert.doesNotMatch(closedTrigger, /paths(?:-ignore)?:/);
  assert.match(deploy, /github\.event\.pull_request\.merged == true/);
  assert.match(deploy, /github\.event\.pull_request\.base\.ref == 'main'/);
  assert.match(promotion, /CONDUIT_HEAD_SHA:.*pull_request\.head\.sha/);
  assert.match(promotion, /Verify exact source snapshot and merge ancestry/);
  assert.match(promotion, /fetch-depth: 2/);
  assert.match(promotion, /FIRST_PARENT.*DEV_SNAPSHOT.*EXTRA/);
  assert.match(promotion, /HEAD_SHA\^\{tree\}/);
  assert.match(promotion, /Conduit-Dev-Snapshot/);
});

test("documentation-only promotion prepares proofs while ordinary documentation stays cheap", () => {
  const classifier = resolve("tools/ci/classify-docs-only.sh");
  const directory = mkdtempSync(join(tmpdir(), "conduit-promotion-classify-"));
  const git = (...args) => execFileSync("git", args, { cwd: directory, encoding: "utf8" }).trim();
  try {
    git("init", "--quiet");
    git("config", "user.name", "Conduit proof");
    git("config", "user.email", "proof@example.invalid");
    writeFileSync(join(directory, "README.md"), "before\n");
    git("add", "README.md");
    git("commit", "--quiet", "-m", "base");
    const base = git("rev-parse", "HEAD");
    writeFileSync(join(directory, "README.md"), "after\n");
    git("commit", "--quiet", "-am", "documentation");
    const head = git("rev-parse", "HEAD");
    for (const full of ["false", "true"]) {
      const output = execFileSync("bash", [classifier, base, head], {
        cwd: directory, encoding: "utf8",
        env: { ...process.env, CONDUIT_FULL_SUITE: full },
      });
      const fields = Object.fromEntries(output.trim().split("\n").map((line) => line.split("=")));
      assert.equal(fields.docs_only, full === "true" ? "false" : "true");
      assert.equal(fields.reason, full === "true" ? "full-suite" : "all-markdown");
      assert.equal(fields.base_sha, base);
      assert.equal(fields.head_sha, head);
      assert.equal(fields.comparison_base_sha, base);
    }
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
