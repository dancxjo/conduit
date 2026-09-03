import { spawn } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";
import { expect, test } from "@playwright/test";

async function startAuthoringEntrance() {
  const directory = await mkdtemp(join(tmpdir(), "conduit-browser-authoring-"));
  const source = join(directory, "making.conduit");
  await writeFile(source, "form making {\n}\n");
  return { ...(await spawnEntrance(source)), directory, source };
}

async function spawnEntrance(source) {
  const child = spawn("target/debug/patchbay-html", ["--seed", "Empty Form", source], {
    stdio: ["ignore", "pipe", "pipe"],
  });
  const errors = [];
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", chunk => errors.push(chunk));
  const lines = createInterface({ input: child.stdout });
  const url = await new Promise((resolve, reject) => {
    lines.once("line", line => resolve(line.replace("PATCHBAY_HTML_URL=", "")));
    child.once("exit", code => reject(new Error(`Patchbay exited ${code}: ${errors.join("")}`)));
  });
  return { child, errors, url };
}

async function current(page) {
  return page.evaluate(async () => (await fetch("/api/snapshot", { cache: "no-store" })).json());
}

async function selectRole(page, role, name = null) {
  const candidates = page.locator(`#structured-navigator button[data-role="${role}"]`);
  const target = name?.startsWith("subject:")
    ? page.locator(`#structured-navigator button[data-role="${role}"][data-subject*="${name.slice(8)}"]`).first()
    : name ? candidates.filter({ hasText: name }).first() : candidates.first();
  const identity = await target.getAttribute("data-subject");
  await target.click();
  await expect.poll(async () => (await current(page)).navigation.cursor.focus).toBe(identity);
}

async function clickEdit(page, locator) {
  const response = page.waitForResponse(candidate =>
    candidate.url().endsWith("/api/interaction") && candidate.request().method() === "POST");
  await locator.click();
  expect((await response).ok()).toBe(true);
  await expect.poll(async () => (await current(page)).interaction.last_disposition).toBe("Succeeded");
}

async function clickInteraction(page, locator) {
  const response = page.waitForResponse(candidate =>
    candidate.url().endsWith("/api/interaction") && candidate.request().method() === "POST");
  await locator.click();
  const delivered = await response;
  expect(delivered.ok()).toBe(true);
  return delivered.json();
}

async function clickNavigation(page, locator) {
  const response = page.waitForResponse(candidate =>
    candidate.url().endsWith("/api/navigation") && candidate.request().method() === "POST");
  await locator.click();
  const snapshot = await (await response).json();
  expect(snapshot.interaction.last_disposition).toBe("Succeeded");
  return snapshot;
}

