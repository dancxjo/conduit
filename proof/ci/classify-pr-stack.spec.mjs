import assert from "node:assert/strict";
import { readFileSync, mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

import { classifyPullStack, resolvePullStack } from "../../tools/ci/classify-pr-stack.mjs";

const repository = "dancxjo/conduit";
const sha = (character) => character.repeat(40);
const pull = (number, head, base, owner = "victor", headSha = sha(String(number % 10))) => ({
  number,
  state: "open",
  user: { login: owner },
  head: { ref: head, sha: headSha, repo: { full_name: repository } },
  base: { ref: base, repo: { full_name: repository } },
});

const five = [
  pull(10, "victor/slice-1", "main", "victor", sha("a")),
  pull(11, "victor/slice-2", "victor/slice-1", "victor", sha("b")),
  pull(12, "victor/slice-3", "victor/slice-2", "victor", sha("c")),
  pull(13, "victor/slice-4", "victor/slice-3", "victor", sha("d")),
  pull(14, "victor/slice-5", "victor/slice-4", "victor", sha("e")),
];

test("five dependent PRs schedule expensive proof only for the immutable cumulative tip", () => {
  const roles = five.map((candidate) => classifyPullStack(five, candidate.number, candidate.head.sha, repository));
  assert.deepEqual(roles.map(({ role }) => role), ["intermediate", "intermediate", "intermediate", "intermediate", "tip"]);
  assert.equal(roles.filter(({ role }) => role === "tip").length, 1);
  for (const role of roles) {
    assert.deepEqual(role.pull_numbers, [10, 11, 12, 13, 14]);
    assert.equal(role.tip_pr, 14);
    assert.equal(role.tip_sha, sha("e"));
  }
});

test("an advanced tip changes only the exact cumulative candidate", () => {
  const advanced = structuredClone(five);
  advanced.at(-1).head.sha = sha("f");
  const prior = classifyPullStack(five, 14, sha("e"), repository);
  const next = classifyPullStack(advanced, 14, sha("f"), repository);
  assert.equal(prior.tip_sha, sha("e"));
  assert.equal(next.tip_sha, sha("f"));
  assert.deepEqual(next.pull_numbers, prior.pull_numbers);
});

test("Victor's 2582 to 2584 continuation collapses to the later immutable tip", () => {
  const continuation = [
    pull(2582, "victor/2292-patchbay-adopt-offer-evidence", "victor/2292-browser-admitted-offer-evidence", "victor", "6119e6794bb40c53b1c7398c1315f13aa9b8a7b2"),
    pull(2584, "victor/2292-request-browser-offer-detail", "victor/2292-patchbay-adopt-offer-evidence", "victor", "1f8247d68632edd79021e5bb4d13d0bcd15144c5"),
  ];
  const slice = classifyPullStack(continuation, 2582, continuation[0].head.sha, repository);
  const tip = classifyPullStack(continuation, 2584, continuation[1].head.sha, repository);
  assert.equal(slice.role, "intermediate");
  assert.equal(slice.tip_pr, 2584);
  assert.equal(tip.role, "tip");
  assert.deepEqual(tip.members, continuation.map((candidate) => ({
    number: candidate.number,
    head_ref: candidate.head.ref,
    head_sha: candidate.head.sha,
  })));
});

test("siblings, owner boundaries, forks, and stale event heads fail closed to independent proof", () => {
  const siblings = [...five.slice(0, 2), pull(15, "victor/sibling", "victor/slice-1", "victor", sha("f"))];
  assert.deepEqual(classifyPullStack(siblings, 10, sha("a"), repository), {
    role: "independent", reason: "conflicting-siblings", owner: "victor",
    root_pr: 10, tip_pr: 10, tip_sha: sha("a"), pull_numbers: [10],
    members: [{ number: 10, head_ref: "victor/slice-1", head_sha: sha("a") }], candidate_sha: sha("a"),
  });
  const otherOwner = [five[0], pull(11, "other/slice", "victor/slice-1", "other", sha("b"))];
  assert.equal(classifyPullStack(otherOwner, 10, sha("a"), repository).reason, "multiple-owners");
  const fork = structuredClone(five[0]);
  fork.head.repo.full_name = "fork/conduit";
  assert.throws(() => classifyPullStack([fork], 10, sha("a"), repository), /exact open repository candidate/);
  assert.throws(() => classifyPullStack(five, 14, sha("9"), repository), /exact open repository candidate/);
});

test("API pagination retains exact PR, author, branch, and candidate provenance", async () => {
  const requests = [];
  const role = await resolvePullStack({
    repository,
    pullNumber: 12,
    candidateSha: sha("c"),
    token: "test",
    request: async (url) => {
      requests.push(url);
      return { ok: true, json: async () => five };
    },
  });
  assert.equal(role.role, "intermediate");
  assert.equal(role.owner, "victor");
  assert.equal(role.candidate_sha, sha("c"));
  assert.deepEqual(role.pull_numbers, [10, 11, 12, 13, 14]);
  assert.equal(requests.length, 1);
});

test("unknown API truth fails closed instead of guessing that a slice is intermediate", async () => {
  await assert.rejects(() => resolvePullStack({
    repository,
    pullNumber: 12,
    candidateSha: sha("c"),
    token: "test",
    request: async () => ({ ok: false, status: 403 }),
  }), /HTTP 403/);
});

test("candidate workflows retain only cheap gates for an intermediate stack slice", () => {
  const check = readFileSync(".github/workflows/check.yml", "utf8");
  const products = readFileSync(".github/workflows/tour-products.yml", "utf8");
  for (const workflow of [check, products]) {
    assert.match(workflow, /Classify this immutable candidate within its open PR stack/);
    assert.ok(workflow.includes('git -C "$RUNNER_TEMP/conduit-ci-controller" ls-files -- \'*/classify-pr-stack.mjs\''));
    assert.ok(workflow.includes('test "${#classifiers[@]}" -ne 1'));
    assert.match(workflow, /steps\.stack\.outputs\.role == 'intermediate'/);
    assert.match(workflow, /steps\.stack\.outputs\.role != 'intermediate'/);
  }
  assert.match(check, /reason=stacked-intermediate-slice/);
  assert.match(check, /docs_only=true/);
  assert.match(products, /no product fabrication is scheduled/);
  assert.match(products, /echo 'required=false'/);
});


test("workflow classifier lookup follows only one tracked trusted implementation", (t) => {
  const workflow = readFileSync(".github/workflows/candidate-shared-compile.yml", "utf8");
  const script = workflow.match(/          mapfile -t classifiers[\s\S]*?          node "\$RUNNER_TEMP\/conduit-ci-controller\/\$\{classifiers\[0\]\}"/)[0];
  for (const name of ["check.yml", "tour-products.yml"]) {
    assert.ok(readFileSync(`.github/workflows/${name}`, "utf8").includes(script));
  }
  for (const owner of ["scripts/ci", "tools/ci"]) {
    const temporary = mkdtempSync(join(tmpdir(), "conduit-controller-owner-"));
    t.after(() => rmSync(temporary, { recursive: true, force: true }));
    const controller = join(temporary, "conduit-ci-controller");
    mkdirSync(controller);
    assert.equal(spawnSync("git", ["init", "-q", controller]).status, 0);
    const selected = `${owner}/classify-pr-stack.mjs`;
    mkdirSync(dirname(join(controller, selected)), { recursive: true });
    writeFileSync(join(controller, selected), `console.log(${JSON.stringify(owner)});`);
    assert.equal(spawnSync("git", ["-C", controller, "add", selected]).status, 0);
    const untracked = "untracked/classify-pr-stack.mjs";
    mkdirSync(dirname(join(controller, untracked)));
    writeFileSync(join(controller, untracked), 'throw new Error("untracked source executed");');
    const run = () => spawnSync("bash", ["-e", "-c", script], {
      cwd: temporary, env: { ...process.env, RUNNER_TEMP: temporary }, encoding: "utf8",
    });
    const exact = run();
    assert.equal(exact.status, 0, exact.stderr);
    assert.equal(exact.stdout.trim(), owner);
    assert.equal(spawnSync("git", ["-C", controller, "add", untracked]).status, 0);
    const ambiguous = run();
    assert.notEqual(ambiguous.status, 0);
    assert.match(ambiguous.stderr, /Expected one tracked stack classifier/);
    assert.equal(ambiguous.stdout, "");
  }
});
