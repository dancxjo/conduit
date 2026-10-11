import assert from "node:assert/strict";
import test from "node:test";
import {
  ConduitSdkError,
  IncompatibleRuntimeAbiError,
  InvalidLifecycleError,
  PermissionDeniedError,
  PlanRefusalError,
  ResourceLossError,
} from "./browser-sdk.mjs";

test("typed lifecycle refusals preserve their category, operation, evidence, and exact identities", () => {
  const evidence = Object.freeze({ schema: "conduit.body/wake-refusal@1", reason: "resource pressure" });
  const identities = Object.freeze({ hostId: "host/one", bootId: "boot/two", bodyId: "body/three", planId: "plan/four" });
  const types = [
    [PlanRefusalError, "PlanRefusal"],
    [ResourceLossError, "ResourceLoss"],
    [InvalidLifecycleError, "InvalidLifecycle"],
    [PermissionDeniedError, "PermissionDenied"],
    [IncompatibleRuntimeAbiError, "IncompatibleRuntimeAbi"],
  ];
  for (const [Type, category] of types) {
    const error = new Type({ code: "exact-runtime-code", message: "refused", operation: "Body.wake", evidence, identities });
    assert(error instanceof ConduitSdkError);
    assert.equal(error.category, category);
    assert.equal(error.operation, "Body.wake");
    assert.equal(error.evidence, evidence);
    assert.deepEqual(error.identities, identities);
    assert(Object.isFrozen(error.identities));
  }
});

test("BrowserHost cannot be made outside the admitted Conduit entrance", async () => {
  const { BrowserHost, BrowserPlay } = await import("./browser-sdk.mjs");
  assert.throws(() => new BrowserHost(), /come from Conduit\.browser/);
  assert.throws(() => new BrowserPlay(), /admitted runtime start/);
});

test("external Body executor accepts the Thermostat registry within its exact finite envelope", async () => {
  const { checkedBrowserExecutionCapabilities } = await import("./browser-sdk.mjs");
  const installed = Array.from({ length: 117 }, (_, index) => `capability/installed-${index}`);
  installed.push("browser-thermostat-combine-v1", "browser/thermostat-scan@1");
  const disclosure = { schema: "conduit.browser/body-capabilities@1", capability_ids: installed };
  const accepted = checkedBrowserExecutionCapabilities(disclosure);
  assert.deepEqual(accepted, installed);
  assert(Object.isFrozen(accepted));
  assert.notEqual(accepted, installed);
  assert.equal(checkedBrowserExecutionCapabilities({ ...disclosure,
    capability_ids: [...installed, "capability/final-admitted-slot"] }).length, 120);
  for (const invalid of [
    { ...disclosure, capability_ids: [...installed, "capability/120", "capability/121"] },
    { ...disclosure, schema: "wrong" },
    { ...disclosure, capability_ids: {} },
    ...[null, "", "x".repeat(257)].map(identity => ({ ...disclosure,
      capability_ids: [...installed.slice(0, -1), identity] })),
  ]) {
    assert.throws(() => checkedBrowserExecutionCapabilities(invalid),
      /invalid external Body execution capabilities/);
  }
});
