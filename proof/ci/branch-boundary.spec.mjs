import assert from "node:assert/strict";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { test } from "node:test";
import { validateBoundary } from "../../tools/ci/branch-boundary.mjs";

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

test("only a repairable same-repository release branch can promote to main", () => {
  const snapshot = "0123456789abcdef0123456789abcdef01234567";
  const valid = {
    CONDUIT_EVENT_NAME: "pull_request",
    CONDUIT_BASE_REF: "main",
    CONDUIT_HEAD_REF: `release/${snapshot}`,
    CONDUIT_HEAD_REPOSITORY: "dancxjo/conduit",
    CONDUIT_REPOSITORY: "dancxjo/conduit",
  };
  assert.equal(validateBoundary("promotion", valid).admission, "exhaustive");
  assert.equal(validateBoundary("promotion", valid).snapshot, snapshot);
  assert.equal(validateBoundary("promotion", valid).source, "repairable-release-branch");
  assert.throws(() => validateBoundary("promotion", { ...valid, CONDUIT_HEAD_REF: "dev" }), /promotion head/);
  assert.throws(() => validateBoundary("promotion", { ...valid, CONDUIT_HEAD_REF: "release/different" }), /promotion head/);
  assert.throws(() => validateBoundary("promotion", { ...valid, CONDUIT_HEAD_REPOSITORY: "fork/conduit" }), /promotion repository/);
});

