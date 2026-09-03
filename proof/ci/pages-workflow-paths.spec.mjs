import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

function pullRequestPaths(path, marker) {
  const lines = readFileSync(path, "utf8").split("\n");
  const start = lines.findIndex((line) => line.trimStart().startsWith(marker));
  assert.notEqual(start, -1, `${path} omits ${marker}`);
  const paths = [];
  for (const line of lines.slice(start + 1)) {
    const match = line.match(/^      - (.+)$/);
    if (match) {
      paths.push(match[1]);
      continue;
    }
    if (paths.length > 0 && !line.startsWith("      ")) break;
  }
  return paths;
}

test("merged-PR deployment triggers exactly when PR product proof builds a carrier", () => {
  const productPaths = pullRequestPaths(
    ".github/workflows/executable-book-pages.yml",
    "paths: &product-paths",
  );
  const deployPaths = pullRequestPaths(
    ".github/workflows/executable-book-deploy.yml",
    "paths:",
  );
  assert.deepEqual(deployPaths, productPaths);
});

test("product jobs build the immutable PR head and deployments queue", () => {
  const productWorkflow = readFileSync(".github/workflows/executable-book-pages.yml", "utf8");
  const checkoutCount = [...productWorkflow.matchAll(/uses: actions\/checkout@v7/g)].length;
  const exactHeadCount = [...productWorkflow.matchAll(
    /ref: \$\{\{ github\.event\.pull_request\.head\.sha \|\| github\.sha \}\}/g,
  )].length;
  assert.ok(checkoutCount > 0);
  assert.equal(exactHeadCount, checkoutCount);
  assert.doesNotMatch(productWorkflow, /\n        with:\n(?:          [^\n]+\n)+        with:/);
  assert.match(productWorkflow, /source_commit=\$\(git .* rev-parse HEAD\)/);
  assert.doesNotMatch(productWorkflow, /source_commit="\$GITHUB_SHA"/);

  const deployWorkflow = readFileSync(".github/workflows/executable-book-deploy.yml", "utf8");
  assert.match(
    deployWorkflow,
    /concurrency:\n  group: book-and-creche-pages\n  cancel-in-progress: false/,
  );
});
