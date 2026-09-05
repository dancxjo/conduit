import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

test("the PR product controller owns applicability while promotion stays privileged", () => {
  const productWorkflow = readFileSync(".github/workflows/executable-book-pages.yml", "utf8");
  const candidateWorkflow = readFileSync(".github/workflows/candidate.yml", "utf8");
  assert.match(candidateWorkflow, /^  pull_request:\s*$/m);
  assert.match(candidateWorkflow, /uses: \.\/\.github\/workflows\/executable-book-pages\.yml/);
  assert.doesNotMatch(productWorkflow, /^  pull_request:\s*$/m);
  assert.doesNotMatch(productWorkflow, /paths:\s*&product-paths/);
  assert.match(productWorkflow, /jobs:\n  plan:/);
  assert.match(productWorkflow, /Resolve the current trusted CI controller/);
  assert.match(productWorkflow, /git worktree add --detach "\$RUNNER_TEMP\/conduit-ci-controller"/);
  assert.match(productWorkflow, /pages_products_required/);

  const deploy = readFileSync(".github/workflows/executable-book-deploy.yml", "utf8");
  assert.match(deploy, /pull_request_target:\n    types: \[closed\]\n    branches: \[main\]/);
  assert.doesNotMatch(deploy, /^    paths(?:-ignore)?:/m);
});

test("every browser product admits the complete shared presentation theme", () => {
  const themeBytes = readFileSync("targets/browser/host/assets/application-theme.css").byteLength;
  for (const path of [
    "targets/browser/host/assets/book.application.template.json",
    "targets/browser/host/assets/creche.application.template.json",
    "products/patchbay/html/assets/patchbay.application.template.json",
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
    /ref: \$\{\{ env\.CONDUIT_CANDIDATE_SHA \}\}/g,
  )].length;
  assert.ok(checkoutCount > 0);
  assert.equal(exactHeadCount, checkoutCount);
  assert.doesNotMatch(productWorkflow, /target\/ci-controller/);
  assert.doesNotMatch(productWorkflow, /ref: \$\{\{ github\.sha \}\}/);
  assert.doesNotMatch(productWorkflow, /\n        with:\n(?:          [^\n]+\n)+        with:/);
  assert.match(productWorkflow, /source_commit=\$\(git .* rev-parse HEAD\)/);
  assert.doesNotMatch(productWorkflow, /source_commit="\$GITHUB_SHA"/);
  assert.match(
    productWorkflow,
    /CONDUIT_CANDIDATE_SHA: \$\{\{ inputs\.candidate_sha \|\| github\.event\.pull_request\.head\.sha \}\}/,
  );
  assert.match(
    productWorkflow,
    /CONDUIT_BASE_SHA: \$\{\{ inputs\.base_sha \|\| github\.event\.pull_request\.base\.sha \}\}/,
  );
  assert.doesNotMatch(productWorkflow, /github\.event\.pull_request\.(?:head|base)\.sha \|\| inputs\./);
  assert.match(productWorkflow, /name: browser-proof-\$\{\{ matrix\.shard \}\}/);
  assert.match(
    productWorkflow,
    /tour-patchbay-proof:\n    needs: \[plan, browser-runtimes\]/,
  );
  assert.doesNotMatch(productWorkflow, /conduit-staged-tour-patchbay/);
  assert.match(productWorkflow, /--grep-invert/);
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
  assert.match(productWorkflow, /products-proof:\n    needs: \[plan, tour-patchbay-proof, products-stage, browser-proof, pages-carrier, proof-receipts\]/);
  assert.match(productWorkflow, /if test "\$PRODUCT_REQUIRED" != true/);
  assert.match(productWorkflow, /test "\$STAGE_RESULT" = success/);
  assert.match(productWorkflow, /test "\$TOUR_PATCHBAY_RESULT" = success/);
  assert.match(productWorkflow, /test "\$BROWSER_RESULT" = success/);
  assert.match(productWorkflow, /test "\$CARRIER_RESULT" = success/);
  assert.match(productWorkflow, /test "\$RECEIPTS_RESULT" = success/);

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

test("standalone locks fail before ESP32 fabrication fans out", () => {
  const checkWorkflow = readFileSync(".github/workflows/check.yml", "utf8");
  const productWorkflow = readFileSync(".github/workflows/executable-book-pages.yml", "utf8");

  assert.match(
    checkWorkflow,
    /standalone-locks:\n    needs: classify[\s\S]*?"\$\{controller\[@\]\}" ci standalone-locks --locked/,
  );
  assert.match(checkWorkflow, /esp32-firmware:\n    needs: \[classify, standalone-locks\]/);
  assert.match(
    productWorkflow,
    /standalone-locks:\n    needs: plan[\s\S]*?conduit-xtask-dispatch"\n          ci standalone-locks --locked/,
  );
  assert.match(productWorkflow, /esp32-release-images:\n    needs: \[plan, standalone-locks\]/);
});
