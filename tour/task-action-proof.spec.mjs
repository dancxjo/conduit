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

async function gotoTourProof(page, lesson = "panels.jacks-on-the-front") {
  await page.goto(`/tour/public/index.html?lesson=${lesson}&tourTaskActionProof=1`);
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

async function assertTaskFrontExactRunResult(page) {
  return assertTaskFrontExactTaskActionResult(page);
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
  } else {
    if (front.taskRequestId !== null) {
      expect(front.taskRequestId).toMatch(/^request\//);
    }
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

async function bindCopyTaskFiles(page, taskFront) {
  const activate = async (control) => {
    await control.focus();
    await page.keyboard.press("Enter");
  };

  const from = taskFront.locator('[data-control-id="copy-from"]');
  const to = taskFront.locator('[data-control-id="copy-to"]');
  const activeFrom = from;
  const activeTo = to;

  await activate(activeFrom.getByRole("button", { name: "Choose source for From" }));
  await activeFrom.getByLabel("Select browser file for From").setInputFiles({
    name: "actual-input.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("bounded filesystem fixture\n"),
  });
  await expect(activeFrom.locator(".task-front-resource-status")).toContainText(
    "actual-input.txt — required",
  );
  await activate(activeFrom.getByRole("button", { name: "Grant read for From" }));
  await expect(activeFrom.locator(".task-front-resource-status")).toContainText(
    "actual-input.txt — ready",
  );

  await activate(activeTo.getByRole("button", { name: "Replace destination for To" }));
  await expect(activeTo.locator(".task-front-resource-status")).toContainText(
    "selection-pending",
  );
  await activeTo.getByLabel("Download filename for To").fill("copied-output.txt");
  await activate(activeTo.getByRole("button", { name: "Use browser download for To" }));
  await expect(activeTo.locator(".task-front-resource-status")).toContainText(
    "copied-output.txt — required",
  );
  await activate(activeTo.getByRole("button", { name: "Grant write + replace for To" }));
  await expect(activeTo.locator(".task-front-resource-status")).toContainText(
    "copied-output.txt — ready",
  );
  await expect(activeFrom.locator(".task-front-resource-status")).toContainText(
    "actual-input.txt — ready",
  );
}

async function readLiveRunId(page) {
  return page.locator("#live-flow-status").getAttribute("data-run-id");
}

test("ignores synthetic task-action request when proof mode is disabled", async ({ page }) => {
  await page.goto("/tour/public/index.html?lesson=panels.jacks-on-the-front");
  await waitForTourReady(page);
  await postHostTaskActionPolicy(page, {
    state: "permitted",
    generation: 2,
    code: "CND-PBY-ACT-READY",
    observedAtTick: 10,
    validUntilTick: 100,
  });

  await expect(page.locator("#task-front-state")).toHaveText("ready");
  const action = await readPrimaryAction(page);
  expect(action.operationId).toBeTruthy();
  await postHostTaskActionRequest(page, action);

  await expect.poll(() => readLiveRunId(page), { timeout: 5_000 }).toBeNull();
  const front = await readTaskFront(page);
  expect(front.taskRequestId).toBeNull();
  expect(front.readinessState).toBe("ready");
});

test("starts exact run from synthetic task-action request in test mode", async ({ page }) => {
  await gotoTourProof(page);
  await postHostTaskActionPolicy(page, {
    state: "permitted",
    generation: 2,
    code: "CND-PBY-ACT-READY",
    observedAtTick: 10,
    validUntilTick: 100,
  });

  await expect(page.locator("#task-front-state")).toHaveText("ready");
  await expect(page.locator("#run")).toBeEnabled();
  const action = await readPrimaryAction(page);
  expect(action.operationId).toBeTruthy();
  await postHostTaskActionRequest(page, action);

  await expect.poll(() => readLiveRunId(page), { timeout: 60_000 }).not.toBeNull();
  const runId = await readLiveRunId(page);
  expect(typeof runId).toBe("string");

  const result = page.locator("#task-front-result-value");
  await expect(result).toContainText("JACKS (succeeded)", { timeout: 60_000 });
  await assertTaskFrontExactRunResult(page);
});

test("runs task-front task through ordinary tour controls without proof wiring", async ({ page }) => {
  await page.goto("/tour/public/index.html?lesson=panels.jacks-on-the-front");
  await waitForTourReady(page);
  await expect(page.locator("#task-front-state")).toHaveText("denied");
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
  const front = await assertTaskFrontExactRunResult(page);
  expect(front.runId).toEqual(await readLiveRunId(page));
});

test("rejects synthetic task-action request when policy is absent in proof mode", async ({ page }) => {
  await gotoTourProof(page);
  await expect(page.locator("#task-front-state")).toHaveText("denied");
  await expect(page.locator("#run")).toBeDisabled();

  const action = await readPrimaryAction(page);
  expect(action.operationId).toBeTruthy();
  await postHostTaskActionRequest(page, action);

  await expect.poll(async () => page.locator("#result").textContent(), {
    timeout: 15_000,
  }).toContain("Task-action policy is not currently request-available.");
  await expect.poll(() => readLiveRunId(page), { timeout: 5_000 }).toBeNull();
  const front = await readTaskFront(page);
  expect(front.taskRequestId).toBeNull();
  expect(front.readinessState).toBe("denied");
});

test("runs copy task-front through production permission flow", async ({ page }) => {
  await page.goto("/tour/public/index.html?lesson=library.bounded-filesystem");
  await waitForTourReady(page);
  await expect(page.locator("#task-front-state")).toHaveText("denied");
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

test("runs copy task-front through synthetic request in proof mode", async ({ page }) => {
  await gotoTourProof(page, "library.bounded-filesystem");
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

  const action = await readPrimaryAction(page);
  expect(action.operationId).toBeTruthy();
  await postHostTaskActionRequest(page, action);

  await expect.poll(() => readLiveRunId(page), { timeout: 60_000 }).not.toBeNull();
  await expect(taskFront.locator("#task-front-result-value")).toContainText(
    "Copied 27 bytes — committed",
    { timeout: 60_000 },
  );
  await expect(taskFront.locator("#task-front-result-value")).toContainText(
    "Terminal: succeeded",
    { timeout: 60_000 },
  );
  const front = await assertTaskFrontExactTaskActionResult(page);
  expect(front.taskRequestId).toBe(front.resultRequestId);
  expect(front.runId).toBe(front.resultRunId);
});

test("rejects synthetic task-action request when policy is denied in proof mode", async ({ page }) => {
  await gotoTourProof(page);
  await postHostTaskActionPolicy(page, {
    state: "denied",
    generation: 1,
    code: "CND-HOST-TASK-DENIED",
    observedAtTick: 10,
    validUntilTick: 100,
  });
  await expect(page.locator("#task-front-state")).toHaveText("denied");
  await expect(page.locator("#run")).toBeDisabled();

  const action = await readPrimaryAction(page);
  expect(action.operationId).toBeTruthy();
  await postHostTaskActionRequest(page, action);

  await expect.poll(async () => page.locator("#result").textContent(), {
    timeout: 15_000,
  }).toContain("Task-action policy is not currently request-available.");
  await expect.poll(() => readLiveRunId(page), { timeout: 5_000 }).toBeNull();
  const front = await readTaskFront(page);
  expect(front.taskRequestId).toBeNull();
});

test("synthetic run is rejected as stale when source changes after completion", async ({ page }) => {
  await gotoTourProof(page);
  await postHostTaskActionPolicy(page, {
    state: "permitted",
    generation: 2,
    code: "CND-PBY-ACT-READY",
    observedAtTick: 10,
    validUntilTick: 100,
  });
  await expect(page.locator("#task-front-state")).toHaveText("ready");

  const action = await readPrimaryAction(page);
  expect(action.operationId).toBeTruthy();
  await postHostTaskActionRequest(page, action);
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
  const front = await readTaskFront(page);
  expect(front.resultRunId).not.toBe(before.runId);
  expect(front.resultRunId).not.toBe(front.runId);
  expect(front.resultObservationState).toBe("stale-or-mismatched-result-rejected");
  expect(front.resultRequestId).toBe(before.taskRequestId);
  expect(front.terminalRequestId).toBe(before.taskRequestId);
  expect(front.readinessState).not.toBe("terminal");
});

test("runs task action then verifies late-result rejection after source mutation", async ({ page }) => {
  await page.goto("/tour/public/index.html?lesson=panels.jacks-on-the-front");
  await waitForTourReady(page);
  await postHostTaskActionPolicy(page, {
    state: "permitted",
    generation: 2,
    code: "CND-PBY-ACT-READY",
    observedAtTick: 10,
    validUntilTick: 100,
  });
  await expect(page.locator("#task-front-state")).toHaveText("ready");
  await page.getByRole("button", { name: "Run the checked uppercase-text plan" }).click();
  await expect(page.locator("#task-front-result-value")).toContainText(
    "JACKS (succeeded)",
    { timeout: 60_000 },
  );
  const firstRunId = await readLiveRunId(page);
  expect(typeof firstRunId).toBe("string");
  const source = page.locator("#source");
  await source.fill((await source.inputValue()).replace("value = \"jacks\"", "value = \"late\""));
  await expect(page.locator("#task-front-result-value")).toContainText(
    "Terminal: succeeded",
    { timeout: 60_000 },
  );
  const front = await readTaskFront(page);
  expect(front.runId).not.toBe(firstRunId);
  expect(front.resultRunId).not.toBe(front.runId);
  expect(front.resultObservationState).toBe("stale-or-mismatched-result-rejected");
  expect(front.readinessState).not.toBe("terminal");
});

test("ignores stale synthetic task-action request after completion", async ({ page }) => {
  await gotoTourProof(page);
  await postHostTaskActionPolicy(page, {
    state: "permitted",
    generation: 2,
    code: "CND-PBY-ACT-READY",
    observedAtTick: 10,
    validUntilTick: 100,
  });

  const action = await readPrimaryAction(page);
  expect(action.operationId).toBeTruthy();
  await postHostTaskActionRequest(page, action);

  await expect.poll(() => readLiveRunId(page), { timeout: 60_000 }).not.toBeNull();
  const terminalRunId = await readLiveRunId(page);
  expect(typeof terminalRunId).toBe("string");
  await expect(page.locator("#task-front-result-value")).toContainText("JACKS (succeeded)", {
    timeout: 60_000,
  });

  await postHostTaskActionRequest(page, {
    ...action,
    sourceIdentity: `${action.sourceIdentity}-wrong`,
  });
  await expect.poll(async () => page.locator("#result").textContent(), {
    timeout: 15_000,
  }).toContain(
    "Task action request belongs to a stale source, plan, operation, or epoch",
  );

  const front = await readTaskFront(page);
  expect(front.taskRequestId).toMatch(/^request\//);
  expect(front.taskRequestId).toBe(front.terminalRequestId);
  expect(front.readinessState).toBe("terminal");
});

test("rejects synthetic task-action request with wrong source identity", async ({ page }) => {
  await gotoTourProof(page);
  await postHostTaskActionPolicy(page, {
    state: "permitted",
    generation: 2,
    code: "CND-PBY-ACT-READY",
    observedAtTick: 10,
    validUntilTick: 100,
  });

  const action = await readPrimaryAction(page);
  expect(action.operationId).toBeTruthy();
  await postHostTaskActionRequest(page, {
    ...action,
    sourceIdentity: `${action.sourceIdentity}-wrong`,
    planEpoch: 99,
  });

  await expect.poll(async () => page.locator("#result").textContent(), {
    timeout: 15_000,
  }).not.toBe("Task-action policy is not currently request-available.");
  const result = await page.locator("#result").textContent();
  expect(result).toContain("Task action request belongs to a stale source, plan, operation, or epoch");
  await expect.poll(() => readLiveRunId(page), { timeout: 5_000 }).toBeNull();
  const front = await readTaskFront(page);
  expect(front.taskRequestId).toBeNull();
});

test("rejects synthetic request for stale plan epoch", async ({ page }) => {
  await gotoTourProof(page);
  await postHostTaskActionPolicy(page, {
    state: "permitted",
    generation: 2,
    code: "CND-PBY-ACT-READY",
    observedAtTick: 10,
    validUntilTick: 100,
  });

  const action = await readPrimaryAction(page);
  expect(action.planEpoch).toBeGreaterThan(0);
  await postHostTaskActionRequest(page, {
    ...action,
    planEpoch: action.planEpoch - 1,
  });

  await expect(page.locator("#result")).toContainText("stale source, plan, operation, or epoch", {
    timeout: 15_000,
  });
  const runId = await readLiveRunId(page);
  expect(runId).toBeNull();
});

test("stale host policy updates do not become authoritative", async ({ page }) => {
  await gotoTourProof(page);
  await postHostTaskActionPolicy(page, {
    state: "permitted",
    generation: 5,
    code: "CND-PBY-ACT-READY",
    observedAtTick: 10,
    validUntilTick: 100,
  });
  await expect(page.locator("#task-front-state")).toHaveText("ready");

  await postHostTaskActionPolicy(page, {
    state: "denied",
    generation: 4,
    code: "CND-HOST-TASK-DENIED",
    observedAtTick: 10,
    validUntilTick: 100,
  });
  await expect(page.locator("#result")).toContainText("CND-PBY-ACT-006");
  await expect(page.locator("#task-front-state")).toHaveText("ready");

  await postHostTaskActionPolicy(page, {
    state: "permitted",
    generation: 6,
    code: "CND-PBY-ACT-READY",
    observedAtTick: 10,
    validUntilTick: 100,
  });
  await expect(page.locator("#task-front-state")).toHaveText("ready");

  const action = await readPrimaryAction(page);
  await postHostTaskActionRequest(page, action);
  await expect.poll(() => readLiveRunId(page), { timeout: 60_000 }).not.toBeNull();
  const front = await readTaskFront(page);
  expect(front.taskRequestId).toMatch(/^request\//);
  expect(front.readinessState).toBe("terminal");
});

test("revoked task-action policy blocks synthetic requests in proof mode", async ({ page }) => {
  await gotoTourProof(page);
  await postHostTaskActionPolicy(page, {
    state: "permitted",
    generation: 2,
    code: "CND-PBY-ACT-READY",
    observedAtTick: 10,
    validUntilTick: 100,
  });
  await expect(page.locator("#task-front-state")).toHaveText("ready");

  await postHostTaskActionPolicy(page, {
    state: "revoked",
    generation: 3,
    code: "CND-HOST-TASK-REVOKED",
    observedAtTick: 10,
    validUntilTick: 100,
  });
  await expect(page.locator("#task-front-state")).toHaveText("denied");
  await expect(page.locator("#run")).toBeDisabled();

  const action = await readPrimaryAction(page);
  expect(action.operationId).toBeTruthy();
  await postHostTaskActionRequest(page, action);
  await expect.poll(async () => page.locator("#result").textContent(), {
    timeout: 15_000,
  }).toContain("Task-action policy is not currently request-available.");
  await expect.poll(() => readLiveRunId(page), { timeout: 5_000 }).toBeNull();
});

test("revoked task-action policy blocks synthetic requests after an admitted action in proof mode", async ({ page }) => {
  await gotoTourProof(page);
  await postHostTaskActionPolicy(page, {
    state: "permitted",
    generation: 2,
    code: "CND-PBY-ACT-READY",
    observedAtTick: 10,
    validUntilTick: 100,
  });
  await expect(page.locator("#task-front-state")).toHaveText("ready");

  const action = await readPrimaryAction(page);
  expect(action.operationId).toBeTruthy();
  await postHostTaskActionRequest(page, action);
  await expect.poll(() => readLiveRunId(page), { timeout: 60_000 }).not.toBeNull();
  const firstRequestFront = await assertTaskFrontExactTaskActionResult(page);
  const firstRequestId = firstRequestFront.taskRequestId;
  expect(firstRequestId).toMatch(/^request\//);

  await postHostTaskActionPolicy(page, {
    state: "revoked",
    generation: 3,
    code: "CND-HOST-TASK-REVOKED",
    observedAtTick: 10,
    validUntilTick: 100,
  });
  await expect(page.locator("#task-front-state")).toHaveText("denied");
  await expect(page.locator("#run")).toBeDisabled();

  await postHostTaskActionRequest(page, action);
  await expect.poll(async () => page.locator("#result").textContent(), {
    timeout: 15_000,
  }).toContain("Task-action policy is not currently request-available.");
  await expect.poll(() => readLiveRunId(page), { timeout: 5_000 }).toBeNull();

  await expect.poll(() => readTaskFront(page), { timeout: 5_000 }).toMatchObject({
    taskRequestId: firstRequestId,
    terminalRequestId: firstRequestId,
  });
});