test("workflow topology keeps fast development separate from stable promotion", () => {
  const candidate = readFileSync(".github/workflows/candidate.yml", "utf8");
  const integration = readFileSync(".github/workflows/dev-integration.yml", "utf8");
  const promotion = readFileSync(".github/workflows/promotion.yml", "utf8");
  const deploy = readFileSync(".github/workflows/tour-pages-deploy.yml", "utf8");
  assert.match(candidate, /branches: \[dev\]/);
  assert.match(candidate, /cargo test --locked --package conduit-xtask-dispatch/);
  assert.match(candidate, /proof\/ci\/release-lane-controller\.spec\.mjs/);
  assert.doesNotMatch(candidate, /tour-products\.yml/);
  assert.match(candidate, /group: candidate-\$\{\{ github\.event\.pull_request\.number \}\}/);
  assert.match(candidate, /cancel-in-progress: true/);
  assert.match(integration, /branches: \[dev\]/);
  assert.match(integration, /tour-products\.yml/);
  assert.match(integration, /group: dev-integration\n/);
  assert.match(integration, /cancel-in-progress: false/);
  assert.match(promotion, /branches: \[main\]/);
  assert.match(promotion, /full_suite: true/g);
  assert.match(promotion, /group: promotion-release-lane/);
  assert.match(promotion, /cancel-in-progress: false/);
  const check = readFileSync(".github/workflows/check.yml", "utf8");
  const classification = check.split("      - name: Classify exact change set\n")[1]
    .split("      - name:")[0];
  assert.match(classification, /CONDUIT_FULL_SUITE: \$\{\{ inputs.full_suite \}\}/);
  assert.match(check, /docs_only: \$\{\{ steps.slice.outputs.docs_only \|\| steps.changes.outputs.docs_only \}\}/);
  const fingerprint = check.split("      - name: Fingerprint immutable candidate proof inputs\n")[1]
    .split("      - name:")[0];
  assert.match(fingerprint, /CONDUIT_FULL_SUITE: \$\{\{ inputs.full_suite \}\}/);
  assert.match(fingerprint, /if test "\$CONDUIT_FULL_SUITE" != true && grep/);
  assert.match(deploy, /startsWith\(github\.event\.pull_request\.head\.ref, 'release\/'\)/);
  const closedTrigger = deploy.split("  pull_request_target:\n")[1].split("  workflow_dispatch:")[0];
  assert.match(closedTrigger, /types: \[closed\]/);
  assert.match(closedTrigger, /branches: \[main\]/);
  // A std-only or documentation-only promotion still has a proven carrier.
  assert.doesNotMatch(closedTrigger, /paths(?:-ignore)?:/);
  assert.match(deploy, /github\.event\.pull_request\.merged == true/);
  assert.match(deploy, /github\.event\.pull_request\.base\.ref == 'main'/);
  assert.match(promotion, /Verify the captured development snapshot remains in the release/);
  assert.match(promotion, /group: promotion-release-lane/);
  assert.doesNotMatch(promotion, /group: promotion-\$\{\{ github\.event\.pull_request\.number \}\}/);
  assert.match(promotion, /fetch-depth: 0/);
  assert.match(promotion, /git merge-base --is-ancestor "\$snapshot" origin\/dev/);
  assert.match(promotion, /git merge-base --is-ancestor "\$snapshot" HEAD/);
  const request = readFileSync(".github/workflows/promote-dev.yml", "utf8");
  assert.match(request, /workflow_dispatch/);
  assert.match(request, /workflow_run:/);
  assert.match(request, /workflows: \[dev-integration\]/);
  assert.match(request, /Record a superseded development integration/);
  assert.match(request, /A newer development head owns the next release decision/);
  assert.match(request, /Check out the exact successfully integrated development snapshot/);
  assert.match(request, /integrated_sha:/);
  assert.match(request, /inputs\.integrated_sha \|\| 'dev'/);
  assert.match(request, /test "\$dev_sha" = "\$INTEGRATION_SHA"/);
  assert.match(request, /git merge-base --is-ancestor "\$dev_sha" origin\/dev/);
  assert.doesNotMatch(request, /already-running/);
  assert.match(request, /already-current/);
  assert.match(request, /release\/\$dev_sha/);
  assert.match(request, /gh workflow run release-lane\.yml --ref main/);
  assert.doesNotMatch(request, /gh pr merge/);
  const sync = readFileSync(".github/workflows/sync-release-to-dev.yml", "utf8");
  assert.match(sync, /Sync release fixes to dev/);
  assert.match(sync, /workflow_dispatch:/);
  assert.match(sync, /Resolve the exact accepted release/);
  assert.match(sync, /--base dev/);
  assert.doesNotMatch(sync, /gh pr merge/);
  const devIntegration = readFileSync(".github/workflows/dev-integration.yml", "utf8");
  assert.match(devIntegration, /workflow_dispatch:/);
  assert.match(devIntegration, /inputs\.base_sha/);
  assert.match(devIntegration, /inputs\.candidate_sha/);
  assert.match(devIntegration, /permissions:\n  actions: write/);
  assert.match(devIntegration, /Continue the release train after an explicitly dispatched integration/);
  assert.match(devIntegration, /if: github\.event_name == 'workflow_dispatch'/);
  assert.match(devIntegration, /gh workflow run promote-dev\.yml --repo "\$GITHUB_REPOSITORY" --ref main/);
  assert.match(devIntegration, /-f integrated_sha="\$INTEGRATED_SHA"/);
  const finalizer = readFileSync(".github/workflows/finalize-release.yml", "utf8");
  assert.match(finalizer, /types: \[completed\]/);
  assert.match(finalizer, /gh pr merge "\$pr_url" --merge --match-head-commit "\$HEAD_SHA"/);
  assert.match(finalizer, /test "\$\(git rev-parse "\$merge_sha\^2"\)" = "\$HEAD_SHA"/);
  assert.match(finalizer, /gh pr merge "\$pr_url" --squash --match-head-commit "\$HEAD_SHA"/);
  assert.match(finalizer, /git merge-tree --write-tree "\$base_sha" "\$HEAD_SHA"/);
  assert.match(finalizer, /if test "\$expected_tree" = "\$base_tree"/);
  assert.match(finalizer, /Development already contains the accepted release tree/);
  assert.match(finalizer, /if: steps\.merge\.outputs\.changed == 'true'/);
  assert.match(finalizer, /test "\$\(git rev-parse "\$merge_sha\^\{tree\}"\)" = "\$expected_tree"/);
  assert.match(finalizer, /gh workflow run tour-and-creche-pages --ref main/);
  assert.match(finalizer, /gh workflow run sync-release-to-dev\.yml --ref main/);
  assert.match(finalizer, /gh workflow run dev-integration\.yml --ref dev/);
  const monitor = readFileSync(".github/workflows/monitor-trusted-pr.yml", "utf8");
  assert.match(request, /gh workflow run monitor-trusted-pr\.yml --ref main/);
  assert.doesNotMatch(sync, /gh workflow run monitor-trusted-pr\.yml --ref main/);
  assert.match(request, /permissions:\n  actions: write\n  contents: write/);
  assert.match(sync, /permissions:\n  actions: write\n  contents: write/);
  assert.match(monitor, /types?: choice/);
  assert.match(monitor, /options: \[release, sync\]/);
  assert.match(monitor, /runs-on: ubuntu-24\.04/);
  assert.match(monitor, /timeout-minutes: 48/);
  assert.match(monitor, /if: inputs\.kind == 'sync'/);
  assert.match(monitor, /checks: write/);
  assert.match(monitor, /GITHUB_EVENT_PATH="\$event_path" node tools\/ci\/pr-closing-intent\.mjs/);
  assert.match(monitor, /cargo test --locked --package conduit-xtask-dispatch/);
  assert.match(monitor, /proof\/ci\/tour-compatibility\.test\.mjs/);
  assert.match(monitor, /proof\/ci\/release-lane-controller\.spec\.mjs/);
  assert.match(monitor, /repos\/\$GITHUB_REPOSITORY\/check-runs/);
  assert.match(monitor, /-f name=candidate/);
  assert.match(monitor, /-f head_sha="\$HEAD_SHA"/);
  assert.match(monitor, /-f conclusion=success/);
  assert.match(monitor, /actions\/runs\/\$run_id\/approve/);
  assert.match(monitor, /actions\/runs\/\$run_id\/jobs/);
  assert.match(monitor, /gh run cancel "\$run_id"/);
  assert.match(monitor, /conduit-release-cancelled-bad/);
  assert.match(monitor, /test "\$conclusion" = cancelled/);
  assert.match(monitor, /release-lane reconciliation owns its terminal classification/);
  assert.match(monitor, /test \$\(\(now - last_progress\)\) -ge 900/);
  assert.match(monitor, /gh workflow run release-lane\.yml --ref main/);
  assert.match(monitor, /group: monitor-trusted-pr-\$\{\{ inputs\.kind \}\}-\$\{\{ inputs\.pr_number \}\}\n  cancel-in-progress: false/);
  assert.match(monitor, /comment_body=\$\(printf/);
  assert.doesNotMatch(monitor, /^Release attempt /m);
  assert.match(monitor, /sleep 30/);
  assert.match(monitor, /gh pr merge "\$pr_url" --merge --match-head-commit "\$HEAD_SHA"/);
  assert.match(monitor, /gh pr merge "\$pr_url" --squash --match-head-commit "\$HEAD_SHA"/);
  assert.match(monitor, /git merge-tree --write-tree "\$base_sha" "\$HEAD_SHA"/);
  assert.match(monitor, /test "\$\(git rev-parse "\$merge_sha\^\{tree\}"\)" = "\$expected_tree"/);
  assert.match(monitor, /gh workflow run tour-and-creche-pages --ref main/);
  assert.match(monitor, /gh workflow run dev-integration\.yml --ref dev/);
  const releaseLane = readFileSync(".github/workflows/release-lane.yml", "utf8");
  assert.match(releaseLane, /workflows: \[promotion\]/);
  assert.match(releaseLane, /types: \[requested, in_progress, completed\]/);
  assert.match(releaseLane, /cron: "\*\/5 \* \* \* \*"/);
  assert.match(releaseLane, /actions: write/);
  assert.match(releaseLane, /issues: write/);
  assert.match(releaseLane, /group: release-lane-controller/);
  assert.match(releaseLane, /cancel-in-progress: false/);
  assert.match(releaseLane, /node tools\/ci\/release-lane-github\.mjs --apply/);
  const approval = readFileSync(".github/workflows/approve-release-automation.yml", "utf8");
  assert.match(approval, /workflows: \[promotion, candidate\]/);
  assert.match(approval, /types: \[requested, completed\]/);
  assert.match(approval, /workflow_run\.conclusion == 'action_required'/);
  assert.match(approval, /actor\.login == 'github-actions\[bot\]'/);
  assert.doesNotMatch(approval, /workflow_run\.head_branch/);
  assert.match(approval, /actions\/runs\/\$RUN_ID/);
  assert.match(approval, /\.pull_requests \| length/);
  assert.match(approval, /\.head\.repo\.full_name/);
  assert.match(approval, /\.head\.sha.*\.head_sha/s);
  assert.match(approval, /release\/\*:main\|sync-release\/\*:dev/);
  assert.match(approval, /actions\/runs\/\$RUN_ID\/approve/);
  for (const retired of [
    "candidate-shared-compile.yml",
    "reconcile-candidate.yml",
    "retire-merged-pr-branch.yml",
    "retire-superseded-candidates.yml",
  ]) {
    assert.equal(existsSync(`.github/workflows/${retired}`), false, retired);
  }
});

test("artifact transport gets one bounded retry without hiding repeated failure", () => {
  for (const path of [
    ".github/workflows/check.yml",
    ".github/workflows/promotion.yml",
    ".github/workflows/tour-products.yml",
  ]) {
    const workflow = readFileSync(path, "utf8");
    assert.doesNotMatch(workflow, /uses: actions\/upload-artifact@v7/);
    assert.doesNotMatch(workflow, /uses: actions\/download-artifact@v8/);
  }
  const upload = readFileSync(".github/actions/upload-artifact-retry/action.yml", "utf8");
  const download = readFileSync(".github/actions/download-artifact-retry/action.yml", "utf8");
  assert.equal((upload.match(/uses: actions\/upload-artifact@v7/g) ?? []).length, 2);
  assert.equal((download.match(/uses: actions\/download-artifact@v8/g) ?? []).length, 2);
  assert.match(upload, /continue-on-error: true/);
  assert.match(upload, /if: steps\.primary\.outcome == 'failure'/);
  assert.match(upload, /overwrite: true/);
  assert.match(download, /continue-on-error: true/);
  assert.match(download, /if: steps\.primary\.outcome == 'failure'/);
});

test("cancelled exact heads cannot start more reusable proof jobs", () => {
  for (const path of [".github/workflows/check.yml", ".github/workflows/tour-products.yml"]) {
    const workflow = readFileSync(path, "utf8");
    assert.doesNotMatch(
      workflow,
      /^    if: (?:\$\{\{ )?always\(\) && (?!\!cancelled\(\))/m,
      `${path} has a cancellation-resistant job condition`,
    );
    assert.doesNotMatch(
      workflow,
      /^      always\(\) && (?!\!cancelled\(\))/m,
      `${path} has a cancellation-resistant multiline job condition`,
    );
  }
});

test("documentation-only promotion prepares proofs while ordinary documentation stays cheap", () => {
  const classifier = resolve("tools/ci/classify-docs-only.sh");
  const directory = mkdtempSync(join(tmpdir(), "conduit-promotion-classify-"));
  const git = (...args) => execFileSync("git", args, { cwd: directory, encoding: "utf8" }).trim();
  try {
    git("init", "--quiet");
    git("config", "user.name", "Conduit proof");
    git("config", "user.email", "proof@example.invalid");
    writeFileSync(join(directory, "README.md"), "before\n");
    git("add", "README.md");
    git("commit", "--quiet", "-m", "base");
    const base = git("rev-parse", "HEAD");
    writeFileSync(join(directory, "README.md"), "after\n");
    git("commit", "--quiet", "-am", "documentation");
    const head = git("rev-parse", "HEAD");
    for (const full of ["false", "true"]) {
      const output = execFileSync("bash", [classifier, base, head], {
        cwd: directory, encoding: "utf8",
        env: { ...process.env, CONDUIT_FULL_SUITE: full },
      });
      const fields = Object.fromEntries(output.trim().split("\n").map((line) => line.split("=")));
      assert.equal(fields.docs_only, full === "true" ? "false" : "true");
      assert.equal(fields.reason, full === "true" ? "full-suite" : "all-markdown");
      assert.equal(fields.base_sha, base);
      assert.equal(fields.head_sha, head);
      assert.equal(fields.comparison_base_sha, base);
    }
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