test("actual browser entrance authors, saves, plans, and plays one canonical Form", async ({ page }) => {
  test.setTimeout(45_000);
  const server = await startAuthoringEntrance();
  try {
    await page.goto(server.url);
    await page.getByRole("button", { name: "Seeds", exact: true }).click();
    await page.getByRole("button", { name: "Open Seed Empty Form" }).click();
    await expect(page.getByRole("button", { name: "Gears", exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Gears", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Gears · reusable Kinds" })).toBeVisible();
    await expect(page.locator("#gear-results-status")).toContainText("69 of 69 Gears");

    const search = page.getByRole("searchbox", { name: "Find a Gear" });
    await search.fill("text literal");
    const literal = page.getByRole("button", { name: "Place Text literal Gear" });
    await clickEdit(page, literal);
    await clickEdit(page, literal);
    await search.fill("text presentation");
    await clickEdit(page, page.getByRole("button", { name: "Place Text presentation Gear" }));

    let snapshot = await current(page);
    const gears = snapshot.presentation.subjects.filter(subject => subject.role === "Gear");
    expect(gears.map(gear => gear.label).sort()).toEqual(["making/literal", "making/literal-2", "making/text"]);
    expect(new Set(gears.map(gear => gear.label)).size).toBe(3);
    expect(snapshot.presentation.subjects.filter(subject => subject.role === "Port")).toHaveLength(3);

    await page.getByRole("button", { name: "Subjects", exact: true }).click();
    await selectRole(page, "Gear", "making/literal Gear");
    const configure = page.locator('#authoring-actions [data-application-component="form-field"]').filter({ hasText: "Configure value" });
    await configure.locator("input").fill("Browser-authored truth");
    await clickEdit(page, page.locator("#authoring-actions").getByRole("button", { name: "Apply" }));

    await page.locator('#structured-navigator button[data-role="Port"]').nth(0).click();
    await page.getByRole("button", { name: "Start Cord here" }).click();
    await page.locator('#structured-navigator button[data-role="Port"]').nth(2).click();
    await clickEdit(page, page.getByRole("button", { name: "Connect selected output here" }));
    snapshot = await current(page);
    expect(snapshot.presentation.subjects.filter(subject => subject.role === "Cord")).toHaveLength(1);

    await selectRole(page, "Cord");
    await page.getByRole("button", { name: "Reroute one endpoint" }).click();
    await page.locator('#structured-navigator button[data-role="Port"]').nth(1).click();
    await clickEdit(page, page.getByRole("button", { name: "Reroute armed Cord here" }));
    await selectRole(page, "Cord");
    await clickEdit(page, page.getByRole("button", { name: "Remove Cord" }));
    expect((await current(page)).presentation.subjects.filter(subject => subject.role === "Cord")).toHaveLength(0);
    await selectRole(page, "Gear", "making/literal-2 Gear");
    await clickEdit(page, page.getByRole("button", { name: "Remove Gear" }));

    await page.locator('#structured-navigator button[data-role="Port"]').nth(0).click();
    await page.getByRole("button", { name: "Start Cord here" }).click();
    await page.locator('#structured-navigator button[data-role="Port"]').nth(1).click();
    await clickEdit(page, page.getByRole("button", { name: "Connect selected output here" }));
    await selectRole(page, "Form");
    await page.getByRole("button", { name: "SAVE", exact: true }).click();
    await expect.poll(async () => {
      const authoring = (await current(page)).authoring;
      return authoring.saved_revision === authoring.source_revision;
    }).toBe(true);
    const saved = await readFile(server.source, "utf8");
    expect(saved).toContain('literal: text/literal("Browser-authored truth")');
    expect(saved).not.toContain("literal-2:");
    expect(saved).toContain("literal.text > text.text");

    await page.getByRole("button", { name: "Inspector", exact: true }).click();
    await expect(page.locator("body")).toHaveAttribute("data-inspector-open", "false");
    await expect.poll(async () => (await current(page)).navigation.cursor.depth).toBe("Primary");
    await clickNavigation(page, page.getByRole("button", { name: "Entrance", exact: true }));
    await expect(page.locator("body")).toHaveAttribute("data-place", "Entrance");
    await selectRole(page, "Seed", "Empty Form");
    await clickInteraction(page, page.getByRole("button", { name: "BIRTH", exact: true }));
    await expect.poll(async () => Boolean((await current(page)).presentation.basis.body_id)).toBe(true);
    await selectRole(page, "Form", "Current checked and expanded Form");
    await clickInteraction(page, page.getByRole("button", { name: "WAKE", exact: true }));
    await expect.poll(async () => Boolean((await current(page)).presentation.basis.wake_id)).toBe(true);
    await page.getByRole("button", { name: "Subjects", exact: true }).click();
    await clickInteraction(page, page.getByRole("button", { name: "Plan current Form" }));
    await expect(page.locator("#front-door-feedback")).toContainText("Plan Succeeded");
    await clickInteraction(page, page.getByRole("button", { name: "Play current Plan" }));
    await expect(page.locator("#front-door-feedback")).toContainText("Play Succeeded");
    snapshot = await current(page);
    expect(snapshot.presentation.basis.plan_id).toBeTruthy();
    expect(snapshot.presentation.basis.active_play_id).toBeTruthy();
    expect(snapshot.presentation.subjects.some(subject => subject.role === "Sign")).toBe(true);
    await expect(page.locator("#sign")).not.toBeEmpty();

    server.child.kill("SIGTERM");
    await new Promise(resolve => server.child.once("exit", resolve));
    const reopened = await spawnEntrance(server.source);
    server.child = reopened.child;
    await page.goto(reopened.url);
    await page.getByRole("button", { name: "Seeds", exact: true }).click();
    await page.getByRole("button", { name: "Open Seed Empty Form" }).click();
    const restored = await current(page);
    expect(restored.presentation.subjects.filter(subject => subject.role === "Gear").map(subject => subject.label).sort()).toEqual(["making/literal", "making/text"]);
    expect(restored.presentation.subjects.filter(subject => subject.role === "Cord")).toHaveLength(1);
    expect(restored.presentation.properties.some(property => property.name.startsWith("authored-control-") && property.value.Text.includes("Browser-authored truth"))).toBe(true);
  } finally {
    server.child.kill("SIGTERM");
    await rm(server.directory, { recursive: true, force: true });
  }
});
