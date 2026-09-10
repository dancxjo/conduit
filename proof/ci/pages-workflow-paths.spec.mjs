import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { execFileSync, spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

test("affected product proof begins after cheap PR entry while promotion stays privileged", () => {
  const productWorkflow = readFileSync(".github/workflows/tour-products.yml", "utf8");
  const candidateWorkflow = readFileSync(".github/workflows/candidate.yml", "utf8");
  const integrationWorkflow = readFileSync(".github/workflows/dev-integration.yml", "utf8");
  assert.match(candidateWorkflow, /^  pull_request:\s*$/m);
  assert.match(candidateWorkflow, /products:\n    needs: admission\n    uses: \.\/\.github\/workflows\/tour-products\.yml/);
  assert.match(candidateWorkflow, /development_admission: true/);
  assert.match(integrationWorkflow, /uses: \.\/\.github\/workflows\/tour-products\.yml/);
  assert.doesNotMatch(productWorkflow, /^  pull_request:\s*$/m);
  assert.doesNotMatch(productWorkflow, /paths:\s*&product-paths/);
  assert.match(productWorkflow, /jobs:\n  plan:/);
  assert.match(productWorkflow, /Resolve the current trusted CI controller/);
  assert.match(productWorkflow, /git worktree add --detach "\$RUNNER_TEMP\/conduit-ci-controller"/);
  assert.match(productWorkflow, /pages_products_required/);

  const deploy = readFileSync(".github/workflows/tour-pages-deploy.yml", "utf8");
  assert.match(deploy, /pull_request_target:\n    types: \[closed\]\n    branches: \[main\]/);
  const closedTrigger = deploy.split("  pull_request_target:\n")[1].split("  workflow_dispatch:")[0];
  assert.doesNotMatch(closedTrigger, /^    paths(?:-ignore)?:/m);
});

test("every browser product admits the complete shared presentation theme", () => {
  const themeBytes = readFileSync("products/shared/browser/conduit.css").byteLength;
  for (const path of [
    "products/tour/browser/tour.application.template.json",
    "products/creche/browser/creche.application.template.json",
    "products/patchbay/html/assets/patchbay.application.template.json",
  ]) {
    const manifest = JSON.parse(readFileSync(path, "utf8"));
    const resource = manifest.resources.find(({ role }) => role === "shared-presentation-style");
    assert.ok(resource, `${path} omits the shared presentation theme`);
    assert.ok(themeBytes <= resource.maximum_bytes, `${path} theme bound is stale`);
  }
});

test("product jobs build the immutable PR head and deployments queue", () => {
  const productWorkflow = readFileSync(".github/workflows/tour-products.yml", "utf8");
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
  assert.match(productWorkflow, /browser-admission-stage:\n    needs: \[plan, browser-runtimes\]/);
  assert.match(productWorkflow, /stage-creche-product\.sh[^\n]+unused browser-proof/);
  assert.match(productWorkflow, /name: browser-admission-\$\{\{ matrix\.shard \}\}/);
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
  assert.match(productWorkflow, /products-proof:\n    needs: \[plan, tour-patchbay-proof, browser-admission-proof, products-stage, browser-proof, pages-carrier, proof-receipts\]/);
  assert.match(productWorkflow, /if test "\$PRODUCT_REQUIRED" != true/);
  assert.match(productWorkflow, /test "\$STAGE_RESULT" = success/);
  assert.match(productWorkflow, /test "\$TOUR_PATCHBAY_RESULT" = success/);
  assert.match(productWorkflow, /test "\$BROWSER_RESULT" = success/);
  assert.match(productWorkflow, /test "\$BROWSER_ADMISSION_RESULT" = success/);
  assert.match(productWorkflow, /test "\$CARRIER_RESULT" = success/);
  assert.match(productWorkflow, /test "\$RECEIPTS_RESULT" = success/);

  const deployWorkflow = readFileSync(".github/workflows/tour-pages-deploy.yml", "utf8");
  assert.match(
    deployWorkflow,
    /concurrency:\n  group: tour-and-creche-pages\n  cancel-in-progress: false/,
  );
  assert.match(
    deployWorkflow,
    /github\.event\.pull_request\.merged == true\s*&&\s*github\.event\.pull_request\.base\.ref == 'main'/,
  );
  assert.match(deployWorkflow, /carrier_name: \$\{\{ steps\.resolve\.outputs\.carrier_name \}\}/);
  assert.match(deployWorkflow, /name: \$\{\{ needs\.resolve\.outputs\.carrier_name \}\}/);

  const promotionWorkflow = readFileSync(".github/workflows/promotion.yml", "utf8");
  assert.match(promotionWorkflow, /pages-carrier-with-conduitos:\n    needs: \[products, conduitos-spore-acceptance\]/);
  assert.match(promotionWorkflow, /stage-conduitos-pages-evidence\.mjs/);
  assert.match(promotionWorkflow, /name: conduit-pages-carrier-with-conduitos/);
});

test("standalone locks fail before ESP32 fabrication fans out", () => {
  const checkWorkflow = readFileSync(".github/workflows/check.yml", "utf8");
  const productWorkflow = readFileSync(".github/workflows/tour-products.yml", "utf8");

  assert.match(
    checkWorkflow,
    /standalone-locks:\n    needs: classify[\s\S]*?"\$\{controller\[@\]\}" ci standalone-locks --locked/,
  );
  assert.match(checkWorkflow, /esp32-firmware:\n    needs: \[classify, standalone-locks, conduitos-x86\]/);
  assert.match(
    productWorkflow,
    /standalone-locks:\n    needs: plan[\s\S]*?conduit-xtask-dispatch"\n          ci standalone-locks --locked/,
  );
  assert.match(productWorkflow, /esp32-release-images:\n    needs: \[plan, standalone-locks\]/);
});

function pagesJob(name) {
  const source = readFileSync(".github/workflows/tour-pages-deploy.yml", "utf8");
  const jobs = source.split("\njobs:\n")[1];
  const body = jobs.match(new RegExp(`^  ${name}:\\n([\\s\\S]*?)(?=^  [\\w-]+:\\n|$(?![\\s\\S]))`, "m"))?.[1];
  assert.ok(body, `missing Pages job ${name}`);
  return body;
}

function recoveryScript(name) {
  const body = pagesJob("deploy-first-rescue").split(`      - name: ${name}\n`)[1];
  assert.ok(body, `missing recovery step ${name}`);
  const script = body.match(/^        run: \|\n((?:          [^\n]*\n)+)/m)?.[1];
  assert.ok(script, `missing recovery script ${name}`);
  return script.replace(/^          /gm, "");
}

test("Pages execute explicitly selects the carrier even for metadata-only changes", () => {
  const producer = pagesJob("integration-products");
  assert.match(producer, /if: needs\.resolve\.outputs\.disposition == 'execute'/);
  const selected = producer.match(/^      execute_proofs: '([^']+)'$/m)?.[1];
  assert.ok(selected, "execute must not fall back to diff-only product selection");
  assert.deepEqual(JSON.parse(selected), ["products.pages-carrier"]);
  assert.match(producer, /candidate_sha: \$\{\{ needs\.resolve\.outputs\.merge_commit \}\}/);
  assert.doesNotMatch(producer, /full_suite: true|development_admission: true/);
  const consumer = pagesJob("deploy");
  assert.match(consumer, /needs: \[resolve, integration-products\]/);
  assert.match(consumer, /needs\.integration-products\.result == 'success'/);
  assert.match(consumer, /name: conduit-pages-carrier/);
  assert.match(consumer, /verify-pages-carrier\.mjs target\/pages-carrier "\$EXPECTED_TREE"/);
});

