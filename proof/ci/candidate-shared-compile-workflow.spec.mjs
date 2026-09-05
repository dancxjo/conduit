import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const workflow = await readFile(
  new URL("../../.github/workflows/candidate-shared-compile.yml", import.meta.url),
  "utf8",
);
const checkWorkflow = await readFile(
  new URL("../../.github/workflows/check.yml", import.meta.url),
  "utf8",
);

test("stack classification receives the exact immutable candidate identity", () => {
  const step = workflow.match(
    /- name: Classify the immutable PR stack position[\s\S]*?(?=\n      - name:)/,
  )?.[0];

  assert.ok(step, "stack-classification step is present");
  assert.match(step, /CONDUIT_PR_NUMBER: \$\{\{ inputs\.pr_number \}\}/);
  assert.match(step, /CONDUIT_CANDIDATE_SHA: \$\{\{ inputs\.candidate_sha \}\}/);
});

test("the reusable check lane cannot deadlock with its top-level caller", () => {
  assert.match(checkWorkflow, /group: check-\$\{\{/);
  assert.doesNotMatch(checkWorkflow, /group: \$\{\{ github\.workflow \}\}/);
});
