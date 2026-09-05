import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const workflow = readFileSync(process.env.CONDUIT_CHECK_WORKFLOW
  ?? new URL("../../.github/workflows/check.yml", import.meta.url), "utf8");
const gate = workflow.split("      - name: Preserve the stable required workspace gate\n")[1]
  ?.split("\n\n  browser-tools:")[0];
assert.ok(gate, "final check gate exists");
const encoded = gate.split("        run: |\n")[1];
assert.ok(encoded, "final check gate owns executable result validation");
const script = encoded.split("\n").map((line) => line.startsWith("          ") ? line.slice(10) : line).join("\n");

function results(overrides = {}) {
  return {
    CLASSIFY_RESULT: "success",
    DOCS_ONLY: "false",
    WORKSPACE_RESULT: "success",
    WORKSPACE_MATRIX: '["test-products"]',
    ESP32_RESULT: "skipped",
    STANDALONE_LOCKS_RESULT: "skipped",
    BROWSER_HOST_RESULT: "skipped",
    LIMINE_RESULT: "skipped",
    TOOLS_RESULT: "skipped",
    X86_RESULT: "skipped",
    ARCHITECTURE_RESULT: "skipped",
    AARCH64_PRODUCT_RESULT: "skipped",
    CONDUITOS_REQUIRED: "false",
    X86_REQUIRED: "false",
    ARCHITECTURE_REQUIRED: "false",
    AARCH64_PRODUCT_REQUIRED: "false",
    ESP32_REQUIRED: "false",
    EVENT_NAME: "pull_request",
    ...overrides,
  };
}

function prove(environment) {
  return spawnSync("bash", ["-euo", "pipefail", "-c", script], {
    env: { PATH: process.env.PATH, GITHUB_STEP_SUMMARY: "/dev/null", ...environment },
    encoding: "utf8",
  });
}

test("selective pull request admits intentionally skipped ConduitOS", () => {
  assert.equal(prove(results()).status, 0);
});

test("shared compile failure causally blocks candidate-code proof domains", () => {
  const blocked = prove(results({
    SHARED_COMPILE_RESULT: "failure",
    SHARED_COMPILE_PACKAGES: '["conduit-presentation"]',
    WORKSPACE_RESULT: "skipped",
  }));
  assert.notEqual(blocked.status, 0);
  assert.match(blocked.stderr, /blocked-by: workspace\.shared-compile/);
});

test("selective pull request admits only its required x86 subset", () => {
  assert.equal(prove(results({
    CONDUITOS_REQUIRED: "true",
    X86_REQUIRED: "true",
    LIMINE_RESULT: "success",
    TOOLS_RESULT: "success",
    X86_RESULT: "success",
  })).status, 0);
});

test("exhaustive integration requires every ConduitOS aggregate", () => {
  const exhaustive = results({
    EVENT_NAME: "merge_group",
    ESP32_RESULT: "success",
    STANDALONE_LOCKS_RESULT: "success",
    LIMINE_RESULT: "success",
    TOOLS_RESULT: "success",
    X86_RESULT: "success",
    ARCHITECTURE_RESULT: "success",
    AARCH64_PRODUCT_RESULT: "success",
  });
  assert.equal(prove(exhaustive).status, 0);
  for (const field of [
    "LIMINE_RESULT",
    "TOOLS_RESULT",
    "X86_RESULT",
    "ARCHITECTURE_RESULT",
    "AARCH64_PRODUCT_RESULT",
  ]) {
    const failed = prove({ ...exhaustive, [field]: "failure" });
    assert.notEqual(failed.status, 0, field);
    assert.match(failed.stderr, /expected success, got failure/);
  }
});

test("documentation-only behavior does not promote skipped machine proof", () => {
  assert.equal(prove(results({
    DOCS_ONLY: "true",
    WORKSPACE_RESULT: "skipped",
  })).status, 0);
});
