import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import test from "node:test";

test("affected product proof begins after cheap PR entry while promotion stays privileged", () => {
  const productWorkflow = readFileSync(".github/workflows/product-carrier.yml", "utf8");
  const candidateWorkflow = readFileSync(".github/workflows/candidate.yml", "utf8");
  const integrationWorkflow = readFileSync(".github/workflows/dev-integration.yml", "utf8");
  assert.match(candidateWorkflow, /^  pull_request:\s*$/m);
  assert.match(candidateWorkflow, /products:\n    needs: admission\n    if: github\.event\.pull_request\.draft == false\n    uses: \.\/\.github\/workflows\/product-carrier\.yml/);
  assert.match(candidateWorkflow, /development_admission: true/);
  assert.match(integrationWorkflow, /uses: \.\/\.github\/workflows\/product-carrier\.yml/);
  assert.doesNotMatch(productWorkflow, /^  pull_request:\s*$/m);
  assert.doesNotMatch(productWorkflow, /paths:\s*&product-paths/);
  assert.match(productWorkflow, /jobs:\n  plan:/);
  assert.match(productWorkflow, /Resolve the current trusted CI controller/);
  assert.match(productWorkflow, /git worktree add --detach "\$RUNNER_TEMP\/conduit-ci-controller"/);
  assert.match(productWorkflow, /pages_products_required/);

  const deploy = readFileSync(".github/workflows/pages-deploy.yml", "utf8");
  assert.match(deploy, /pull_request_target:\n    types: \[closed\]\n    branches: \[main\]/);
  const closedTrigger = deploy.split("  pull_request_target:\n")[1].split("  workflow_dispatch:")[0];
  assert.doesNotMatch(closedTrigger, /^    paths(?:-ignore)?:/m);
});

test("every browser product admits the complete shared presentation theme", () => {
  const themeBytes = readFileSync("targets/browser/host/assets/conduit.css").byteLength;
  for (const path of [
    "products/workspace/browser/workspace.application.template.json",
    "products/patchbay/html/assets/patchbay.application.template.json",
  ]) {
    const manifest = JSON.parse(readFileSync(path, "utf8"));
    const resource = manifest.resources.find(({ role }) => role === "shared-presentation-style");
    assert.ok(resource, `${path} omits the shared presentation theme`);
    assert.ok(themeBytes <= resource.maximum_bytes, `${path} theme bound is stale`);
  }
});

