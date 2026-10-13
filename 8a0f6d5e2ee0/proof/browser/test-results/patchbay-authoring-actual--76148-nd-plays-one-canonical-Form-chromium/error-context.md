# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: patchbay-authoring.spec.mjs >> actual browser entrance authors, saves, plans, and plays one canonical Form
- Location: proof/browser/patchbay-authoring.spec.mjs:72:1

# Error details

```
Error: expect(locator).toContainText(expected) failed

Locator: locator('#gear-results-status')
Expected substring: "74 of 74 Gears"
Received string:    "0 of 0 Gears available from the canonical catalog"
Timeout: 5000ms

Call log:
  - Expect "toContainText" with timeout 5000ms
  - waiting for locator('#gear-results-status')
    14 × locator resolved to <div id="gear-results-status" data-application-revision="1" data-application-slot="gear-results-status">…</div>
       - unexpected value "0 of 0 Gears available from the canonical catalog"

```

```yaml
- status: 0 of 0 Gears available from the canonical catalog
```

# Test source

```ts
  1   | import { spawn } from "node:child_process";
  2   | import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
  3   | import { tmpdir } from "node:os";
  4   | import { join } from "node:path";
  5   | import { createInterface } from "node:readline";
  6   | import { expect, test } from "@playwright/test";
  7   | 
  8   | async function startAuthoringEntrance() {
  9   |   const directory = await mkdtemp(join(tmpdir(), "conduit-browser-authoring-"));
  10  |   const source = join(directory, "making.conduit");
  11  |   await writeFile(source, "form making {\n}\n");
  12  |   return { ...(await spawnEntrance(source)), directory, source };
  13  | }
  14  | 
  15  | async function spawnEntrance(source) {
  16  |   const child = spawn("target/debug/patchbay-html", ["--form", "Empty Form", source], {
  17  |     stdio: ["ignore", "pipe", "pipe"],
  18  |   });
  19  |   const errors = [];
  20  |   child.stderr.setEncoding("utf8");
  21  |   child.stderr.on("data", chunk => errors.push(chunk));
  22  |   const lines = createInterface({ input: child.stdout });
  23  |   const url = await new Promise((resolve, reject) => {
  24  |     lines.once("line", line => resolve(line.replace("PATCHBAY_HTML_URL=", "")));
  25  |     child.once("exit", code => reject(new Error(`Patchbay exited ${code}: ${errors.join("")}`)));
  26  |   });
  27  |   return { child, errors, url };
  28  | }
  29  | 
  30  | async function current(page) {
  31  |   return page.evaluate(async () => (await fetch("/api/snapshot", { cache: "no-store" })).json());
  32  | }
  33  | 
  34  | async function selectRole(page, role, name = null) {
  35  |   await page.locator("#structured-navigator").evaluate(element => { element.closest("details").open = true; });
  36  |   const candidates = page.locator('#structured-navigator [data-application-component="choice-option-label"]')
  37  |     .filter({ has: page.locator(`input[type="radio"][data-role="${role}"]`) });
  38  |   const target = name?.startsWith("subject:")
  39  |     ? page.locator(`#structured-navigator input[type="radio"][data-role="${role}"][data-subject*="${name.slice(8)}"]`).first()
  40  |     : (name ? candidates.filter({ hasText: name }).first() : candidates.first()).locator('input[type="radio"]');
  41  |   const identity = await target.getAttribute("data-subject");
  42  |   await target.click();
  43  |   await expect.poll(async () => (await current(page)).navigation.cursor.focus).toBe(identity);
  44  | }
  45  | 
  46  | async function clickEdit(page, locator) {
  47  |   const response = page.waitForResponse(candidate =>
  48  |     candidate.url().endsWith("/api/interaction") && candidate.request().method() === "POST");
  49  |   await locator.click();
  50  |   expect((await response).ok()).toBe(true);
  51  |   await expect.poll(async () => (await current(page)).interaction.last_disposition).toBe("Succeeded");
  52  | }
  53  | 
  54  | async function clickInteraction(page, locator) {
  55  |   const response = page.waitForResponse(candidate =>
  56  |     candidate.url().endsWith("/api/interaction") && candidate.request().method() === "POST");
  57  |   await locator.click();
  58  |   const delivered = await response;
  59  |   expect(delivered.ok()).toBe(true);
  60  |   return delivered.json();
  61  | }
  62  | 
  63  | async function clickNavigation(page, locator) {
  64  |   const response = page.waitForResponse(candidate =>
  65  |     candidate.url().endsWith("/api/navigation") && candidate.request().method() === "POST");
  66  |   await locator.press("Enter");
  67  |   const snapshot = await (await response).json();
  68  |   expect(snapshot.interaction.last_disposition).toBe("Succeeded");
  69  |   return snapshot;
  70  | }
  71  | 
  72  | test("actual browser entrance authors, saves, plans, and plays one canonical Form", async ({ page }) => {
  73  |   test.setTimeout(45_000);
  74  |   const server = await startAuthoringEntrance();
  75  |   try {
  76  |     await page.goto(server.url);
  77  |     await page.getByRole("button", { name: "Open Form Empty Form" }).click();
  78  |     await expect(page.getByRole("heading", { name: "Gears · reusable Kinds" })).toBeVisible();
> 79  |     await expect(page.locator("#gear-results-status")).toContainText("74 of 74 Gears");
      |                                                        ^ Error: expect(locator).toContainText(expected) failed
  80  | 
  81  |     const search = page.getByRole("searchbox", { name: "Find Forms, Gears, and Parts" });
  82  |     await search.fill("text literal");
  83  |     const literal = page.getByRole("button", { name: "Place Text literal Gear" });
  84  |     await clickEdit(page, literal);
  85  |     await clickEdit(page, literal);
  86  |     await search.fill("text presentation");
  87  |     await clickEdit(page, page.getByRole("button", { name: "Place Text presentation Gear" }));
  88  | 
  89  |     let snapshot = await current(page);
  90  |     const gears = snapshot.presentation.subjects.filter(subject => subject.role === "Gear");
  91  |     expect(gears.map(gear => gear.label).sort()).toEqual(["making/literal", "making/literal-2", "making/text"]);
  92  |     expect(new Set(gears.map(gear => gear.label)).size).toBe(3);
  93  |     expect(snapshot.presentation.subjects.filter(subject => subject.role === "Port")).toHaveLength(3);
  94  | 
  95  |     await selectRole(page, "Gear", "making/literal Gear");
  96  |     const configure = page.locator('#authoring-actions [data-application-component="form-field"]').filter({ hasText: "Configure value" });
  97  |     await configure.locator("input").fill("Browser-authored truth");
  98  |     await clickEdit(page, page.locator("#authoring-actions").getByRole("button", { name: "Apply" }));
  99  | 
  100 |     await page.locator('#structured-navigator input[type="radio"][data-role="Port"]').nth(0).click();
  101 |     await page.getByRole("button", { name: "Start Cord here" }).click();
  102 |     await page.locator('#structured-navigator input[type="radio"][data-role="Port"]').nth(2).click();
  103 |     await clickEdit(page, page.getByRole("button", { name: "Connect selected output here" }));
  104 |     snapshot = await current(page);
  105 |     expect(snapshot.presentation.subjects.filter(subject => subject.role === "Cord")).toHaveLength(1);
  106 | 
  107 |     await selectRole(page, "Cord");
  108 |     await page.getByRole("button", { name: "Reroute one endpoint" }).click();
  109 |     await page.locator('#structured-navigator input[type="radio"][data-role="Port"]').nth(1).click();
  110 |     await clickEdit(page, page.getByRole("button", { name: "Reroute armed Cord here" }));
  111 |     await selectRole(page, "Cord");
  112 |     await clickEdit(page, page.getByRole("button", { name: "Remove Cord" }));
  113 |     expect((await current(page)).presentation.subjects.filter(subject => subject.role === "Cord")).toHaveLength(0);
  114 |     await selectRole(page, "Gear", "making/literal-2 Gear");
  115 |     await clickEdit(page, page.getByRole("button", { name: "Remove Gear" }));
  116 | 
  117 |     await page.locator('#structured-navigator input[type="radio"][data-role="Port"]').nth(0).click();
  118 |     await page.getByRole("button", { name: "Start Cord here" }).click();
  119 |     await page.locator('#structured-navigator input[type="radio"][data-role="Port"]').nth(1).click();
  120 |     await clickEdit(page, page.getByRole("button", { name: "Connect selected output here" }));
  121 |     await selectRole(page, "Form");
  122 |     await page.getByRole("button", { name: "SAVE", exact: true }).click();
  123 |     await expect.poll(async () => {
  124 |       const authoring = (await current(page)).authoring;
  125 |       return authoring.saved_revision === authoring.source_revision;
  126 |     }).toBe(true);
  127 |     const saved = await readFile(server.source, "utf8");
  128 |     expect(saved).toContain('literal: text/literal("Browser-authored truth")');
  129 |     expect(saved).not.toContain("literal-2:");
  130 |     expect(saved).toContain("literal.text >> text.text");
  131 | 
  132 |     await page.getByRole("button", { name: "Inspect", exact: true }).click();
  133 |     await expect(page.locator("body")).toHaveAttribute("data-inspector-open", "false");
  134 |     await expect.poll(async () => (await current(page)).navigation.cursor.depth).toBe("Primary");
  135 |     await clickNavigation(page, page.getByRole("button", { name: "Entrance", exact: true }));
  136 |     await expect(page.locator("body")).toHaveAttribute("data-place", "Entrance");
  137 |     await page.getByRole("button", { name: "Inspect", exact: true }).click();
  138 |     await selectRole(page, "Form", "Empty Form");
  139 |     await clickInteraction(page, page.getByRole("button", { name: "BIRTH", exact: true }));
  140 |     await expect.poll(async () => Boolean((await current(page)).presentation.basis.body_id)).toBe(true);
  141 |     await selectRole(page, "Form", "Current checked and expanded Form");
  142 |     await clickInteraction(page, page.getByRole("button", { name: "WAKE", exact: true }));
  143 |     await expect.poll(async () => Boolean((await current(page)).presentation.basis.wake_id)).toBe(true);
  144 |     await page.locator("#structured-navigator").evaluate(element => { element.closest("details").open = true; });
  145 |     await clickInteraction(page, page.getByRole("button", { name: "Plan current form" }));
  146 |     await expect(page.locator("#front-door-feedback")).toContainText("Plan Succeeded");
  147 |     await clickInteraction(page, page.getByRole("button", { name: "Play current plan" }));
  148 |     await expect(page.locator("#front-door-feedback")).toContainText("Play Succeeded");
  149 |     snapshot = await current(page);
  150 |     expect(snapshot.presentation.basis.plan_id).toBeTruthy();
  151 |     expect(snapshot.presentation.basis.active_play_id).toBeTruthy();
  152 |     expect(snapshot.presentation.subjects.some(subject => subject.role === "Sign")).toBe(true);
  153 |     await expect(page.locator("#sign")).not.toBeEmpty();
  154 | 
  155 |     server.child.kill("SIGTERM");
  156 |     await new Promise(resolve => server.child.once("exit", resolve));
  157 |     const reopened = await spawnEntrance(server.source);
  158 |     server.child = reopened.child;
  159 |     await page.goto(reopened.url);
  160 |     await page.getByRole("button", { name: "Open Form Empty Form" }).click();
  161 |     const restored = await current(page);
  162 |     expect(restored.presentation.subjects.filter(subject => subject.role === "Gear").map(subject => subject.name).sort()).toEqual(["Gear making/literal", "Gear making/text"]);
  163 |     expect(restored.presentation.subjects.filter(subject => subject.role === "Cord")).toHaveLength(1);
  164 |     expect(restored.presentation.properties.some(property => property.name.startsWith("authored-control-") && property.value.Text.includes("Browser-authored truth"))).toBe(true);
  165 |   } finally {
  166 |     server.child.kill("SIGTERM");
  167 |     await rm(server.directory, { recursive: true, force: true });
  168 |   }
  169 | });
  170 | 
```