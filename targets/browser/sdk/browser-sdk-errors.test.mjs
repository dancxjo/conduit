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

test("BrowserHost cannot be fabricated outside the admitted Conduit entrance", async () => {
  const { BrowserHost } = await import("./browser-sdk.mjs");
  assert.throws(() => new BrowserHost(), /come from Conduit\.browser/);
});
