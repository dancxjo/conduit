import { spawn } from "node:child_process";
import { mkdir, rename, writeFile } from "node:fs/promises";
import { dirname } from "node:path";
import { createInterface } from "node:readline";
import { expect, test } from "@playwright/test";

function startPublicEntrance() {
  const child = spawn("target/debug/patchbay-html", [
    "--form", "Text Lab", "forms/text-lab/main.conduit",
    "--form", "Hello", "forms/hello/main.conduit",
  ], {
    stdio: ["ignore", "pipe", "pipe"],
  });
  const errors = [];
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", (chunk) => errors.push(chunk));
  const lines = createInterface({ input: child.stdout });
  const url = new Promise((resolve, reject) => {
    lines.once("line", (line) => resolve(line.replace("PATCHBAY_HTML_URL=", "")));
    child.once("exit", (code) => {
      reject(new Error(`public Patchbay entrance exited ${code}: ${errors.join("")}`));
    });
  });
  return { child, errors, lines, url };
}

const semanticBasis = ({ presentation }) => ({
  source_document_id: presentation.basis.source_document_id,
  checked_form_id: presentation.basis.checked_form_id,
  expanded_form_id: presentation.basis.expanded_form_id,
  body_id: presentation.basis.body_id,
  wake_id: presentation.basis.wake_id,
  plan_id: presentation.basis.plan_id,
  active_play_id: presentation.basis.active_play_id,
});

async function snapshot(page) {
  return page.evaluate(async () => (await fetch("/api/snapshot", { cache: "no-store" })).json());
}

async function enactNavigation(page, steps, operation, enact) {
  const before = await snapshot(page);
  const response = page.waitForResponse(
    candidate => candidate.url().endsWith("/api/navigation") && candidate.request().method() === "POST",
  );
  await enact();
  const after = await (await response).json();
  expect(after.interaction.last_disposition).toBe("Succeeded");
  expect(after.presentation.identity).toBe(before.presentation.identity);
  expect(after.presentation.revision).toBe(before.presentation.revision);
  expect(after.navigation.navigation.identity).toBe(before.navigation.navigation.identity);
  expect(semanticBasis(after)).toEqual(semanticBasis(before));
  steps.push({
    sequence: steps.length,
    operation,
    disposition: after.interaction.last_disposition,
    presentation_id: after.presentation.identity,
    presentation_revision: after.presentation.revision,
    navigation_id: after.navigation.navigation.identity,
    before_cursor: before.navigation.cursor,
    after_cursor: after.navigation.cursor,
    semantic_basis: semanticBasis(after),
  });
  return after;
}

async function refuseNavigation(page, refusals, operation, request, expected) {
  const before = await snapshot(page);
  const after = await page.evaluate(async body => (await fetch("/api/navigation", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  })).json(), request);
  expect(after.interaction.last_disposition).toBe(`Refused(${expected})`);
  expect(after.navigation.cursor).toEqual(before.navigation.cursor);
  expect(after.presentation.identity).toBe(before.presentation.identity);
  expect(after.presentation.revision).toBe(before.presentation.revision);
  expect(semanticBasis(after)).toEqual(semanticBasis(before));
  refusals.push({
    operation,
    disposition: after.interaction.last_disposition,
    cursor: after.navigation.cursor,
    semantic_basis: semanticBasis(after),
  });
  return after;
}

async function refuseInvocation(page, refusals, operation, request, expected) {
  const before = await snapshot(page);
  const after = await page.evaluate(async body => (await fetch("/api/interaction", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  })).json(), request);
  expect(after.interaction.last_disposition).toBe(`Refused(${expected})`);
  expect(after.navigation.cursor).toEqual(before.navigation.cursor);
  expect(after.presentation.identity).toBe(before.presentation.identity);
  expect(after.presentation.revision).toBe(before.presentation.revision);
  expect(semanticBasis(after)).toEqual(semanticBasis(before));
  refusals.push({
    operation,
    disposition: after.interaction.last_disposition,
    cursor: after.navigation.cursor,
    semantic_basis: semanticBasis(after),
  });
  return after;
}

