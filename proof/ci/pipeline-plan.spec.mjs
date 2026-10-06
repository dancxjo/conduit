import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import { FAMILIES, UNIT_SHARDS, planChanges } from "../../tools/ci/pipeline/plan.mjs";

const all = { docsOnly: false, families: [...FAMILIES], unitShards: [...UNIT_SHARDS], conduitosProof: true };

test("only recognized prose can omit target lanes", () => {
  assert.deepEqual(planChanges(["README.md", "STATUS.md", "docs/contributing/ci.md"]), {
    docsOnly: true, families: [], unitShards: [], conduitosProof: false,
  });
  for (const path of ["docs/schema.json", "docs/index.html", "docs/config.yml", "wiki/example.md"]) {
    assert.deepEqual(planChanges([path]), all, path);
  }
});

test("unknown, empty, and shared changes select every target", () => {
  assert.deepEqual(planChanges([]), all);
  for (const path of [
    "new-target/code.rs", "architecture/planner/src/lib.rs", "semantics/time/src/lib.rs",
    "mechanisms/runtime/src/lib.rs", "make/packages.rs", "tools/xtask/src/main.rs",
    ".github/workflows/candidate.yml", "products/conduit/src/main.rs", "Cargo.toml",
    "targets/esp32/Cargo.lock", "proof/browser/package-lock.json", "",
  ]) assert.deepEqual(planChanges([path]), all, path);
});

test("target-local changes select the owning family", () => {
  for (const [path, families] of [
    ["targets/browser/src/lib.rs", ["browser"]],
    ["proof/browser/workspace.spec.ts", ["browser"]],
    ["site/index.html", ["browser"]],
    ["tools/xtask/src/evidence/one_body_journey.rs", ["browser"]],
    ["tools/xtask/src/evidence/one_body_journey/render.rs", ["browser"]],
    ["targets/std/src/lib.rs", ["browser", "hosted"]],
    ["targets/conduitos/kernel/src/lib.rs", ["conduitos", "orange-pi"]],
    ...["esp32", "avr", "raspberry-pi", "orange-pi", "rp2040"]
      .map((family) => [`targets/${family}/src/lib.rs`, [family]]),
  ]) assert.deepEqual(planChanges([path]), { docsOnly: false, families, conduitosProof: !/^(proof\/browser|site)\//.test(path), unitShards: /^(proof\/browser|site)\//.test(path)
    ? ["hosts-browser", "hosts-workbench", "products", "lint"] : [...UNIT_SHARDS] }, path);
});

test("other xtask evidence changes retain the full target matrix", () => {
  for (const path of [
    "tools/xtask/src/evidence.rs",
    "tools/xtask/src/evidence/gallery.rs",
    "tools/xtask/src/evidence/one_body_journey_extra.rs",
  ]) assert.deepEqual(planChanges([path]), all, path);
});

test("renamed paths select both old and new owners in stable order", () => {
  assert.deepEqual(planChanges([
    "targets/esp32/old.rs", "targets/browser/new.rs", "targets/esp32/other.rs", "docs/ci.md",
  ]), { docsOnly: false, families: ["browser", "esp32"], unitShards: [...UNIT_SHARDS], conduitosProof: true });
  assert.deepEqual(planChanges(["architecture/old.rs", "targets/browser/new.rs"]), all);
});

test("full integration always selects every family, even for prose", () => {
  assert.deepEqual(planChanges(["README.md"], { full: true }), all);
  assert.deepEqual(planChanges(["targets/browser/src/lib.rs"], { full: true }), all);
  assert.deepEqual(planChanges(["tools/xtask/src/evidence/one_body_journey/render.rs"], { full: true }), all);
});

test("malformed path collections cannot silently become docs-only", () => {
  assert.throws(() => planChanges("README.md"), TypeError);
  assert.throws(() => planChanges([null]), TypeError);
});


test("shared target libraries retain every consuming platform proof", () => {
  for (const path of [
    "targets/browser/runtime/src/lib.rs",
    "targets/std/offers/src/lib.rs",
    "targets/std/make/src/lib.rs",
    "targets/browser/make/src/lib.rs",
    "targets/esp32/make/src/lib.rs",
    "targets/avr/make/src/lib.rs",
    "targets/raspberry-pi/make/src/lib.rs",
    "targets/rp2040/make/src/lib.rs",
    "targets/rp2040/network-realization/src/lib.rs",
    "targets/conduitos/make/xtask/mod.rs",
  ]) assert.deepEqual(planChanges([path]), all, path);
});

test("isolated firmware remains target-local despite shared target libraries", () => {
  for (const family of ["avr", "esp32", "rp2040"]) {
    assert.deepEqual(planChanges([`targets/${family}/firmware/device/src/main.rs`]), {
      docsOnly: false, families: [family], unitShards: [...UNIT_SHARDS], conduitosProof: true,
    });
  }
});


test("browser-only assets omit unrelated hosts but mixed and unknown changes fail closed", () => {
  const browser = ["hosts-browser", "hosts-workbench", "products", "lint"];
  for (const path of ["proof/browser/workspace.spec.mjs", "site/index.html"]) {
    assert.deepEqual(planChanges([path, "docs/ci.md"]).unitShards, browser);
    assert.equal(planChanges([path, "docs/ci.md"]).conduitosProof, false);
    assert.deepEqual(planChanges([path], { full: true }).unitShards, UNIT_SHARDS);
    for (const other of ["targets/std/src/lib.rs", "site/unknown.bin", "proof/browser/package.json", "Cargo.lock"]) {
      assert.deepEqual(planChanges([path, other]).unitShards, UNIT_SHARDS);
      assert.equal(planChanges([path, other]).conduitosProof, true);
    }
  }
  assert.deepEqual(planChanges(["site/old.html", "targets/std/new.rs"]).unitShards, UNIT_SHARDS);
});


test("Integration scanner emits the complete unit matrix consumed by Actions", () => {
  const sha = spawnSync("git", ["rev-parse", "HEAD"], { encoding: "utf8" });
  assert.equal(sha.status, 0);
  const result = spawnSync("node", ["tools/ci/pipeline/cli.mjs", "scan", "all", sha.stdout.trim()], {
    encoding: "utf8", env: { ...process.env, GITHUB_OUTPUT: "" },
  });
  assert.equal(result.status, 0, result.stderr);
  const line = result.stdout.split("\n").find(line => line.startsWith("unit-matrix="));
  assert.deepEqual(JSON.parse(line.slice("unit-matrix=".length)), { shard: [...UNIT_SHARDS] });
  assert.match(result.stdout, /docs-only=false/);
  assert.match(result.stdout, /conduitos-proof=true/);
});
