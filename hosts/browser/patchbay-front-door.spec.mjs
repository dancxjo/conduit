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
  return { child, lines, url };
}

test("public browser entrance stays unbodied until OPEN then explicit BE BORN", async ({ browser, page }) => {
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
    await expect(page.getByRole("heading", { name: "Host and Body possibilities" })).toBeVisible();
    await expect(page.locator("#status")).toContainText("Manifestation Available");
    expect(await page.evaluate(() => globalThis.__patchbayMembership)).toBeUndefined();
    expect(await page.locator("#subjects button").evaluateAll((buttons) =>
      buttons.every((button) => ["Host", "Body"].includes(button.dataset.role)),
    )).toBe(true);
    const workspaceBox = await page.locator(".workspace").boundingBox();
    expect(workspaceBox.y + workspaceBox.height).toBeLessThanOrEqual(768);
    await page.getByRole("button", { name: "Seeds", exact: true }).click();
    await expect(page.getByRole("list", { name: "Available Seeds" }).getByRole("button")).toHaveCount(3);
    await page.getByRole("searchbox", { name: "Find a Seed" }).fill("tExT lAb");
    await expect(page.locator("#seed-results-status")).toHaveText("1 of 3 Seeds available");
    const seed = initial.presentation.subjects.find(({ role, label }) => role === "Seed" && label === "Text Lab");
    const seedButton = page.getByRole("button", { name: "Open Seed Text Lab" });
    await page.getByRole("searchbox", { name: "Find a Seed" }).press("ArrowDown");
    await expect(seedButton).toBeFocused();
    const openResponses = [];
    let resolveOpenSequence;
    const openSequence = new Promise((resolve) => { resolveOpenSequence = resolve; });
    page.on("response", (response) => {
      if(response.url().endsWith("/api/interaction") && response.request().method() === "POST") {
        openResponses.push(response);
        if(openResponses.length === 2) resolveOpenSequence();
      }
    });
    await Promise.all([openSequence, seedButton.press("Enter")]);
    expect(openResponses).toHaveLength(2);
    await expect(page.locator("body")).toHaveAttribute("data-inspector-open", "true");
    const beBornButton = page.getByRole("button", { name: "BE BORN" });
    await expect(beBornButton).toBeVisible();
    await expect(beBornButton).toBeEnabled();
    expect(openResponses.every((response) => response.ok())).toBe(true);
    await expect(page.getByRole("button", { name: "OPEN", exact: true })).toBeEnabled();
    const openAction = initial.presentation.actions.find(
      ({ intent, target }) => intent === "conduit.intent/open@1" && target === seed.identity,
    );
    const opened = await (await fetch(`${url}/api/snapshot`)).json();
    expect(opened.interaction.last_request_id).toMatch(/^patchbay\/interaction\/invoke\//);
    expect(opened.interaction.last_disposition).toBe("Succeeded");
    expect(openAction.identity).toMatch(/^action\/open\//);
    expect(opened.revision).toBe(initial.revision + 1);
    expect(opened.presentation.basis.body_id).toBeNull();
    expect(opened.parts).toBeUndefined();
    expect(opened.presentation.properties).toContainEqual(
      expect.objectContaining({ subject: seed.identity, name: "opened", value: { Flag: true } }),
    );
    expect(opened.presentation.subjects).toEqual(expect.arrayContaining([
      expect.objectContaining({ role: "Form" }),
      expect.objectContaining({ role: "Gear", label: "text-lab/uppercase" }),
      expect.objectContaining({ role: "Cord" }),
    ]));

    const [birthResponse] = await Promise.all([
      page.waitForResponse(
        (response) => response.url().endsWith("/api/interaction") && response.request().method() === "POST",
      ),
      beBornButton.click(),
    ]);
    expect(birthResponse.ok()).toBe(true);
    await expect(page.getByRole("heading", { name: "Live Body topology" })).toBeVisible();
    const born = await (await fetch(`${url}/api/snapshot`)).json();
    expect(born.interaction.last_request_id).toMatch(/^patchbay\/interaction\/invoke\//);
    expect(born.interaction.last_disposition).toBe("Succeeded");
    expect(born.presentation.basis.body_id).toBeTruthy();
    expect(born.presentation.basis.seed_id).toBeTruthy();
    expect(born.presentation.basis.wake_id).toBeTruthy();
    expect(born.parts.parts).toHaveLength(1);
    expect(born.parts.parts[0].state).toBe("Here");
    expect(born.presentation.subjects.some(({ role }) => role === "Form")).toBe(true);

    const exact = page.locator("#inspector .exact-selection");
    await expect(exact).not.toHaveAttribute("open", "");
    await exact.locator("summary").click();
    await expect(exact).toHaveAttribute("open", "");
    await expect(exact).toContainText(seed.identity);
    await exact.locator("summary").click();
    await expect(exact).not.toHaveAttribute("open", "");

    const stale = await page.evaluate(async ({ presentationId, revision, subject }) =>
      (await fetch("/api/front-door-transition", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          presentation_id: presentationId,
          revision,
          action: "be-born",
          subject,
        }),
      })).json(),
    { presentationId: opened.presentation.identity, revision: opened.revision, subject: seed.identity });
    expect(stale.interaction.last_disposition).toBe("Refused(StalePresentation)");
    expect(stale.presentation.basis.body_id).toBe(born.presentation.basis.body_id);

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
        revisions: [initial.revision, opened.revision, born.revision, planned.revision, playing.revision],
        presentation_ids: [
          initial.presentation.identity,
          opened.presentation.identity,
          born.presentation.identity,
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
          explicit_be_born_only: true,
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
