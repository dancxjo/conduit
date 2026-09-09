import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import test from "node:test";

const workflow = readFileSync(".github/workflows/tour-products.yml", "utf8");
const gate = workflow.split("  products-proof:\n")[1];
const script = gate.split("        run: |\n")[1]
  .split("\n").map(line => line.replace(/^          /, "")).join("\n");
const env = {
  ...process.env, PRODUCT_REQUIRED: "true", TOUR_REQUIRED: "true",
  DEBUGGER_REQUIRED: "false", TOUR_PATCHBAY_RESULT: "success",
  CARRIER_REQUIRED: "false", RECEIPTS_RESULT: "skipped",
};
const run = overrides => spawnSync("bash", ["-e", "-c", script], {
  env: { ...env, ...overrides }, encoding: "utf8",
}).status;

test("development requires executed proof but never certifies a partial Tour as complete", () => {
  const receipts = workflow.split("  proof-receipts:\n")[1].split("    runs-on:")[0];
  assert.match(receipts, /!inputs\.development_admission/);
  assert.equal(run({ DEVELOPMENT_ADMISSION: "true" }), 0);
  assert.notEqual(run({ DEVELOPMENT_ADMISSION: "true", TOUR_PATCHBAY_RESULT: "failure" }), 0);
  assert.notEqual(run({ DEVELOPMENT_ADMISSION: "false" }), 0);
  assert.equal(run({ DEVELOPMENT_ADMISSION: "false", RECEIPTS_RESULT: "success" }), 0);
  assert.notEqual(run({ DEVELOPMENT_ADMISSION: "false", RECEIPTS_RESULT: "success",
    CARRIER_REQUIRED: "true", STAGE_RESULT: "failure" }), 0);
});
