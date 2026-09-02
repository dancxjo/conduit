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
