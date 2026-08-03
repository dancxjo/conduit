import { expect, test } from "@playwright/test";

const TEST_HOST_TOKEN = process.env.CONDUIT_TEST_HOST_TOKEN ??
  "conduit-playwright-task-action-host";
const TASK_ACTION_POLICY_OBSERVATION_ID = "conduit.task-policy/tour-browser-host";

async function waitForTourReady(page) {
  await expect(page.locator("html")).toHaveAttribute("data-tour-ready", "true", {
    timeout: 60_000,
  });
}

async function gotoTour(page, lesson, suffix = "") {
  await page.goto(`/tour/public/index.html?lesson=${lesson}${suffix}`);
  await waitForTourReady(page);
}

async function setHostTaskActionPolicy(page, state, options = {}) {
  const response = await page.request.post(
    `${new URL(page.url()).origin}/__conduit-test/task-action-policy`,
    {
      headers: { "X-Conduit-Test-Host-Token": TEST_HOST_TOKEN },
      data: {
        schemaVersion: 0,
        observationId: TASK_ACTION_POLICY_OBSERVATION_ID,
        generation: 2,
        action: "run-exact-plan",
        activeControls: ["cancel", "drain"],
        state,
        observedAtTick: 10,
        validUntilTick: 100,
        code: state === "permitted"
          ? "CND-PBY-ACT-READY"
          : `CND-HOST-TASK-${state.toUpperCase()}`,
        explanation: `The independent test host policy is ${state}.`,
        ...options,
      },
    },
  );
  expect(response.status()).toBe(204);
}

async function readTaskFront(page) {
  return page.locator("#task-front").evaluate((element) => ({
    sourceIdentity: element.dataset.sourceIdentity || null,
    semanticIdentity: element.dataset.semanticIdentity || null,
    planIdentity: element.dataset.planIdentity || null,
    descriptorIdentity: element.dataset.descriptorIdentity || null,
    runId: element.dataset.runId || null,
    taskRequestId: element.dataset.taskRequestId || null,
    primaryActionRequestId: element.dataset.primaryActionRequestId || null,
    primaryActionPlanEpoch: Number.parseInt(element.dataset.primaryActionPlanEpoch || "", 10),
    readinessState: element.dataset.readinessState || null,
    resultObservationState: element.dataset.resultObservationState || null,
    resultSemanticStatus: element.dataset.resultSemanticStatus || null,
    resultRequestId: element.dataset.resultRequestId || null,
    resultOperationId: element.dataset.resultOperationId || null,
    resultPlanIdentity: element.dataset.resultPlanIdentity || null,
    resultPlanEpoch: Number.parseInt(element.dataset.resultPlanEpoch || "", 10),
    resultRunId: element.dataset.resultRunId || null,
    terminalRequestId: element.dataset.terminalRequestId || null,
    terminalOperationId: element.dataset.terminalOperationId || null,
    terminalPlanIdentity: element.dataset.terminalPlanIdentity || null,
    terminalPlanEpoch: Number.parseInt(element.dataset.terminalPlanEpoch || "", 10),
    terminalRunId: element.dataset.terminalRunId || null,
    terminalState: element.dataset.terminalState || null,
    cleanupState: element.dataset.cleanupState || null,
    evidenceState: element.dataset.evidenceState || null,
  }));
}

async function readLiveRunId(page) {
  return page.locator("#live-flow-status").getAttribute("data-run-id");
}

async function readPrimaryActionIdentity(page) {
  return page.locator("button.task-front-primary-action").evaluate((button) => ({
    operationId: button.dataset.operationId || null,
    planIdentity: button.dataset.planIdentity || null,
    planEpoch: Number.parseInt(button.dataset.planEpoch || "", 10),
  }));
}

