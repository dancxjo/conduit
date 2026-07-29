import { expect, test } from "@playwright/test";

test("executes browser-host vectors without engine-name branches", async ({ page }) => {
  const failures = [];
  page.on("pageerror", (error) => failures.push(error.stack ?? String(error)));
  page.on("console", (message) => {
    if (message.type() === "error") failures.push(message.text());
  });

  await page.goto("/browser/conduit-browser-host.test.html");
  await expect(page.locator("#result")).toHaveText("ok", { timeout: 20_000 });
  const result = await page.evaluate(() => globalThis.__conduitBrowserResults);
  const fixture = await page.evaluate(async () =>
    fetch("/conformance/c5/browser-host-v1.json").then((response) => response.json()));
  const fixtureCase = (id) =>
    fixture.cases.find((entry) => entry.id === id)?.expected;

  expect(failures).toEqual([]);
  expect(result.statusBound).toBe(true);
  expect(result.presentationIdentitySeparate).toBe(true);
  expect(result.unsupportedExplicit).toBe(true);
  expect(result.uaBranch).toBe(false);
  expect(result.resolverEffects).toEqual({
    prompted: false,
    fetched: false,
    mutated: false,
  });
  expect(result.terminalEvidence).toBeGreaterThanOrEqual(2);
  expect(result.membershipSignals).toHaveLength(5);
  expect(result.membershipSignals.every((decision) =>
    decision.ok === false && decision.code === "CND-GEN-005")).toBe(true);
  expect(result.supervision).toEqual({
    restartAttempt: 2,
    restartAffectedScope: "observed-subject",
    restartNotBeforeTick: 3,
    evidenceKinds: [
      "terminal-observed",
      "observation-admitted",
      "decision-accepted",
      "attempt-started",
    ],
    constrainedCode: "CND-SUP-015",
    correlationCode: "CND-SUP-003",
    correlationEvidenceReason: "CND-SUP-003",
    handlerTimeoutCode: "CND-SUP-008",
    handlerTimeoutSubject: "root/handler",
  });
  expect(result.pool).toEqual({
    live: 2,
    queued: 0,
    restarting: 0,
    retiring: 1,
    queueFullCode: "CND-POL-005",
    queueFullReason: "queue-full",
    restartAttempt: 2,
    evidenceCount: 15,
    evidenceBound: 64,
    generationDrainAffected: 3,
    evidenceExhaustionCode: "CND-POL-006",
  });
  expect(result.placementOutcomes.window).toBe("executed");
  expect(result.placementOutcomes["dedicated-worker"]).toBe("executed");
  expect(result.placementOutcomes.wasm).toBe("executed");
  expect(fixtureCase("concrete-window-worker-service-worklet-wasm-gpu-adapters")).toEqual({
    executed_or_explicit_unsupported: true,
  });
  expect(fixtureCase("chromium-firefox-webkit-run-identical-vectors")).toEqual({
    engines: 3,
    ua_branch: false,
  });

  for (const placement of [
    "shared-worker",
    "service-worker",
    "audio-worklet",
    "webgpu",
  ]) {
    expect(["executed", "unsupported"]).toContain(result.placementOutcomes[placement]);
  }
});
