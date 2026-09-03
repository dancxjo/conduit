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

test("every browser product admits the complete shared presentation theme", () => {
  const themeBytes = readFileSync("targets/browser/host/assets/application-theme.css").byteLength;
  for (const path of [
    "targets/browser/host/assets/book.application.template.json",
    "targets/browser/host/assets/creche.application.template.json",
    "apps/patchbay/html/assets/patchbay.application.template.json",
  ]) {
    const manifest = JSON.parse(readFileSync(path, "utf8"));
    const resource = manifest.resources.find(({ role }) => role === "shared-presentation-style");
    assert.ok(resource, `${path} omits the shared presentation theme`);
    assert.ok(themeBytes <= resource.maximum_bytes, `${path} theme bound is stale`);
  }
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
  assert.match(productWorkflow, /name: browser-proof-\$\{\{ matrix\.shard \}\}/);
  assert.match(productWorkflow, /shard: tour/);
  assert.match(productWorkflow, /shard: browser-host/);
  assert.match(productWorkflow, /shard: creche-machines/);
  assert.match(productWorkflow, /shard: pages/);
  assert.match(productWorkflow, /--workers 1/);
  assert.match(productWorkflow, /--retries 0/);
  assert.match(
    productWorkflow,
    /Retain the browser two-profile fabrication report\n        if: matrix\.shard == 'creche-machines'/,
  );
  assert.match(productWorkflow, /name: conduitos-release-\$\{\{ matrix\.architecture \}\}/);
  for (const architecture of ["x86_64", "aarch64", "ia32", "riscv64", "loongarch64"]) {
    assert.match(productWorkflow, new RegExp(`architecture: ${architecture}`));
  }
  assert.match(productWorkflow, /Restore an identical admitted ConduitOS image/);
  assert.match(productWorkflow, /if: steps\.image-cache\.outputs\.cache-hit != 'true'/);
  assert.match(productWorkflow, /conduitos-releases:\n    needs: conduitos-release-images/);
  assert.match(productWorkflow, /products-proof:\n    needs: \[products-stage, browser-proof, pages-carrier\]\n    if: always\(\)/);
  assert.match(productWorkflow, /test "\$STAGE_RESULT" = success/);
  assert.match(productWorkflow, /test "\$BROWSER_RESULT" = success/);
  assert.match(productWorkflow, /test "\$CARRIER_RESULT" = success/);

  const deployWorkflow = readFileSync(".github/workflows/executable-book-deploy.yml", "utf8");
  assert.match(
    deployWorkflow,
    /concurrency:\n  group: book-and-creche-pages\n  cancel-in-progress: false/,
  );
  assert.match(
    deployWorkflow,
    /github\.event\.pull_request\.merged == true && github\.event\.pull_request\.base\.ref == 'main'/,
  );
});