async function assertExactTaskActionResult(page, action) {
  const front = await readTaskFront(page);
  const runId = await readLiveRunId(page);
  const resultText = await page.locator("#task-front-result-value").textContent();
  const taskContext = /Task context: operation (sha256:[^,]+), request (request\/[^,]+), run ([^,]+), plan (sha256:[^,]+), epoch (\d+)/
    .exec(resultText || "");
  expect(front.descriptorIdentity).toMatch(/^sha256:/);
  expect(front.sourceIdentity).toMatch(/^sha256:/);
  expect(action.operationId).toBeTruthy();
  expect(action.planIdentity).toMatch(/^sha256:/);
  expect(action.planEpoch).toBeGreaterThan(0);
  expect(taskContext).not.toBeNull();
  expect(taskContext[1]).toBe(action.operationId);
  expect(taskContext[2]).toMatch(/^request\//);
  expect(taskContext[3]).toBe(runId);
  expect(taskContext[4]).toBe(action.planIdentity);
  expect(Number.parseInt(taskContext[5], 10)).toBe(action.planEpoch);
  expect(resultText).toContain("Semantic result (authoritative-result)");
  expect(resultText).toContain("Terminal: succeeded");
  expect(resultText).toContain("cleanup: complete");
  expect(resultText).toContain("evidence: published");
  return {
    ...front,
    taskRequestId: taskContext[2],
    runId: taskContext[3],
    resultText,
  };
}

async function bindCopyTaskFiles(page, taskFront) {
  const from = taskFront.locator('[data-control-id="copy-from"]');
  const to = taskFront.locator('[data-control-id="copy-to"]');
  await from.getByRole("button", { name: "Choose source for From" }).click();
  await expect(from.locator(".task-front-resource-status")).toContainText(
    "selection-pending",
  );
  await from.getByLabel("Select browser file for From").setInputFiles({
    name: "actual-input.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("bounded filesystem fixture\n"),
  });
  await expect(from.locator(".task-front-resource-status")).toContainText(
    "actual-input.txt — required",
  );
  await from.getByRole("button", { name: "Grant read for From" }).click();
  await expect(from.locator(".task-front-resource-status")).toContainText(
    "actual-input.txt — ready",
  );
  await to.getByRole("button", { name: "Replace destination for To" }).click();
  await expect(to.locator(".task-front-resource-status")).toContainText(
    "selection-pending",
  );
  await to.getByLabel("Download filename for To").fill("copied-output.txt");
  await to.getByRole("button", { name: "Use browser download for To" }).click();
  await expect(to.locator(".task-front-resource-status")).toContainText(
    "copied-output.txt — required",
  );
  await to.getByRole("button", { name: "Grant write + replace for To" }).click();
  await expect(to.locator(".task-front-resource-status")).toContainText(
    "copied-output.txt — ready",
  );
}

test("public URL and page globals cannot authorize task execution", async ({ page }) => {
  await page.addInitScript(() => {
    window.taskActionPolicy = { state: "permitted", generation: 99 };
    window.taskActionPolicyState = "permitted";
  });
  await gotoTour(
    page,
    "panels.jacks-on-the-front",
    "&taskActionPolicy=permitted&taskActionPolicyGeneration=99",
  );
  await expect(page.locator("#task-front-state")).toHaveText("unavailable");
  await expect(page.locator("#run")).toBeDisabled();
  await expect(page.locator("#task-front-result-value")).toContainText(
    "Semantic result (not-run)",
  );
  await expect.poll(() => readLiveRunId(page)).toBeNull();
});

test("standalone host observation runs one action through ordinary controls", async ({ page }) => {
  await gotoTour(page, "panels.jacks-on-the-front");
  await setHostTaskActionPolicy(page, "permitted");
  await expect(page.locator("#task-front-state")).toHaveText("ready");
  const action = await readPrimaryActionIdentity(page);
  await page.getByRole("button", { name: "Run the checked uppercase-text plan" }).click();
  await expect(page.locator("#task-front-result-value")).toContainText("JACKS (succeeded)", {
    timeout: 60_000,
  });
  const front = await assertExactTaskActionResult(page, action);
  expect(front.runId).toBe(await readLiveRunId(page));
});

test("independent host observation runs copy through ordinary controls", async ({ page }) => {
  await gotoTour(page, "library.bounded-filesystem");
  await setHostTaskActionPolicy(page, "permitted");
  await expect(page.locator("#task-front-state")).toHaveText("incomplete-choices");
  const taskFront = page.locator("#task-front");
  await bindCopyTaskFiles(page, taskFront);
  await expect(page.locator("#task-front-state")).toHaveText("ready");
  const copy = taskFront.getByRole("button", { name: "Copy selected file" });
  await expect(copy).toBeEnabled();
  const action = await readPrimaryActionIdentity(page);
  await copy.click();
  await expect(taskFront.locator("#task-front-result-value")).toContainText(
    "Copied 27 bytes — committed",
    { timeout: 60_000 },
  );
  await expect(taskFront.locator("#task-front-result-value")).toContainText(
    "Terminal: succeeded",
  );
  await assertExactTaskActionResult(page, action);
});

test("denied host policy keeps ordinary Run unavailable", async ({ page }) => {
  await gotoTour(page, "panels.jacks-on-the-front");
  await setHostTaskActionPolicy(page, "denied");
  await expect(page.locator("#task-front-state")).toHaveText("denied");
  await expect(page.locator("#run")).toBeDisabled();
  await expect.poll(() => readLiveRunId(page)).toBeNull();
});

test("stale and wrong-observer host policies never replace the accepted observation", async ({
  page,
}) => {
  await gotoTour(page, "panels.jacks-on-the-front");
  await setHostTaskActionPolicy(page, "permitted", { generation: 4 });
  await expect(page.locator("#task-front-state")).toHaveText("ready");

  await setHostTaskActionPolicy(page, "denied", { generation: 3 });
  await expect(page.locator("#task-front-state")).toHaveText("ready");
  await expect(page.locator("#run")).toBeEnabled();

  await setHostTaskActionPolicy(page, "revoked", {
    generation: 5,
    observationId: "conduit.task-policy/wrong-host",
  });
  await expect(page.locator("#task-front-state")).toHaveText("ready");

  await setHostTaskActionPolicy(page, "permitted", { generation: 5 });
  await expect(page.locator("#task-front-state")).toHaveText("ready");
});

test("revocation blocks a second ordinary action after an admitted action", async ({ page }) => {
  await gotoTour(page, "panels.jacks-on-the-front");
  await setHostTaskActionPolicy(page, "permitted");
  await expect(page.locator("#task-front-state")).toHaveText("ready");
  const action = await readPrimaryActionIdentity(page);
  await page.getByRole("button", { name: "Run the checked uppercase-text plan" }).click();
  await expect(page.locator("#task-front-result-value")).toContainText("JACKS (succeeded)", {
    timeout: 60_000,
  });
  const prior = await assertExactTaskActionResult(page, action);

  await setHostTaskActionPolicy(page, "revoked", { generation: 3 });
  await expect(page.locator("#task-front-state")).toHaveText("denied");
  await expect(page.locator("#run")).toBeDisabled();
  await expect(page.locator("#task-front-result-value")).toContainText(
    "Semantic result (not-run)",
  );
  expect(await readLiveRunId(page)).toBe(prior.runId);
});

test("source mutation cannot reuse an exact result from the prior plan", async ({ page }) => {
  await gotoTour(page, "panels.jacks-on-the-front");
  await setHostTaskActionPolicy(page, "permitted");
  await expect(page.locator("#task-front-state")).toHaveText("ready");
  const action = await readPrimaryActionIdentity(page);
  await page.getByRole("button", { name: "Run the checked uppercase-text plan" }).click();
  await expect(page.locator("#task-front-result-value")).toContainText("JACKS (succeeded)", {
    timeout: 60_000,
  });
  const before = await assertExactTaskActionResult(page, action);
  await page.getByRole("button", { name: "Show how this works" }).click();
  await expect(page.locator("#workspace")).toHaveAttribute("data-presentation-mode", "build");
  const source = page.locator("#source");
  await source.fill((await source.inputValue()).replace('value = "jacks"', 'value = "late"'));
  await page.locator('[data-presentation-mode="use"]').click();
  await expect(page.locator("#task-front")).toBeVisible();
  await expect.poll(async () => page.locator("#task-front-result-value").textContent()).not.toBe(
    before.resultText,
  );
  await expect(page.locator("#task-front-result-value")).toContainText(
    /not-run|stale|mismatch/i,
  );
});
