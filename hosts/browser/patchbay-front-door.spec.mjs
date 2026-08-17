import { spawn } from "node:child_process";
import { mkdir, rename, writeFile } from "node:fs/promises";
import { dirname } from "node:path";
import { createInterface } from "node:readline";
import { expect, test } from "@playwright/test";

function startPublicEntrance() {
  const child = spawn("target/debug/patchbay-html", [
    "--seed", "Text Lab", "examples/text-lab.conduit",
    "--seed", "Hello", "examples/hello.conduit",
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
      seed_id: null,
      body_id: null,
      wake_id: null,
      source_document_id: null,
      checked_form_id: null,
      expanded_form_id: null,
      plan_id: null,
      active_play_id: null,
    });
    expect(initial.presentation.subjects.some(({ role }) => role === "Host")).toBe(true);
    expect(initial.presentation.subjects.some(({ role }) => role === "Seed")).toBe(true);
    expect(initial.presentation.subjects.some(({ role }) => role === "Body")).toBe(false);
    expect(initial.presentation.subjects.some(({ role }) => role === "Part")).toBe(false);
    expect(initial.presentation.properties).toEqual(expect.arrayContaining([
      expect.objectContaining({ name: "current-body", value: { Text: "none" } }),
      expect.objectContaining({ name: "this-host", value: { Flag: true } }),
    ]));

    await page.goto(url);
    await expect(page.getByRole("heading", { name: "Entrance choices" })).toBeVisible();
    await expect(page.locator("body")).toHaveAttribute("data-place", "Entrance");
    await expect(page.locator("#status")).toContainText("Manifestation Available");
    expect(await page.evaluate(() => globalThis.__patchbayMembership)).toBeUndefined();
    expect(await page.locator("#subjects button").evaluateAll((buttons) =>
      buttons.every((button) => !["Body", "Part", "Gear", "Port", "Cord", "Line"].includes(button.dataset.role)),
    )).toBe(true);
    const workspaceBox = await page.locator(".workspace").boundingBox();
    expect(workspaceBox.y + workspaceBox.height).toBeLessThanOrEqual(768);
    await page.getByRole("button", { name: "Seeds", exact: true }).click();
    await expect(page.getByRole("list", { name: "Available Seeds" }).getByRole("button")).toHaveCount(3);
    await expect(page.getByRole("button", { name: "Open Seed Text Lab" })).toBeVisible();
    await page.getByRole("searchbox", { name: "Find a Seed" }).fill("hElLo");
    await expect(page.locator("#seed-results-status")).toHaveText("1 of 3 Seeds available");
    const seed = initial.presentation.subjects.find(({ role, label }) => role === "Seed" && label === "Hello");
    const seedButton = page.getByRole("button", { name: "Open Seed Hello" });
    await page.getByRole("searchbox", { name: "Find a Seed" }).press("ArrowDown");
    await expect(seedButton).toBeFocused();
    const openResponses = [];
    let resolveOpenSequence;
    const openSequence = new Promise((resolve) => { resolveOpenSequence = resolve; });
    page.on("response", (response) => {
      if(["/api/navigation", "/api/interaction"].some(path => response.url().endsWith(path)) && response.request().method() === "POST") {
        openResponses.push(response);
        if(openResponses.length === 2) resolveOpenSequence();
      }
    });
    await Promise.all([openSequence, seedButton.press("Enter")]);
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
      ({ intent, target }) => intent === "conduit.intent/open@1" && target === seed.identity,
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
      expect.objectContaining({ subject: seed.identity, name: "opened", value: { Flag: true } }),
    );
    expect(opened.presentation.subjects).toEqual(expect.arrayContaining([
      expect.objectContaining({ role: "Form" }),
      expect.objectContaining({ role: "Gear", label: "hello/upper" }),
      expect.objectContaining({ role: "Cord" }),
    ]));
    const birthAction = opened.presentation.actions.find(
      ({ intent, target }) => intent === "conduit.intent/birth@1" && target === seed.identity,
    );
    expect(birthAction.identity).toMatch(/^action\/birth\//);

    const exact = page.locator("#inspector .exact-selection");
    await expect(exact).not.toHaveAttribute("open", "");
    await exact.locator("summary").click();
    await expect(exact).toHaveAttribute("open", "");
    await expect(exact).toContainText(seed.identity);
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
    expect(born.presentation.basis.seed_id).toBeTruthy();
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
    { presentationId: opened.presentation.identity, revision: opened.revision, subject: seed.identity });
    expect(stale.interaction.last_disposition).toBe("Refused(StalePresentation)");
    expect(stale.presentation.basis.body_id).toBe(born.presentation.basis.body_id);

    const wakeAction = born.presentation.actions.find(
      ({ intent }) => intent === "conduit.intent/wake@1",
    );
    expect(wakeAction.availability).toBe("Available");
    await page.getByRole("button", { name: "Navigate", exact: true }).click();
    await page.locator(`#subjects button[data-subject="${wakeAction.target}"]`).click();
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

    const receiptPath = process.env.CONDUIT_PATCHBAY_FRONT_DOOR_RECEIPT_PATH;
    if (receiptPath) {
      const receipt = {
        schema: "conduit.patchbay/zero-body-front-door-capstone@1",
        proof_class: "live-browser",
        browser_engine: "chromium",
        browser_version: browser.version(),
        exact_initial_body: null,
        opened_seed_id: seed.identity,
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
