import assert from "node:assert/strict";
import { test } from "node:test";
import { FAMILIES, planChanges } from "../../tools/ci/pipeline/plan.mjs";

const all = { docsOnly: false, families: [...FAMILIES] };

test("only recognized prose can omit target lanes", () => {
  assert.deepEqual(planChanges(["README.md", "STATUS.md", "docs/contributing/ci.md"]), {
    docsOnly: true, families: [],
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
    ["targets/std/src/lib.rs", ["hosted"]],
    ["targets/conduitos/kernel/src/lib.rs", ["conduitos", "orange-pi"]],
    ...["esp32", "avr", "raspberry-pi", "orange-pi", "rp2040"]
      .map((family) => [`targets/${family}/src/lib.rs`, [family]]),
  ]) assert.deepEqual(planChanges([path]), { docsOnly: false, families }, path);
});

test("renamed paths select both old and new owners in stable order", () => {
  assert.deepEqual(planChanges([
    "targets/esp32/old.rs", "targets/browser/new.rs", "targets/esp32/other.rs", "docs/ci.md",
  ]), { docsOnly: false, families: ["browser", "esp32"] });
  assert.deepEqual(planChanges(["architecture/old.rs", "targets/browser/new.rs"]), all);
});

test("full integration always selects every family, even for prose", () => {
  assert.deepEqual(planChanges(["README.md"], { full: true }), all);
  assert.deepEqual(planChanges(["targets/browser/src/lib.rs"], { full: true }), all);
});

test("malformed path collections cannot silently become docs-only", () => {
  assert.throws(() => planChanges("README.md"), TypeError);
  assert.throws(() => planChanges([null]), TypeError);
});