test("Pages recovery is restricted to the exact dev audit trigger or accepted release merge", () => {
  const source = readFileSync(".github/workflows/tour-pages-deploy.yml", "utf8");
  const push = source.match(/^  push:\n((?:    [^\n]+\n)+)/m)?.[1];
  assert.equal(push, "    branches: [dev]\n    paths: [.github/release-audit/47372-publication.md]\n");
  const rescue = pagesJob("deploy-first-rescue");
  const guard = rescue.match(/^    if: >-\n((?:      [^\n]+\n)+)/m)?.[1];
  assert.ok(guard);
  const evaluate = Function("github", `"use strict"; return (${guard.trim()});`);
  const release = "release/47372b96d31ca8a04e9868a88bdd1919a9cad986";
  for (const event_name of ["push", "pull_request", "pull_request_target", "workflow_dispatch"]) {
    for (const ref of ["refs/heads/dev", "refs/heads/main", "refs/heads/feature"]) {
      for (const merged of [true, false]) {
        for (const base of ["main", "dev"]) {
          for (const head of [release, "release/other", "feature"]) {
            const github = { event_name, ref, event: { pull_request: { merged, base: { ref: base }, head: { ref: head } } } };
            const expected = event_name === "push" && ref === "refs/heads/dev"
              || event_name === "pull_request_target" && merged && base === "main" && head === release;
            assert.equal(evaluate(github), expected, JSON.stringify(github));
          }
        }
      }
    }
  }
  assert.match(rescue, /EXPECTED_MAIN_SHA: 69b39d54c5294f0c2781dded213d52789eff953e/);
  assert.match(rescue, /ref: f1e0245a8a55116a91c334d4e6f2719d81bc2ff3/);
  assert.match(rescue, /name: conduit-staged-browser-products/);
  assert.match(rescue, /run-id: 34522795014/);
  assert.match(rescue, /digest-mismatch: error/);
  assert.ok(rescue.indexOf("Refuse a stale recovery") < rescue.indexOf("Reuse the exact already-built"));
  assert.ok(rescue.indexOf("Recheck current main") < rescue.indexOf("      - name: Deploy Conduit Pages"));
  assert.doesNotMatch(rescue, /ref: \$\{\{ github\.(?:sha|ref) \}\}|ref: dev\n/);
});

