import test from "node:test";
import assert from "node:assert/strict";
import { prepareInstalledHostEncounter } from "./installed-host-encounter.mjs";

test("independent preparations both start before either completes", async () => {
  const started = [];
  let hostReady, browserReady;
  const host = { close: () => assert.fail("successful Host must remain owned") };
  const pending = prepareInstalledHostEncounter(
    () => { started.push("host"); return new Promise(resolve => { hostReady = resolve; }); },
    () => { started.push("browser"); return new Promise(resolve => { browserReady = resolve; }); },
  );
  await Promise.resolve();
  assert.deepEqual(started, ["host", "browser"]);
  browserReady("actual browser"); hostReady(host);
  assert.deepEqual(await pending, { installed: host, browser: "actual browser" });
});

test("browser refusal waits for and retires the Host without losing the cause", async () => {
  const failure = new Error("browser refused");
  let hostReady, closed = 0;
  const pending = prepareInstalledHostEncounter(
    () => new Promise(resolve => { hostReady = resolve; }),
    () => { throw failure; },
  );
  await Promise.resolve();
  assert.equal(closed, 0);
  hostReady({ close: async () => { closed++; } });
  await assert.rejects(pending, error => error === failure);
  assert.equal(closed, 1);
});

test("Host refusal cannot return browser readiness as successful admission", async () => {
  const failure = new Error("Host refused");
  await assert.rejects(prepareInstalledHostEncounter(
    () => { throw failure; }, () => "browser ready",
  ), error => error === failure);
});

test("cleanup refusal retains both the browser and cleanup failures", async () => {
  const browser = new Error("browser refused"), cleanup = new Error("cleanup refused");
  await assert.rejects(prepareInstalledHostEncounter(
    () => ({ close: () => { throw cleanup; } }), () => { throw browser; },
  ), error => error instanceof AggregateError && error.errors[0] === browser && error.errors[1] === cleanup);
});

test("independent preparation refusals retain both causes", async () => {
  const host = new Error("Host refused"), browser = new Error("browser refused");
  await assert.rejects(prepareInstalledHostEncounter(
    () => { throw host; }, () => { throw browser; },
  ), error => error instanceof AggregateError && error.errors[0] === host && error.errors[1] === browser);
});
