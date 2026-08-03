import { expect, test } from "@playwright/test";

async function waitForTourReady(page) {
  await expect(page.locator("html")).toHaveAttribute("data-tour-ready", "true", { timeout: 60_000 });
}

async function postHostTaskActionPolicy(page, policy) {
  await page.evaluate((update) => {
    window.postMessage({ type: "conduit-task-action-policy", ...update }, "*");
  }, policy);
}

async function postHostTaskActionRequest(page, request) {
  await page.evaluate((message) => {
    window.postMessage({ type: "conduit-task-action-request", ...message }, "*");
  }, request);
}

async function gotoTour(page, lesson, { proofMode = false } = {}) {
  const suffix = proofMode ? "&tourTaskActionProof=1" : "";
  await page.goto(`/tour/public/index.html?lesson=${lesson}${suffix}`);
  await waitForTourReady(page);
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
    resultTypedDetails: element.dataset.resultTypedDetails || null,
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

async function readPrimaryAction(page) {
  return page.locator("#task-front-action").evaluate((element) => {
    const button = element.querySelector("button.task-front-primary-action");
    const front = document.querySelector("#task-front");
    const planEpoch = Number.parseInt(button?.dataset.operationPlanEpoch || "", 10);
    return {
      operationId: button?.dataset.operationId || null,
      planEpoch: Number.isSafeInteger(planEpoch) ? planEpoch : null,
      sourceIdentity: front?.dataset.sourceIdentity || null,
      planIdentity: front?.dataset.planIdentity || null,
    };
  });
}

async function readLiveRunId(page) {
  return page.locator("#live-flow-status").getAttribute("data-run-id");
}

async function assertTaskFrontExactTaskActionResult(page, options = {}) {
  const { requireTaskRequest = true } = options;
  const front = await readTaskFront(page);
  const runId = await readLiveRunId(page);

  expect(front.descriptorIdentity).toMatch(/^sha256:/);
  expect(front.sourceIdentity).toMatch(/^sha256:/);
  expect(front.planIdentity).toMatch(/^sha256:/);
  expect(front.semanticIdentity).toMatch(/^sha256:/);
  if (requireTaskRequest) {
    expect(front.taskRequestId).toMatch(/^request\//);
  } else if (front.taskRequestId !== null) {
    expect(front.taskRequestId).toMatch(/^request\//);
  }
  expect(front.primaryActionRequestId).toBe(front.taskRequestId);
  expect(front.readinessState).toBe("terminal");
  expect(front.resultRequestId).toBe(front.taskRequestId);
  expect(front.terminalRequestId).toBe(front.taskRequestId);
  expect(front.resultOperationId).toBeTruthy();
  expect(front.terminalOperationId).toBe(front.resultOperationId);
  expect(front.resultPlanIdentity).toBe(front.planIdentity);
  expect(front.terminalPlanIdentity).toBe(front.planIdentity);
  expect(front.resultPlanEpoch).toBe(front.primaryActionPlanEpoch);
  expect(front.terminalPlanEpoch).toBe(front.primaryActionPlanEpoch);
  expect(front.resultRunId).toBe(runId);
  expect(front.terminalRunId).toBe(runId);
  expect(front.resultObservationState).toBe("authoritative-result");
  expect(front.resultSemanticStatus).toBe("succeeded");
  expect(front.terminalState).toBe("succeeded");
  expect(front.cleanupState).toBe("complete");
  expect(front.evidenceState).toBe("published");

  return front;
}

async function bindCopyTaskFiles(page, taskFront) {
  const activate = async (control) => {
    await control.focus();
    await page.keyboard.press("Enter");
  };

  const from = taskFront.locator('[data-control-id="copy-from"]');
  const to = taskFront.locator('[data-control-id="copy-to"]');

  await activate(from.getByRole("button", { name: "Choose source for From" }));
  await from.getByLabel("Select browser file for From").setInputFiles({
    name: "actual-input.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("bounded filesystem fixture\n"),
  });
  await expect(from.locator(".task-front-resource-status")).toContainText(
    "actual-input.txt — required",
    { timeout: 5_000 },
  );
  await activate(from.getByRole("button", { name: "Grant read for From" }));
  await expect(from.locator(".task-front-resource-status")).toContainText(
    "actual-input.txt — ready",
    { timeout: 5_000 },
  );

  await activate(to.getByRole("button", { name: "Replace destination for To" }));
  await expect(to.locator(".task-front-resource-status")).toContainText(
    "selection-pending",
    { timeout: 5_000 },
  );
  await to.getByLabel("Download filename for To").fill("copied-output.txt");
  await activate(to.getByRole("button", { name: "Use browser download for To" }));
  await expect(to.locator(".task-front-resource-status")).toContainText(
    "copied-output.txt — required",
    { timeout: 5_000 },
  );
  await activate(to.getByRole("button", { name: "Grant write + replace for To" }));
  await expect(to.locator(".task-front-resource-status")).toContainText(
    "copied-output.txt — ready",
    { timeout: 5_000 },
  );
  await expect(from.locator(".task-front-resource-status")).toContainText(
    "actual-input.txt — ready",
    { timeout: 5_000 },
  );
}

test("does not authorize task-execution from ordinary public navigation", async ({ page }) => {
  await gotoTour(page, "panels.jacks-on-the-front");
  await expect(page.locator("#task-front-state")).toBeVisible();
  await expect(page.locator("#run")).toBeDisabled();
  await expect(page.locator("#task-front-result-value")).toBeHidden();
  await expect(page.locator("#task-front-state")).toHaveText("denied");
  await expect.poll(() => readLiveRunId(page), { timeout: 5_000 }).toBeNull();
});

test("runs uppercase task with production-grade host policy via ordinary UI controls", async ({ page }) => {
  await gotoTour(page, "panels.jacks-on-the-front");
  await postHostTaskActionPolicy(page, {
    state: "permitted",
    generation: 2,
    code: "CND-PBY-ACT-READY",
    observedAtTick: 10,
    validUntilTick: 100,
  });

  await expect(page.locator("#task-front-state")).toHaveText("ready");
  await page.getByRole("button", { name: "Run the checked uppercase-text plan" }).click();
  await expect(page.locator("#task-front-result-value")).toContainText("JACKS (succeeded)", {
    timeout: 60_000,
  });
  const front = await assertTaskFrontExactTaskActionResult(page);
  expect(front.runId).toBe(await readLiveRunId(page));
});

test("runs copy task through ordinary controls after authentic policy grant", async ({ page }) => {
  await gotoTour(page, "library.bounded-filesystem");
  await postHostTaskActionPolicy(page, {
    state: "permitted",
    generation: 2,
    code: "CND-PBY-ACT-READY",
    observedAtTick: 10,
    validUntilTick: 100,
  });

  await expect(page.locator("#task-front-state")).toHaveText("incomplete-choices");
  const taskFront = page.locator("#task-front");
  await bindCopyTaskFiles(page, taskFront);
  await expect(page.locator("#task-front-state")).toHaveText("ready");

  const copy = taskFront.getByRole("button", { name: "Copy selected file" });
  await expect(copy).toBeEnabled();
  await copy.click();
  await expect(taskFront.locator("#task-front-result")).toBeVisible({ timeout: 60_000 });
  await expect(taskFront.locator("#task-front-result-value")).toContainText(
    "Copied 27 bytes — committed",
    { timeout: 60_000 },
  );
  await expect(taskFront.locator("#task-front-result-value")).toContainText(
    "Terminal: succeeded",
    { timeout: 60_000 },
  );
  await assertTaskFrontExactTaskActionResult(page);
});

test("rejects run when policy is denied", async ({ page }) => {
  await gotoTour(page, "panels.jacks-on-the-front", { proofMode: true });
  await postHostTaskActionPolicy(page, {
    state: "denied",
    generation: 1,
    code: "CND-HOST-TASK-DENIED",
    observedAtTick: 10,
    validUntilTick: 100,
  });
  await expect(page.locator("#task-front-state")).toHaveText("denied");
  const action = await readPrimaryAction(page);
  await postHostTaskActionRequest(page, action);
  await expect.poll(async () => page.locator("#result").textContent(), {
    timeout: 15_000,
  }).toContain("Task-action policy is not currently request-available.");
  await expect.poll(() => readLiveRunId(page), { timeout: 5_000 }).toBeNull();
});

test("ignores stale policy generations and keeps latest admissible policy", async ({ page }) => {
  await gotoTour(page, "panels.jacks-on-the-front", { proofMode: true });
  await postHostTaskActionPolicy(page, {
    state: "permitted",
    generation: 4,
    code: "CND-PBY-ACT-READY",
    observedAtTick: 10,
    validUntilTick: 100,
  });
  await expect(page.locator("#task-front-state")).toHaveText("ready");

  await postHostTaskActionPolicy(page, {
    state: "denied",
    generation: 3,
    code: "CND-HOST-TASK-DENIED",
    observedAtTick: 10,
    validUntilTick: 100,
  });
  await expect(page.locator("#result")).toContainText("CND-PBY-ACT-006");
  await expect(page.locator("#task-front-state")).toHaveText("ready");

  await postHostTaskActionPolicy(page, {
    state: "permitted",
    generation: 5,
    code: "CND-PBY-ACT-READY",
    observedAtTick: 10,
    validUntilTick: 100,
  });
  await expect(page.locator("#task-front-state")).toHaveText("ready");
});

test("revoked policy blocks synthetic admission after prior allowed action", async ({ page }) => {
  await gotoTour(page, "panels.jacks-on-the-front", { proofMode: true });
  await postHostTaskActionPolicy(page, {
    state: "permitted",
    generation: 2,
    code: "CND-PBY-ACT-READY",
    observedAtTick: 10,
    validUntilTick: 100,
  });
  await expect(page.locator("#task-front-state")).toHaveText("ready");

  const action = await readPrimaryAction(page);
  await postHostTaskActionRequest(page, action);
  await expect.poll(() => readLiveRunId(page), { timeout: 60_000 }).not.toBeNull();
  await expect.poll(() => readTaskFront(page), { timeout: 5_000 }).toMatchObject({
    taskRequestId: expect.stringMatching(/^request\//),
    terminalRequestId: expect.stringMatching(/^request\//),
  });
  const prior = await readTaskFront(page);

  await postHostTaskActionPolicy(page, {
    state: "revoked",
    generation: 3,
    code: "CND-HOST-TASK-REVOKED",
    observedAtTick: 10,
    validUntilTick: 100,
  });
  await expect(page.locator("#task-front-state")).toHaveText("denied");
  await postHostTaskActionRequest(page, action);
  await expect.poll(async () => page.locator("#result").textContent(), {
    timeout: 15_000,
  }).toContain("Task-action policy is not currently request-available.");
  const after = await readTaskFront(page);
  expect(after.terminalRequestId).toBe(prior.taskRequestId);
  expect(after.terminalRequestId).toBe(after.resultRequestId);
});

test("rejects synthetic request with wrong source identity", async ({ page }) => {
  await gotoTour(page, "panels.jacks-on-the-front", { proofMode: true });
  await postHostTaskActionPolicy(page, {
    state: "permitted",
    generation: 2,
    code: "CND-PBY-ACT-READY",
    observedAtTick: 10,
    validUntilTick: 100,
  });
  const action = await readPrimaryAction(page);

  await postHostTaskActionRequest(page, {
    ...action,
    sourceIdentity: `${action.sourceIdentity}-wrong`,
    planEpoch: action.planEpoch + 1,
  });
  await expect.poll(async () => page.locator("#result").textContent(), {
    timeout: 15_000,
  }).toContain("Task action request belongs to a stale source, plan, operation, or epoch");
  await expect.poll(() => readLiveRunId(page), { timeout: 5_000 }).toBeNull();
});

test("rejects stale plan results after source mutation", async ({ page }) => {
  await gotoTour(page, "panels.jacks-on-the-front");
  await postHostTaskActionPolicy(page, {
    state: "permitted",
    generation: 2,
    code: "CND-PBY-ACT-READY",
    observedAtTick: 10,
    validUntilTick: 100,
  });
  await expect(page.locator("#task-front-state")).toHaveText("ready");
  await page.getByRole("button", { name: "Run the checked uppercase-text plan" }).click();
  await expect(page.locator("#task-front-result-value")).toContainText("JACKS (succeeded)", {
    timeout: 60_000,
  });

  const before = await assertTaskFrontExactTaskActionResult(page);
  const source = page.locator("#source");
  await source.fill((await source.inputValue()).replace("value = \"jacks\"", "value = \"late\""));
  await expect(page.locator("#task-front-result-value")).toContainText(
    "Terminal: succeeded",
    { timeout: 60_000 },
  );

  const after = await readTaskFront(page);
  expect(after.resultRunId).not.toBe(before.resultRunId);
  expect(after.resultObservationState).toBe("stale-or-mismatched-result-rejected");
});