test("Pages recovery executes freshness and source-integrity checks before publishing", () => {
  const directory = mkdtempSync(join(tmpdir(), "conduit-pages-recovery-"));
  const git = (...args) => execFileSync("git", ["-c", "user.name=Conduit proof", "-c", "user.email=proof@example.invalid", ...args], { cwd: directory, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }).trim();
  const beforeDownload = recoveryScript("Refuse a stale recovery or changed released product source");
  const beforePublish = recoveryScript("Recheck current main immediately before publication");
  try {
    git("init", "-b", "main");
    git("remote", "add", "origin", directory);
    writeFileSync(join(directory, "product.txt"), "accepted product\n");
    git("add", "."); git("commit", "-m", "accepted source");
    const source = git("rev-parse", "HEAD");
    mkdirSync(join(directory, ".github/workflows"), { recursive: true });
    mkdirSync(join(directory, ".github/release-audit"), { recursive: true });
    writeFileSync(join(directory, ".github/workflows/tour-pages-deploy.yml"), "recovery wiring\n");
    writeFileSync(join(directory, ".github/release-audit/47372-publication.md"), "accepted recovery\n");
    git("add", "."); git("commit", "-m", "install recovery");
    const acceptedMain = git("rev-parse", "HEAD");
    const run = (script, expectedMain) => spawnSync("bash", ["-e", "-c", script], {
      cwd: directory, encoding: "utf8",
      env: { ...process.env, EXPECTED_MAIN_SHA: expectedMain, RELEASE_SOURCE_SHA: source },
    });
    for (const script of [beforeDownload, beforePublish]) {
      const accepted = run(script, acceptedMain);
      assert.equal(accepted.status, 0, accepted.stderr);
      assert.notEqual(run(script, "0".repeat(40)).status, 0, "wrong current main must refuse");
    }
    writeFileSync(join(directory, "product.txt"), "newer product must not be overwritten\n");
    git("add", "."); git("commit", "-m", "newer release");
    assert.notEqual(run(beforeDownload, acceptedMain).status, 0);
    assert.notEqual(run(beforePublish, acceptedMain).status, 0);
    assert.notEqual(run(beforeDownload, git("rev-parse", "HEAD")).status, 0,
      "even an explicitly named new main cannot relabel changed product source");
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
