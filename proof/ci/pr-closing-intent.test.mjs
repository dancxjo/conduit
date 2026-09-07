import assert from "node:assert/strict";
import fs from "node:fs";
import test from "node:test";

import {
  negatedClosingReferences,
  verifyPullRequestClosingIntent,
} from "../../tools/ci/pr-closing-intent.mjs";

test("rejects negated keywords that GitHub interprets as issue closure", () => {
  for (const body of [
    "This does not close #2297.",
    "The slice did not fix issue #12.",
    "This mustn't resolve #44 yet.",
  ]) {
    assert.throws(
      () => verifyPullRequestClosingIntent({ pull_request: { body } }),
      /GitHub still treats that wording as a closure directive/,
    );
  }
});

test("permits explicit closure and unambiguous partial-work wording", () => {
  for (const body of [
    "Closes #2297.",
    "Fixes issue #12.",
    "Advances #2297 and leaves #2297 open.",
    "This is partial work for #2297.",
    "No issue reference is present.",
  ]) {
    assert.doesNotThrow(() => verifyPullRequestClosingIntent({ pull_request: { body } }));
  }
});

test("reports every unsafe reference for a useful candidate failure", () => {
  assert.deepEqual(
    negatedClosingReferences("Does not close #1 and will not resolve issue #2."),
    ["Does not close #1", "will not resolve issue #2"],
  );
});

test("the candidate gate runs the closing-intent guard", () => {
  const workflow = fs.readFileSync(new URL("../../.github/workflows/candidate.yml", import.meta.url), "utf8");
  assert.match(workflow, /node tools\/ci\/pr-closing-intent\.mjs/);
  assert.match(workflow, /proof\/ci\/pr-closing-intent\.test\.mjs/);
});