test("product jobs build the immutable PR head and deployments queue", () => {
  const productWorkflow = readFileSync(".github/workflows/product-carrier.yml", "utf8");
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
  assert.doesNotMatch(productWorkflow, /shard: tour/);
  assert.match(productWorkflow, /shard: browser-host/);
  assert.match(productWorkflow, /shard: workspace-machines/);
  assert.match(productWorkflow, /shard: pages/);
  assert.match(productWorkflow, /browser-admission-stage:\n    needs: \[plan, browser-runtimes, browser-release\]/);
  assert.match(productWorkflow, /browser-release:\n    needs: plan\n    if: needs\.plan\.outputs\.browser_release_required == 'true'/);
  assert.match(productWorkflow, /browser_release_required:.*contains\(steps\.impact\.outputs\.browser_admission_matrix, 'workspace'\).*contains\(steps\.impact\.outputs\.browser_admission_matrix, 'pages'\)/);
  assert.match(productWorkflow, /if: needs\.plan\.outputs\.browser_release_required == 'true'\n        uses: \.\/\.github\/actions\/download-artifact-retry/);
  assert.doesNotMatch(productWorkflow, /stage-creche-product|target\/creche-product|creche-runtime/);
  assert.match(productWorkflow, /stage-workspace-product\.sh/);
  assert.match(productWorkflow, /name: browser-admission-\$\{\{ matrix\.shard \}\}/);
  assert.match(
    productWorkflow,
    /cargo build --locked -p patchbay-html --bin patchbay-static-assets --bin patchbay-html -p patchbay-native --bin browser-parts-capstone/,
  );
  assert.match(
    productWorkflow,
    /cargo build --locked --bin webchat-server -p conduit-std-host --bin browser-admission-probe --bin protected-line-browser-peer/,
  );
  assert.match(
    productWorkflow,
    /name: conduit-development-browser-products[\s\S]*?target\/debug\/browser-parts-capstone/,
  );
  assert.match(
    productWorkflow,
    /name: conduit-development-browser-products[\s\S]*?target\/debug\/webchat-server/,
  );
  assert.match(
    productWorkflow,
    /name: conduit-staged-browser-products[\s\S]*?target\/debug\/browser-parts-capstone/,
  );
  assert.match(
    productWorkflow,
    /name: conduit-staged-browser-products[\s\S]*?target\/debug\/webchat-server/,
  );
  assert.match(productWorkflow, /--workers 1/);
  assert.match(productWorkflow, /--retries 0/);
  assert.match(
    productWorkflow,
    /browser_admission_matrix: \$\{\{ \(!inputs\.development_admission \|\| inputs\.full_suite\) && '\["browser-host","workspace","pages"\]'/,
  );
  assert.match(
    productWorkflow,
    /shard: \$\{\{ fromJSON\(needs\.plan\.outputs\.browser_admission_matrix\) \}\}/,
  );
  const admissionJob = productWorkflow.split("  browser-admission-proof:\n")[1]
    .split("\n  avr-release:\n")[0];
  assert.match(admissionJob, /timeout-minutes: 15/);
  assert.doesNotMatch(admissionJob, /matrix:\n\s+include:/);
  assert.match(
    productWorkflow,
    /BROWSER_ADMISSION_MATRIX: \$\{\{ needs\.plan\.outputs\.browser_admission_matrix \}\}/,
  );
  assert.match(
    productWorkflow,
    /if \[\[ "\$BROWSER_ADMISSION_MATRIX" == \*'"workspace"'\* \|\| "\$BROWSER_ADMISSION_MATRIX" == \*'"pages"'\* \]\]; then\n\s+cargo build --locked -p conduit-browser-runtime[^\n]+--features creche-surface,form-runner/,
  );
  assert.match(
    productWorkflow,
    /if \[\[ "\$BROWSER_ADMISSION_MATRIX" == \*'"browser-host"'\* \|\| "\$BROWSER_ADMISSION_MATRIX" == \*'"workspace"'\* \|\| "\$BROWSER_ADMISSION_MATRIX" == \*'"pages"'\* \]\]; then\n\s+cargo build --locked -p conduit-browser-runtime --target wasm32-unknown-unknown --release\n\s+cp target\/wasm32-unknown-unknown\/release\/conduit_browser_runtime\.wasm target\/browser-product-runtimes\/patchbay-runtime\.wasm/,
  );
  assert.match(
    productWorkflow,
    /if \[\[ "\$BROWSER_ADMISSION_MATRIX" == \*'"browser-host"'\* \|\| "\$BROWSER_ADMISSION_MATRIX" == \*'"workspace"'\* \|\| "\$BROWSER_ADMISSION_MATRIX" == \*'"pages"'\* \]\]; then\n\s+cp target\/browser-product-runtimes\/patchbay-runtime\.wasm target\/wasm32-unknown-unknown\/release\/conduit_browser_runtime\.wasm/,
  );
  assert.match(
    productWorkflow,
    /Retain the browser two-profile make report\n        if: matrix\.shard == 'workspace-machines'/,
  );
  assert.match(productWorkflow, /name: conduitos-release-\$\{\{ matrix\.architecture \}\}/);
  for (const architecture of ["x86_64", "aarch64", "ia32", "riscv64", "loongarch64"]) {
    assert.match(productWorkflow, new RegExp(`architecture: ${architecture}`));
  }
  assert.match(productWorkflow, /Restore an identical admitted ConduitOS image/);
  assert.match(productWorkflow, /if: steps\.image-cache\.outputs\.cache-hit != 'true'/);
  assert.match(productWorkflow, /conduitos-releases:\n    needs: conduitos-release-images/);
  assert.match(productWorkflow, /products-proof:\n    needs: \[plan, browser-admission-proof, products-stage, browser-proof, pages-carrier\]/);
  assert.doesNotMatch(productWorkflow, /^  (?:journey|little-life)-evidence:/m);
  assert.doesNotMatch(productWorkflow, /^  journey-gallery:/m);
  assert.match(productWorkflow, /if test "\$PRODUCT_REQUIRED" != true/);
  assert.match(productWorkflow, /test "\$STAGE_RESULT" = success/);
  assert.match(productWorkflow, /test "\$BROWSER_RESULT" = success/);
  assert.match(productWorkflow, /test "\$BROWSER_ADMISSION_RESULT" = success/);
  assert.match(productWorkflow, /test "\$CARRIER_RESULT" = success/);

  const deployWorkflow = readFileSync(".github/workflows/pages-deploy.yml", "utf8");
  assert.match(
    deployWorkflow,
    /concurrency:\n  group: pages-deploy\n  cancel-in-progress: false/,
  );
  assert.match(
    deployWorkflow,
    /github\.event\.pull_request\.merged == true\s*&&\s*github\.event\.pull_request\.base\.ref == 'main'/,
  );
  assert.match(deployWorkflow, /carrier_name: \$\{\{ steps\.resolve\.outputs\.carrier_name \}\}/);
  assert.match(deployWorkflow, /name: \$\{\{ needs\.resolve\.outputs\.carrier_name \}\}/);

  const promotionWorkflow = readFileSync(".github/workflows/promotion.yml", "utf8");
  assert.match(promotionWorkflow, /promotion:\n    needs: \[boundary, check, products, conduitos-spore-acceptance\]/);
  assert.doesNotMatch(promotionWorkflow, /generative-body-journey|three-body-journey|journey-documentary/);
  assert.doesNotMatch(promotionWorkflow, /stage-(?:three-body-journey|journey-pages-evidence|conduitos-pages-evidence)/);

  const journeyWorkflow = readFileSync(".github/workflows/journey-publication.yml", "utf8");
  assert.match(journeyWorkflow, /workflow_dispatch:/);
  assert.match(journeyWorkflow, /name: conduit-release-publication-context/);
  assert.match(journeyWorkflow, /name: \$\{\{ steps\.context\.outputs\.carrier_name \}\}/);
  assert.match(journeyWorkflow, /run-id: \$\{\{ steps\.context\.outputs\.carrier_run_id \}\}/);
  assert.match(journeyWorkflow, /test "\$carrier_run_id" = "\$PAGES_RUN_ID"/);
  assert.match(journeyWorkflow, /conduit-browser-body-journey-/);
  assert.match(journeyWorkflow, /name: conduitos-x86-batch-/);
  assert.match(journeyWorkflow, /cargo xtask prove one-form-two-fronts --locked/);
  assert.match(journeyWorkflow, /cargo xtask prove little-life --locked/);
  assert.match(journeyWorkflow, /CONDUIT_CHECKOUT_SHA: \$\{\{ steps\.context\.outputs\.source_commit \}\}/);
  assert.match(journeyWorkflow, /cargo xtask prove gallery/);
  assert.match(journeyWorkflow, /proof\/fixtures\/ollama-http-service\.mjs/);
  assert.match(journeyWorkflow, /three-body-journey-contract/);
  assert.match(journeyWorkflow, /stage-three-body-journey/);
  assert.match(journeyWorkflow, /stage-conduitos-pages-evidence\.mjs/);
  assert.match(journeyWorkflow, /Refuse to overwrite a newer accepted release/);
  assert.match(deployWorkflow, /name: conduit-release-publication-context/);
  assert.match(deployWorkflow, /gh workflow run journey-publication\.yml --ref main/);

  const liveModelWorkflow = readFileSync(".github/workflows/live-local-model-conformance.yml", "utf8");
  assert.match(liveModelWorkflow, /workflow_dispatch:/);
  assert.match(liveModelWorkflow, /ollama\/ollama:0\.12\.3@sha256:/);
  assert.match(liveModelWorkflow, /host prove-local-model --locked/);
});

test("standalone locks fail before ESP32 make fans out", () => {
  const checkWorkflow = readFileSync(".github/workflows/check.yml", "utf8");
  const productWorkflow = readFileSync(".github/workflows/product-carrier.yml", "utf8");

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
  const source = readFileSync(".github/workflows/pages-deploy.yml", "utf8");
  const jobs = source.split("\njobs:\n")[1];
  const body = jobs.match(new RegExp(`^  ${name}:\\n([\\s\\S]*?)(?=^  [\\w-]+:\\n|$(?![\\s\\S]))`, "m"))?.[1];
  assert.ok(body, `missing Pages job ${name}`);
  return body;
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
  assert.match(consumer, /if: needs\.resolve\.outputs\.disposition == 'inherited'[\s\S]*?run-id: \$\{\{ needs\.resolve\.outputs\.run_id \}\}/);
  assert.match(consumer, /if: needs\.resolve\.outputs\.disposition == 'execute'[\s\S]*?name: conduit-pages-carrier/);
  assert.match(consumer, /verify-pages-carrier\.mjs target\/pages-carrier "\$EXPECTED_TREE"/);
  assert.match(consumer, /Install exact current-product truth and reseal the publication carrier/);
  assert.match(consumer, /if: needs\.resolve\.outputs\.direct_main != 'true'/);
  assert.match(consumer, /git fetch --no-tags origin dev/);
  assert.match(consumer, /emit-current-product-truth\.mjs/);
  assert.match(consumer, /target\/pages-carrier-with-truth/);
  assert.match(consumer, /name: conduit-release-pages-carrier-\$\{\{ needs\.resolve\.outputs\.source_head \}\}/);
  assert.match(consumer, /path: target\/pages-carrier/);
  assert.match(consumer, /CARRIER_RUN_ID: \$\{\{ github\.run_id \}\}/);
  assert.match(consumer, /--arg schema conduit\.release-publication-context\/v2/);
  assert.match(consumer, /carrier_run_id:\$carrier_run_id/);
  assert.match(consumer, /name: Upload the already-proven Pages carrier\n        if: needs\.resolve\.outputs\.direct_main == 'true'/);
  assert.match(consumer, /path: target\/pages-carrier\/site/);
  assert.doesNotMatch(consumer, /path: target\/pages-carrier-with-truth\/site/);
});

test("permanent Pages automation contains no completed deploy-first rescue", () => {
  const source = readFileSync(".github/workflows/pages-deploy.yml", "utf8");
  const resolver = readFileSync("tools/ci/resolve-pages-product-run.mjs", "utf8");
  assert.doesNotMatch(source, /^  push:|^  workflow_run:/m);
  assert.doesNotMatch(source, /^  deploy-first-rescue:/m);
  assert.doesNotMatch(source, /47372b96|34522795014|EXPECTED_MAIN_SHA|RELEASE_SOURCE_SHA/);
  assert.doesNotMatch(resolver, /deployFirstMerge|47372b96/);
});


test("Workspace admission receives its Host prerequisites without selecting Pages", () => {
  const workflow = readFileSync(".github/workflows/product-carrier.yml", "utf8");
  const build = workflow.split("  browser-runtimes:\n")[1].split("\n  standalone-locks:")[0];
  const stage = workflow.split("  browser-admission-stage:\n")[1].split("\n  browser-admission-proof:")[0];
  const cases = [
    [[], false, false, false],
    [["unrelated"], false, false, false],
    [["browser-host"], false, false, true],
    [["workspace"], true, false, true],
    [["pages"], true, true, true],
    [["browser-host", "workspace"], true, false, true],
    [["browser-host", "workspace", "pages"], true, true, true],
  ];
  for (const [source, command, kind] of [
    [build, "cargo build --locked -p patchbay-html", "binaries"],
    [stage, "cp target/browser-product-runtimes/patchbay-html", "binaries"],
    [stage, "products/patchbay/tools/stage-patchbay-product.sh", "pages"],
    [build, "cargo build --locked -p conduit-browser-runtime --target wasm32-unknown-unknown --release\n", "runtime"],
    [stage, "cp target/browser-product-runtimes/patchbay-runtime.wasm", "runtime"],
  ]) {
    const blocks = [...source.matchAll(/^          if \[\[ ([^\n]+) \]\]; then\n([\s\S]*?)^          fi$/gm)]
      .filter(([, , body]) => body.includes(command));
    assert.equal(blocks.length, 1, `one explicit admission gate for ${command}`);
    const [, condition, body] = blocks[0];
    if (kind === "binaries" && command.startsWith("cp ")) {
      assert.match(body, /target\/browser-product-runtimes\/browser-parts-capstone target\/debug\//);
      assert.match(body, /chmod 755 target\/debug\/patchbay-html target\/debug\/browser-parts-capstone/);
    }
    for (const [matrix, binaries, pages, runtime] of cases) {
      const result = spawnSync("bash", ["-c", `if [[ ${condition} ]]; then printf selected; fi`], {
        encoding: "utf8",
        env: { ...process.env, BROWSER_ADMISSION_MATRIX: JSON.stringify(matrix) },
      });
      assert.equal(result.status, 0, result.stderr);
      const selected = { binaries, pages, runtime }[kind];
      assert.equal(result.stdout, selected ? "selected" : "",
        `${command} for ${JSON.stringify(matrix)}`);
    }
  }
});
