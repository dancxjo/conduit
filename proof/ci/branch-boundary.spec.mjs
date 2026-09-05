import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { validateBoundary } from "../../scripts/ci/branch-boundary.mjs";

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
  const deploy = readFileSync(".github/workflows/executable-book-deploy.yml", "utf8");
  assert.match(candidate, /branches: \[dev\]/);
  assert.match(integration, /branches: \[dev\]/);
  assert.match(promotion, /branches: \[main\]/);
  assert.match(promotion, /full_suite: true/g);
  assert.match(deploy, /startsWith\(github\.event\.pull_request\.head\.ref, 'promote\/'\)/);
  assert.match(promotion, /CONDUIT_HEAD_SHA:.*pull_request\.head\.sha/);
  assert.match(promotion, /Verify exact source snapshot and merge ancestry/);
  assert.match(promotion, /fetch-depth: 2/);
  assert.match(promotion, /FIRST_PARENT.*DEV_SNAPSHOT.*EXTRA/);
  assert.match(promotion, /HEAD_SHA\^\{tree\}/);
  assert.match(promotion, /Conduit-Dev-Snapshot/);
});