test("public browser entrance stays unbodied until OPEN then explicit BIRTH", async ({ browser, page }) => {
  const server = startPublicEntrance();
  try {
    const url = await server.url;
    const initial = await (await fetch(`${url}/api/snapshot`)).json();
    expect(initial.entrance.layer).toBe("World");
    expect(initial.entrance.body_id).toBeNull();
    expect(initial.entrance.selected_subject).toMatch(/^host\//);
    expect(initial.parts).toBeUndefined();
    expect(initial.presentation.basis).toMatchObject({
      body_id: null,
      wake_id: null,
      source_document_id: null,
      checked_form_id: null,
      expanded_form_id: null,
      plan_id: null,
      active_play_id: null,
    });
    expect(initial.presentation.subjects.some(({ role }) => role === "Host")).toBe(true);
    expect(initial.presentation.subjects.some(({ role }) => role === "Form")).toBe(true);
    expect(initial.presentation.subjects.some(({ role }) => role === "Body")).toBe(false);
    expect(initial.presentation.subjects.some(({ role }) => role === "Part")).toBe(false);
    expect(initial.presentation.properties).toEqual(expect.arrayContaining([
      expect.objectContaining({ name: "current-body", value: { Text: "none" } }),
      expect.objectContaining({ name: "this-host", value: { Flag: true } }),
    ]));

    await page.goto(url);
    const navigationRefusals = [];
    await refuseNavigation(page, navigationRefusals, { kind: "show", aspect: "Plan" }, {
      presentation_id: initial.presentation.identity,
      presentation_revision: initial.presentation.revision,
      navigation_id: initial.navigation.navigation.identity,
      operation: { kind: "show", aspect: "Plan" },
    }, "UnknownAspect");
    await expect(page.getByRole("heading", { name: "Entrance choices" })).toBeVisible();
    await expect(page.locator("body")).toHaveAttribute("data-place", "Entrance");
    await expect(page.locator('[data-application-key="product-status"]')).toContainText("Manifestation Available");
    expect(await page.evaluate(() => globalThis.__patchbayMembership)).toBeUndefined();
    expect(await page.locator('#subjects input[type="radio"]').evaluateAll((choices) =>
      choices.every((choice) => !["Body", "Part", "Gear", "Port", "Cord", "Line"].includes(choice.dataset.role)),
    )).toBe(true);
    const workspaceBox = await page.locator(".workspace").boundingBox();
    expect(workspaceBox.y + workspaceBox.height).toBeLessThanOrEqual(768);
    await page.getByRole("button", { name: "Forms", exact: true }).click();
    await expect(page.getByRole("navigation", { name: "Available Forms" }).getByRole("button")).toHaveCount(3);
    await expect(page.getByRole("button", { name: "Open Form Text Lab" })).toBeVisible();
    await page.getByRole("searchbox", { name: "Find a Form" }).fill("hElLo");
    await expect(page.locator("#form-results-status")).toHaveText("1 of 3 Forms available");
    const form = initial.presentation.subjects.find(({ role, label }) => role === "Form" && label === "Hello");
    const formButton = page.getByRole("button", { name: "Open Form Hello" });
    await page.getByRole("searchbox", { name: "Find a Form" }).press("ArrowDown");
    await expect(formButton).toBeFocused();
    await page.evaluate(() => window.patchbayReload());
    await expect(formButton).toBeFocused();
    const openResponses = [];
    let resolveOpenSequence;
    const openSequence = new Promise((resolve) => { resolveOpenSequence = resolve; });
    page.on("response", (response) => {
      if(["/api/navigation", "/api/interaction"].some(path => response.url().endsWith(path)) && response.request().method() === "POST") {
        openResponses.push(response);
        if(openResponses.length === 2) resolveOpenSequence();
      }
    });
    await Promise.all([openSequence, formButton.press("Enter")]);
    expect(openResponses).toHaveLength(2);
    expect(openResponses.map(response => new URL(response.url()).pathname).sort()).toEqual([
      "/api/interaction",
      "/api/navigation",
    ]);
    await expect(page.locator("body")).toHaveAttribute("data-inspector-open", "true");
    const birthButton = page.getByRole("button", { name: "BIRTH" });
    await expect(birthButton).toBeVisible();
    await expect(birthButton).toBeEnabled();
    expect(openResponses.every((response) => response.ok())).toBe(true);
    await expect(page.getByRole("button", { name: "OPEN", exact: true })).toBeEnabled();
    const openAction = initial.presentation.actions.find(
      ({ intent, target }) => intent === "conduit.intent/open@1" && target === form.identity,
    );
    const opened = await (await fetch(`${url}/api/snapshot`)).json();
    expect(opened.interaction.last_request_id).toMatch(/^navigation\//);
    expect(opened.interaction.last_disposition).toBe("Succeeded");
    expect(openAction.identity).toMatch(/^action\/open\//);
    expect(opened.revision).toBe(initial.revision + 1);
    expect(opened.presentation.basis.body_id).toBeNull();
    expect(opened.navigation.cursor.place).toBe("Program");
    expect(opened.parts).toBeUndefined();
    expect(opened.presentation.properties).toContainEqual(
      expect.objectContaining({ subject: form.identity, name: "opened", value: { Flag: true } }),
    );
    expect(opened.presentation.subjects).toEqual(expect.arrayContaining([
      expect.objectContaining({ role: "Form" }),
      expect.objectContaining({ role: "Gear", label: "hello/upper" }),
      expect.objectContaining({ role: "Cord" }),
    ]));
    const upperFaceplate = page.locator('.faceplate-title[title="hello/upper"]');
    await expect(upperFaceplate).toHaveText("upper");
    await expect(upperFaceplate).toHaveAttribute("title", "hello/upper");
    const birthAction = opened.presentation.actions.find(
      ({ intent, target }) => intent === "conduit.intent/birth@1" && target === form.identity,
    );
    expect(birthAction.identity).toMatch(/^action\/birth\//);

    const exact = page.locator("#inspector .exact-selection");
    await expect(exact).not.toHaveAttribute("open", "");
    await exact.locator("summary").click();
    await expect(exact).toHaveAttribute("open", "");
    await expect(exact).toContainText(form.identity);
    await exact.locator("summary").click();
    await expect(exact).not.toHaveAttribute("open", "");

    const [birthRequest] = await Promise.all([
      page.waitForRequest(
        (request) => request.url().endsWith("/api/interaction") && request.method() === "POST",
      ),
      birthButton.click(),
    ]);
    expect(birthRequest.postDataJSON()).toMatchObject({
      kind: "invoke",
      action_id: birthAction.identity,
    });
    const birthResponse = await birthRequest.response();
    expect(birthRequest.failure(), server.errors.join("")).toBeNull();
    expect(birthResponse, server.errors.join("")).not.toBeNull();
    expect(birthResponse.ok()).toBe(true);
    await expect(page.getByRole("heading", { name: "Program structure" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Body", exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Body", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Body topology" })).toBeVisible();
    await page.getByRole("button", { name: "Program", exact: true }).click();
    const born = await (await fetch(`${url}/api/snapshot`)).json();
    expect(born.interaction.last_request_id).toMatch(/^navigation\//);
    expect(born.interaction.last_disposition).toBe("Succeeded");
    expect(born.presentation.basis.body_id).toBeTruthy();
    expect(born.presentation.basis.checked_form_id).toBeTruthy();
    expect(born.presentation.basis.wake_id).toBeNull();
    expect(born.presentation.basis.plan_id).toBeNull();
    expect(born.presentation.basis.active_play_id).toBeNull();
    expect(born.parts.parts).toHaveLength(1);
    expect(born.parts.parts[0].state).toBe("Here");
    expect(born.presentation.subjects.some(({ role }) => role === "Form")).toBe(true);

    const stale = await page.evaluate(async ({ presentationId, revision, subject }) =>
      (await fetch("/api/front-door-transition", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          presentation_id: presentationId,
          revision,
          action: "birth",
          subject,
        }),
      })).json(),
    { presentationId: opened.presentation.identity, revision: opened.revision, subject: form.identity });
    expect(stale.interaction.last_disposition).toBe("Refused(StalePresentation)");
    expect(stale.presentation.basis.body_id).toBe(born.presentation.basis.body_id);

    const wakeAction = born.presentation.actions.find(
      ({ intent }) => intent === "conduit.intent/wake@1",
    );
    expect(wakeAction.availability).toBe("Available");
    await page.getByRole("button", { name: "Navigate", exact: true }).click();
    await page.locator(`#subjects input[type="radio"][data-subject="${wakeAction.target}"]`).click();
    await Promise.all([
      page.waitForResponse(
        (response) => response.url().endsWith("/api/interaction") && response.request().method() === "POST",
      ),
      page.getByRole("button", { name: "WAKE", exact: true }).click(),
    ]);
    const awakened = await (await fetch(`${url}/api/snapshot`)).json();
    expect(awakened.interaction.last_disposition).toBe("Succeeded");
    expect(awakened.presentation.basis.body_id).toBe(born.presentation.basis.body_id);
    expect(awakened.presentation.basis.wake_id).toBeTruthy();
    expect(awakened.presentation.basis.plan_id).toBeNull();

    await page.getByRole("button", { name: "Plan current Form" }).click();
    await expect(page.locator("#front-door-feedback")).toContainText("Plan Succeeded");
    const planned = await (await fetch(`${url}/api/snapshot`)).json();
    expect(planned.presentation.basis.plan_id).toBeTruthy();
    expect(planned.presentation.basis.active_play_id).toBeNull();
    await page.getByRole("button", { name: "Play current Plan" }).click();
    await expect(page.locator("#front-door-feedback")).toContainText("Play Succeeded");
    const playing = await (await fetch(`${url}/api/snapshot`)).json();
    expect(playing.presentation.basis.active_play_id).toBeTruthy();
    expect(playing.parts.parts[0].in_plan).toBe(true);
    expect(playing.parts.parts[0].playing).toBe(true);

    const navigationSteps = [];
    const journeyStartCursor = playing.navigation.cursor;
    expect(playing.navigation.cursor).toMatchObject({ place: "Program", aspect: "Structure" });
    await page.getByRole("button", { name: "Navigate", exact: true }).click();
    const upper = page.locator('#subjects [data-application-component="choice-option-label"]').filter({ hasText: "hello/upper" }).locator('input[type="radio"]');
    const upperIdentity = await upper.getAttribute("data-subject");
    expect(upperIdentity).toBeTruthy();
    let navigated = await enactNavigation(page, navigationSteps,
      { kind: "focus-and-disclose", subject: upperIdentity, depth: "Detail" },
      () => upper.click());
    expect(navigated.navigation.cursor).toMatchObject({
      place: "Program", aspect: "Structure", focus: upperIdentity, depth: "Detail",
    });
    navigated = await enactNavigation(page, navigationSteps, { kind: "show", aspect: "Plan" },
      () => page.getByRole("button", { name: "Plan", exact: true }).click());
    expect(navigated.navigation.cursor).toMatchObject({ place: "Program", aspect: "Plan", focus: null });

    await page.locator("#toggle-structured").click();
    const upperInPlan = page.locator(`#structured-navigator input[type="radio"][data-subject="${upperIdentity.replaceAll('"', '\\"')}"]`);
    navigated = await enactNavigation(page, navigationSteps,
      { kind: "focus-and-disclose", subject: upperIdentity, depth: "Detail" },
      () => upperInPlan.click());
    const followButton = page.locator("#structured-navigator [data-follow]").filter({ hasText: "Host:" }).first();
    const followIdentity = await followButton.getAttribute("data-follow");
    const follow = navigated.navigation.navigation.follows.find(candidate => candidate.identity === followIdentity);
    expect(follow).toMatchObject({ source_subject: upperIdentity, target_place: "Body", target_aspect: "Plan" });
    navigated = await enactNavigation(page, navigationSteps,
      { kind: "follow", relationship: followIdentity, target: follow.target_subject },
      () => followButton.click());
    expect(navigated.navigation.cursor).toMatchObject({
      place: "Body", aspect: "Plan", focus: follow.target_subject, depth: "Detail",
    });
    const bodyPlace = navigated.navigation.navigation.places.find(place => place.place === "Body");
    const currentTruthAspect = bodyPlace.aspects.find(aspect =>
      ["Play", "Signs"].includes(aspect.aspect)
        && aspect.focusable_subjects.includes(follow.target_subject));
    expect(currentTruthAspect).toBeTruthy();
    navigated = await enactNavigation(page, navigationSteps,
      { kind: "show", aspect: currentTruthAspect.aspect },
      () => page.getByRole("button", { name: currentTruthAspect.aspect, exact: true }).click());
    expect(navigated.navigation.cursor).toMatchObject({
      place: "Body", aspect: currentTruthAspect.aspect, focus: null, depth: "Detail",
    });
    const hostInCurrentTruth = page.locator(
      `#structured-navigator input[type="radio"][data-subject="${follow.target_subject.replaceAll('"', '\\"')}"]`,
    );
    navigated = await enactNavigation(page, navigationSteps,
      { kind: "focus-and-disclose", subject: follow.target_subject, depth: "Detail" },
      () => hostInCurrentTruth.click());
    navigated = await enactNavigation(page, navigationSteps, { kind: "disclose", depth: "Exact" },
      () => page.locator("#toggle-truth").click());
    expect(navigated.navigation.cursor).toMatchObject({
      place: "Body", aspect: currentTruthAspect.aspect, focus: follow.target_subject, depth: "Exact",
    });
    await expect(page.locator("#deep-inspection")).toBeVisible();

    navigated = await enactNavigation(page, navigationSteps, { kind: "back" },
      () => page.keyboard.press("Escape"));
    expect(navigated.navigation.cursor.depth).toBe("Detail");
    while (JSON.stringify(navigated.navigation.cursor) !== JSON.stringify(journeyStartCursor)) {
      navigated = await enactNavigation(page, navigationSteps, { kind: "back" },
        () => page.locator('[data-navigation-back="true"]').click());
    }
    expect(navigationSteps.length).toBeLessThanOrEqual(16);

    const current = navigated;
    const navigationRequest = operation => ({
      presentation_id: current.presentation.identity,
      presentation_revision: current.presentation.revision,
      navigation_id: current.navigation.navigation.identity,
      operation,
    });
    await refuseNavigation(page, navigationRefusals, { kind: "stale-presentation" }, {
      ...navigationRequest({ kind: "enter", place: "Body" }),
      presentation_revision: current.presentation.revision - 1,
    }, "StalePresentation");
    await refuseNavigation(page, navigationRefusals, { kind: "focus", subject: "subject/absent" },
      navigationRequest({ kind: "focus", subject: "subject/absent" }), "UnknownSubject");
    await refuseNavigation(page, navigationRefusals, { kind: "follow", relationship: followIdentity },
      navigationRequest({ kind: "follow", relationship: followIdentity }), "UnknownRelationship");
    await refuseNavigation(page, navigationRefusals, { kind: "back" },
      navigationRequest({ kind: "back" }), "HistoryExhausted");
    const currentAction = current.presentation.actions.find(({ availability }) => availability === "Available");
    expect(currentAction).toBeTruthy();
    await refuseInvocation(page, navigationRefusals, {
      kind: "invoke", action_id: currentAction.identity, presentation_revision: current.presentation.revision - 1,
    }, {
      presentation_id: current.presentation.identity,
      presentation_revision: current.presentation.revision - 1,
      kind: "invoke",
      action_id: currentAction.identity,
    }, "StalePresentation");
    expect(navigationRefusals).toHaveLength(6);

    const receiptPath = process.env.CONDUIT_PATCHBAY_FRONT_DOOR_RECEIPT_PATH;
    if (receiptPath) {
      const receipt = {
        schema: "conduit.patchbay/zero-body-front-door-capstone@2",
        proof_class: "live-browser",
        browser_engine: "chromium",
        browser_version: browser.version(),
        exact_initial_body: null,
        opened_form_id: form.identity,
        born_body_id: playing.presentation.basis.body_id,
        wake_id: playing.presentation.basis.wake_id,
        revisions: [initial.revision, opened.revision, born.revision, awakened.revision, planned.revision, playing.revision],
        presentation_ids: [
          initial.presentation.identity,
          opened.presentation.identity,
          born.presentation.identity,
          awakened.presentation.identity,
          planned.presentation.identity,
          playing.presentation.identity,
        ],
        plan_id: playing.presentation.basis.plan_id,
        active_play_id: playing.presentation.basis.active_play_id,
        stale_outcome: stale.interaction.last_disposition,
        navigation_journey: {
          schema: "conduit.presentation/navigation-journey-receipt@1",
          maximum_steps: 16,
          start_cursor: journeyStartCursor,
          terminal_cursor: navigated.navigation.cursor,
          followed_relationship: follow,
          steps: navigationSteps,
          refusals: navigationRefusals,
        },
        assertions: {
          no_body_on_entry: true,
          open_is_inert: opened.presentation.basis.body_id === null,
          stale_transition_refused: true,
          explicit_birth_only: true,
          birth_does_not_imply_wake_plan_or_play: true,
          intent_plan_play_preserved_after_birth: true,
          renderer_local_state_excluded_from_semantic_subjects: playing.presentation.subjects.every(
            ({ identity }) => !identity.startsWith("dom/") && !identity.startsWith("window/"),
          ),
          navigate_does_not_act: navigationSteps.every(step =>
            JSON.stringify(step.semantic_basis) === JSON.stringify(semanticBasis(playing))),
          program_body_follow_is_exact: follow.source_subject === upperIdentity
            && follow.target_subject === navigationSteps.find(step => step.operation.kind === "follow").after_cursor.focus,
          body_current_truth_is_reachable: ["Play", "Signs"].includes(currentTruthAspect.aspect),
          detail_and_exact_are_depth: navigationSteps.some(step => step.after_cursor.depth === "Detail")
            && navigationSteps.some(step => step.after_cursor.depth === "Exact"),
          returned_to_program_structure: JSON.stringify(navigated.navigation.cursor)
            === JSON.stringify(journeyStartCursor),
          bounded_explicit_refusals: navigationRefusals.length === 6,
        },
      };
      await mkdir(dirname(receiptPath), { recursive: true });
      const temporary = `${receiptPath}.tmp`;
      await writeFile(temporary, `${JSON.stringify(receipt, null, 2)}\n`);
      await rename(temporary, receiptPath);
    }
  } finally {
    server.lines.close();
    if (server.child.exitCode === null) server.child.kill("SIGTERM");
  }
});
