import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync, mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { requestRelease } from "../../tools/ci/release-request.mjs";

const repository = "owner/repo", sha = "a".repeat(40), main = "b".repeat(40);
const integration = (id = 1, head = sha, conclusion = "success") => ({
  id, head_sha: head, head_branch: "dev", head_repository: { full_name: repository },
  event: "push", status: "completed", conclusion, created_at: new Date(id * 1000).toISOString(),
});
const release = (number = 1) => ({ number, base: { ref: "main" },
  head: { ref: `release/${sha}`, sha, repo: { full_name: repository } }, html_url: `https://example.test/${number}` });

function fixture() {
  const state = { pulls: [], runs: [integration()], history: [], writes: [],
    candidateTree: "candidate-tree", mainTree: "main-tree", merged: "candidate-tree",
    main, onDev: true, onMain: false, openReads: 0 };
  const deps = {
    repository,
    api: async (path, method = "GET", value) => {
      if (method !== "GET") {
        state.writes.push({ path, method, value });
        if (path === "/pulls") {
          const pull = release(10);
          pull.head.ref = value.head; pull.head.sha = value.head.slice("release/".length);
          state.pulls.push(pull);
          return pull;
        }
        return null;
      }
      if (path.startsWith("/pulls?state=open")) {
        state.openReads++;
        state.beforeOpenRead?.();
        return state.pulls;
      }
      if (path.startsWith("/pulls?state=closed")) return state.history;
      if (path.startsWith("/actions/workflows/dev-integration.yml/runs?")) return { workflow_runs: state.runs };
      throw new Error(`unexpected API call ${path}`);
    },
    git: (...args) => {
      if (args[0] === "fetch") { state.onFetch?.(args); return ""; }
      if (args[0] === "push") { state.writes.push({ push: args }); return ""; }
      if (args.join(" ") === "rev-parse origin/main") return state.main;
      if (args[0] === "rev-parse") return args[1] === `${main}^{tree}` ? state.mainTree : state.candidateTree;
      throw new Error(`unexpected git call ${args}`);
    },
    isAncestor: (_base, head) => head === "origin/dev" ? state.onDev : state.onMain,
    mergedTree: () => state.merged,
  };
  return { state, deps, request: () => requestRelease(deps) };
}

test("rapid dev updates never create successors behind a running, failed, or repaired release", async () => {
  const f = fixture();
  assert.equal((await f.request()).status, "created");
  for (const status of ["running", "failed", "cancelled-bad", "repairing", "passed-awaiting-merge"]) {
    f.state.pulls[0].promotionStatus = status;
    for (let id = 2; id < 42; id++) {
      f.state.runs = [integration(id, id.toString(16).padStart(40, "0"))];
      assert.equal((await f.request()).status, "release-open");
    }
  }
  assert.equal(f.state.writes.filter(write => write.path === "/pulls").length, 1);
  assert.equal(f.state.writes.filter(write => write.push).length, 1);
});

test("after merge and sync, one fresh batch starts from newest successful integration", async () => {
  const f = fixture();
  f.state.pulls = [{ ...release(), base: { ref: "dev" }, head: { ...release().head, ref: `sync-release/${main}` } }];
  assert.equal((await f.request()).status, "sync-pending");
  assert.deepEqual(f.state.writes, []);
  f.state.pulls = [];
  const latest = "c".repeat(40);
  f.state.runs = [integration(), integration(3, latest), integration(2)];
  assert.equal((await f.request()).headSha, latest);
  assert.equal((await f.request()).status, "release-open");
});

test("newer running development cannot starve a successfully integrated batch", async () => {
  const f = fixture();
  f.state.runs.push({ ...integration(2, "c".repeat(40)), status: "in_progress", conclusion: null });
  assert.equal((await f.request()).headSha, sha);
});

test("manual requests require successful proof and cannot revive stale snapshots", async () => {
  const f = fixture();
  f.deps.integratedSha = "d".repeat(40);
  assert.equal((await f.request()).status, "superseded-integration");
  f.deps.integratedSha = "";
  f.state.runs = [];
  assert.equal((await f.request()).status, "integration-required");
  f.state.runs = [integration(), integration(2, sha, "failure")];
  assert.equal((await f.request()).status, "integration-required");
  assert.deepEqual(f.state.writes, []);
});

test("closed failed release history does not become an automatic rerun loop", async () => {
  const f = fixture();
  f.state.history = [release()];
  assert.equal((await f.request()).status, "previous-release-requires-review");
  assert.deepEqual(f.state.writes, []);
});

test("already accepted, divergent, and release-fix-dropping candidates do not publish", async () => {
  for (const [change, status] of [
    [{ onMain: true }, "already-current"],
    [{ mainTree: "candidate-tree" }, "already-current"],
    [{ onDev: false }, "superseded-integration"],
    [{ merged: "tree-with-release-fix" }, "release-fixes-not-integrated"],
    [{ merged: null }, "release-fixes-not-integrated"],
  ]) {
    const f = fixture(); Object.assign(f.state, change);
    assert.equal((await f.request()).status, status);
    assert.deepEqual(f.state.writes, []);
  }
});

test("admission rechecks ownership and main immediately before publication", async () => {
  const f = fixture();
  f.state.beforeOpenRead = () => { if (f.state.openReads === 2) f.state.pulls = [release()]; };
  assert.equal((await f.request()).status, "release-open");
  assert.deepEqual(f.state.writes, []);
  const moved = fixture();
  moved.state.onFetch = args => { if (args.length === 4) moved.state.main = "c".repeat(40); };
  assert.equal((await moved.request()).status, "main-moved");
  assert.deepEqual(moved.state.writes, []);
});

test("integration covers a skipped intermediate change even when the newest push is docs-only", () => {
  const directory = mkdtempSync(join(tmpdir(), "conduit-integration-baseline-"));
  const git = (...args) => execFileSync("git", args, { cwd: directory, encoding: "utf8" }).trim();
  try {
    git("init", "-q"); git("config", "user.name", "Test"); git("config", "user.email", "test@example.test");
    writeFileSync(join(directory, "base"), "base"); git("add", "."); git("commit", "-qm", "accepted");
    const accepted = git("rev-parse", "HEAD");
    writeFileSync(join(directory, "runtime.rs"), "changed runtime"); git("add", "."); git("commit", "-qm", "coalesced update");
    const before = git("rev-parse", "HEAD");
    writeFileSync(join(directory, "notes.md"), "docs"); git("add", "."); git("commit", "-qm", "latest update");
    assert.equal(git("diff", "--name-only", before, "HEAD"), "notes.md");
    const base = git("merge-base", accepted, "HEAD");
    assert.deepEqual(git("diff", "--name-only", base, "HEAD").split("\n"), ["notes.md", "runtime.rs"]);
  } finally { rmSync(directory, { recursive: true, force: true }); }
  const workflow = readFileSync(".github/workflows/dev-integration.yml", "utf8");
  assert.match(workflow, /git merge-base origin\/main HEAD/);
  assert.equal(workflow.match(/base_sha: \$\{\{ needs.baseline.outputs.base_sha \}\}/g)?.length, 2);
  assert.match(workflow, /test "\$INPUT_SHA" = "\$EVENT_SHA"/);
});

test("release admission has one serialized trusted entry and a lost-wakeup recovery", () => {
  const workflow = readFileSync(".github/workflows/promote-dev.yml", "utf8");
  const adapter = readFileSync("tools/ci/release-request-github.mjs", "utf8");
  assert.match(workflow, /group: promote-dev-to-main\n  cancel-in-progress: false/);
  assert.match(workflow, /cron: "\*\/10 \* \* \* \*"/);
  assert.match(workflow, /ref: main/);
  assert.match(workflow, /if test ! -f tools\/ci\/release-request-github\.mjs/);
  assert.match(workflow, /Deferred until the trusted main controller contains release-request-github\.mjs/);
  assert.match(workflow, /node tools\/ci\/release-request-github.mjs/);
  assert.doesNotMatch(workflow, /gh pr create|git push|gh run rerun/);
  assert.match(adapter, /path\.startsWith\("\/actions\/workflows\/dev-integration\.yml\/runs\?"\)/);
  assert.match(adapter, /--jq/);
  for (const field of ["id", "head_sha", "head_branch", "head_repository", "event", "status", "conclusion", "created_at"]) {
    assert.match(adapter, new RegExp(`\\b${field}\\b`));
  }
});
