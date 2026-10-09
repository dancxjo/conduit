import { spawn } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";
import { expect, test } from "@playwright/test";

async function startAuthoringEntrance(plotSource = "plot making {\n}\n") {
  const directory = await mkdtemp(join(tmpdir(), "conduit-browser-authoring-"));
  const source = join(directory, "making.conduit");
  await writeFile(source, plotSource);
  return { ...(await spawnEntrance(source)), directory, source };
}

test("catalog queries and two durable layouts decorate the same live Plot", async ({ page }) => {
    test.setTimeout(90_000);
    const source = `plot authoring {
      literal: text/literal("authoring truth")
      prefix: text/join("First: ")
      upper: text/upper
      label: text/join("Result: ")
      display: presentation/text(maximum-values = 4)
      scalar: scalar/literal(value = 500000)
      map: math/map-quantity
      quantity: presentation/quantity
      wrapped: structured-info/wrap-quantity
      literal >> prefix >> upper >> label >> display
      scalar.value >> map.in
      map >> wrapped >> quantity
    }\n`;
    const server = await startAuthoringEntrance(source);
    try {
      await page.goto(server.url);
      await page.getByRole("button", { name: "Open Plot Empty Plot" }).click();
      await expect(page.locator(".flow-frontplate.role-gear")).toHaveCount(9);
      const initial = await current(page);
      const basis = initial.authoring.checked_plot_id;
      const semanticGears = initial.presentation.subjects.filter(subject => subject.role === "Gear")
        .map(subject => initial.presentation.properties.find(property => property.subject === subject.identity
          && property.name === "semantic-id").value.Identity).sort();
      expect(initial.presentation.basis.plan_id).toBeNull();
      const mapping = initial.authoring.palette.find(entry => entry.kind_id === "math/map-quantity");
      expect(mapping.authorable).toBe(true);
      expect(mapping.kind_contract_revision).toBeTruthy();
      expect(mapping.configuration.length).toBeGreaterThan(4);
      expect(mapping.front).toBeTruthy();
      const distinctBases = await page.evaluate(async basis => {
        const { authoringBasis } = await import("/assets/authoring.js");
        return [
          authoringBasis({ ...basis, source_document_id: `${basis.source_document_id}/other` }),
          authoringBasis({ ...basis, checked_plot_id: `${basis.checked_plot_id}/other` }),
          authoringBasis({ ...basis, expanded_plot_id: `${basis.expanded_plot_id}/other` }),
          authoringBasis(basis),
        ];
      }, initial.authoring);
      expect(new Set(distinctBases).size).toBe(4);

      const portIdentity = (snapshot, gear, direction) => {
        const subject = snapshot.presentation.subjects.find(subject => subject.role === "Gear"
          && snapshot.presentation.properties.some(property => property.subject === subject.identity
            && property.name === "semantic-id" && property.value.Identity === `gear/authoring/${gear}`));
        const ports = snapshot.presentation.relationships.filter(relation => relation.kind === "Contains" && relation.source === subject.identity);
        const port = ports.find(relation => snapshot.presentation.properties.some(property =>
          property.subject === relation.target && property.name === "direction" && property.value.Text === direction));
        return snapshot.presentation.properties.find(property => property.subject === port.target && property.name === "semantic-id").value.Identity;
      };
      const queries = await page.evaluate(async ({ sourcePort, sinkPort }) => {
        const snapshot = await (await fetch("/api/snapshot")).json();
        const response = await fetch("/api/authoring-query", { method: "POST", headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ query: "connections", revision: snapshot.authoring.source_revision, expanded_plot_id: snapshot.authoring.expanded_plot_id, source: sourcePort }) });
        return (await response.json()).candidates.find(candidate => candidate.sink_identity === sinkPort);
      }, { sourcePort: portIdentity(initial, "map", "outgoing"), sinkPort: portIdentity(initial, "quantity", "receiving") });
      expect(queries.compatible).toBe(false);
      expect(queries.diagnostic).toBeTruthy();
      expect(queries.adapters.map(adapter => adapter.kind_id)).toContain("structured-info/wrap-quantity");
      expect((await current(page)).authoring.checked_plot_id).toBe(basis);

      await selectRole(page, "Gear", "authoring/map Gear");
      const field = page.getByRole("textbox", { name: "Configure target-granularity", exact: true });
      await expect(field).toHaveValue(String(mapping.configuration.find(field => field.key === "target-granularity").default_value.I64));
      await field.fill("9223372036854775808");
      const configuration = page.locator('#authoring-actions [data-application-component="plot-field"]').filter({ hasText: "Configure target-granularity" });
      await configuration.getByRole("button", { name: "Apply", exact: true }).click();
      await expect(configuration.getByRole("status")).not.toBeEmpty();
      expect((await current(page)).authoring.checked_plot_id).toBe(basis);

      const workspace = page.locator("#workspace-controls");
      await workspace.locator("summary").click();
      await page.getByRole("textbox", { name: "Layout name", exact: true }).fill("Teaching");
      await page.getByRole("button", { name: "Save layout", exact: true }).click();
      await expect(page.locator("#workspace-status")).toContainText("Ready");
      await page.getByRole("textbox", { name: "Workspace annotation", exact: true }).fill("This frame is not executable scope");
      await page.getByRole("button", { name: "Add note", exact: true }).click();
      await page.getByRole("button", { name: "Frame selected Gears", exact: true }).click();
      await page.getByRole("button", { name: "Export workspace", exact: true }).click();
      const first = JSON.parse(await page.getByRole("textbox", { name: "Workspace document", exact: true }).inputValue());
      expect(first.basis.checked_plot_id).toBe(basis);
      expect(first.layouts.find(layout => layout.name === "Teaching").notes).toHaveLength(1);

      // Import is an explicit presentation operation; every Gear gets a radically different position.
      const second = structuredClone(first);
      const alternate = structuredClone(second.layouts.find(layout => layout.name === "Teaching"));
      alternate.name = "Wide";
      alternate.notes[0].id = "note-2";
      alternate.positions.forEach((position, index) => { position.x = index * 1800 - 6000; position.y = index % 2 ? 4000 : -4000; });
      alternate.viewport = { x: 100, y: -100, zoom: 0.25 };
      second.layouts.push(alternate);
      second.active_layout = "Wide";
      await page.getByRole("textbox", { name: "Workspace document", exact: true }).fill(JSON.stringify(second));
      await page.getByRole("button", { name: "Import workspace", exact: true }).click();
      await expect(page.getByRole("combobox", { name: "Saved layouts", exact: true })).toHaveValue("Wide");
      await page.getByRole("textbox", { name: "Workspace annotation", exact: true }).fill("A second independent annotation");
      await page.getByRole("button", { name: "Add note", exact: true }).click();
      await page.getByRole("button", { name: "Export workspace", exact: true }).click();
      const annotated = JSON.parse(await page.getByRole("textbox", { name: "Workspace document", exact: true }).inputValue());
      expect(annotated.layouts.find(layout => layout.name === "Wide").notes.map(note => note.text).sort())
        .toEqual(["A second independent annotation", "This frame is not executable scope"]);
      expect((await current(page)).authoring.checked_plot_id).toBe(basis);
      expect(await readFile(server.source, "utf8")).toBe(source);
      await workspace.locator("summary").click();

      await clickNavigation(page, page.getByRole("button", { name: "Entrance", exact: true }));
      await selectRole(page, "Plot", "Empty Plot");
      await clickInteraction(page, page.getByRole("button", { name: "BIRTH", exact: true }));
      await selectRole(page, "Plot", "Current checked and expanded Plot");
      await clickInteraction(page, page.getByRole("button", { name: "WAKE", exact: true }));
      await clickInteraction(page, page.getByRole("button", { name: "Plan current plot" }));
      const planned = await current(page);
      expect(planned.presentation.basis.plan_id).toBeTruthy();
      await clickInteraction(page, page.getByRole("button", { name: "Play current plan" }));
      const live = await current(page);
      expect(live.presentation.basis.active_play_id).toBeTruthy();
      expect(live.presentation.subjects.some(subject => subject.role === "Sign")).toBe(true);
      const plan = live.presentation.basis.plan_id;
      const play = live.presentation.basis.active_play_id;
      await clickNavigation(page, page.getByRole("button", { name: "Plot", exact: true }));
      await workspace.locator("summary").click();
      for (const layout of ["Teaching", "Wide"]) {
        await page.getByRole("combobox", { name: "Saved layouts", exact: true }).selectOption(layout);
        await page.getByRole("button", { name: "Use layout", exact: true }).click();
        const after = await current(page);
        expect(after.presentation.basis.plan_id).toBe(plan);
        expect(after.presentation.basis.active_play_id).toBe(play);
        expect(after.presentation.basis.checked_plot_id).toBe(basis);
        const positions = annotated.layouts.find(saved => saved.name === layout).positions
          .filter(position => semanticGears.includes(position.subject))
          .sort((left, right) => left.subject.localeCompare(right.subject));
        const gearSubjects = initial.presentation.subjects.filter(subject => subject.role === "Gear")
          .map(subject => [subject.identity, initial.presentation.properties.find(property =>
            property.subject === subject.identity && property.name === "semantic-id").value.Identity]);
        // The admitted application uses blob module identities. Importing the
        // raw flow asset would inspect a separate, unused module instance.
        await expect.poll(() => page.locator(".react-flow__node").evaluateAll((nodes, subjects) => {
          const semanticIds = new Map(subjects);
          return nodes.filter(node => node.querySelector(".flow-frontplate.role-gear"))
            .map(node => {
              const position = new DOMMatrixReadOnly(node.style.transform);
              return { subject: semanticIds.get(node.dataset.id), x: position.m41, y: position.m42 };
            })
            .sort((left, right) => left.subject.localeCompare(right.subject));
        }, gearSubjects)).toEqual(positions);
        for (const aspect of ["Plan", "Play", "Signs"]) {
          const button = page.locator(`#aspect-controls button[data-aspect="${aspect}"]`);
          await expect(button).toBeVisible();
          const projected = await clickNavigation(page, button);
          expect(projected.presentation.basis.plan_id).toBe(plan);
          expect(projected.presentation.basis.active_play_id).toBe(play);
          const gears = projected.presentation.subjects.filter(subject => subject.role === "Gear");
          expect(gears.map(subject => projected.presentation.properties.find(property =>
            property.subject === subject.identity && property.name === "semantic-id").value.Identity).sort())
            .toEqual(semanticGears);
          await expect(page.locator(".flow-frontplate.role-gear")).toHaveCount(9);
          await expect(page.locator(".flow-frontplate.role-gear").first())
            .toHaveAttribute("data-lens", aspect.toLowerCase());
          for (const gear of gears) {
            const facts = projected.presentation.properties.filter(property => property.subject === gear.identity);
            const clue = page.locator(".flow-frontplate.role-gear")
              .filter({ has: page.locator(`input[data-subject="${gear.identity}"]`) }).locator(".faceplate-clue");
            if (aspect === "Plan") {
              await expect(clue).toHaveText(facts.find(property => property.name === "host-id").value.Identity);
            } else if (aspect === "Play") {
              await expect(clue).toContainText(facts.find(property => property.name === "play-state").value.Text);
            } else {
              await expect(clue).toHaveText(`${facts.filter(property => property.name.startsWith("sign-")).length} causal Signs`);
            }
          }
        }
      }
    } finally {
      server.child.kill("SIGTERM");
      await rm(server.directory, { recursive: true, force: true });
    }
});

async function spawnEntrance(source) {
  const child = spawn("target/debug/conduit-browser-patchbay-workbench", ["--plot", "Empty Plot", source], {
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
  await page.locator("#structured-navigator").evaluate(element => { element.closest("details").open = true; });
  const candidates = page.locator('#structured-navigator [data-application-component="choice-option-label"]')
    .filter({ has: page.locator(`input[type="radio"][data-role="${role}"]`) });
  const target = name?.startsWith("subject:")
    ? page.locator(`#structured-navigator input[type="radio"][data-role="${role}"][data-subject*="${name.slice(8)}"]`).first()
    : (name ? candidates.filter({ hasText: name }).first() : candidates.first()).locator('input[type="radio"]');
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
  await locator.press("Enter");
  const snapshot = await (await response).json();
  expect(snapshot.interaction.last_disposition).toBe("Succeeded");
  return snapshot;
}

test("actual browser entrance authors, saves, plans, and plays one canonical Plot", async ({ page }) => {
  test.setTimeout(45_000);
  const server = await startAuthoringEntrance();
  try {
    await page.goto(server.url);
    await page.getByRole("button", { name: "Open Plot Empty Plot" }).click();
    await expect(page.getByRole("heading", { name: "Gears · reusable Kinds" })).toBeVisible();
    await expect(page.locator("#gear-results-status")).toContainText(/\d+ of \d+ Gears available/);

    const search = page.getByRole("searchbox", { name: "Find Plots, Gears, and Parts" });
    await search.fill("text literal");
    const literal = page.getByRole("button", { name: "Place Text literal Gear" });
    await clickEdit(page, literal);
    await clickEdit(page, literal);
    await search.fill("text presentation");
    await clickEdit(page, page.getByRole("button", { name: "Place Text presentation Gear" }));

    let snapshot = await current(page);
    const gears = snapshot.presentation.subjects.filter(subject => subject.role === "Gear");
    expect(gears.map(gear => gear.name)).toEqual(expect.arrayContaining([
      expect.stringContaining("making/literal"),
      expect.stringContaining("making/literal-2"),
      expect.stringContaining("making/text"),
    ]));
    expect(new Set(gears.map(gear => gear.name)).size).toBe(3);
    expect(snapshot.presentation.subjects.filter(subject => subject.role === "Port")).toHaveLength(3);

    await selectRole(page, "Gear", "making/literal Gear");
    await page.getByRole("textbox", { name: "Configure value" }).fill("Browser-authored truth");
    await clickEdit(page, page.locator("#authoring-actions").getByRole("button", { name: "Apply" }));

    await page.locator('#structured-navigator input[type="radio"][data-role="Port"]').nth(0).click();
    await page.getByRole("button", { name: "Start Cord here" }).click();
    await page.locator('#structured-navigator input[type="radio"][data-role="Port"]').nth(2).click();
    await clickEdit(page, page.getByRole("button", { name: "Connect selected output here" }));
    snapshot = await current(page);
    expect(snapshot.presentation.subjects.filter(subject => subject.role === "Cord")).toHaveLength(1);

    await selectRole(page, "Cord");
    await page.getByRole("button", { name: "Reroute one endpoint" }).click();
    await page.locator('#structured-navigator input[type="radio"][data-role="Port"]').nth(1).click();
    await clickEdit(page, page.getByRole("button", { name: "Reroute armed Cord here" }));
    await selectRole(page, "Cord");
    await clickEdit(page, page.getByRole("button", { name: "Remove Cord" }));
    expect((await current(page)).presentation.subjects.filter(subject => subject.role === "Cord")).toHaveLength(0);
    await selectRole(page, "Gear", "making/literal-2 Gear");
    await clickEdit(page, page.getByRole("button", { name: "Remove Gear" }));

    await page.locator('#structured-navigator input[type="radio"][data-role="Port"]').nth(0).click();
    await page.getByRole("button", { name: "Start Cord here" }).click();
    await page.locator('#structured-navigator input[type="radio"][data-role="Port"]').nth(1).click();
    await clickEdit(page, page.getByRole("button", { name: "Connect selected output here" }));
    await selectRole(page, "Plot");
    await page.getByRole("button", { name: "SAVE", exact: true }).click();
    await expect.poll(async () => {
      const authoring = (await current(page)).authoring;
      return authoring.saved_revision === authoring.source_revision;
    }).toBe(true);
    const saved = await readFile(server.source, "utf8");
    expect(saved).toContain('literal: text/literal("Browser-authored truth")');
    expect(saved).not.toContain("literal-2:");
    expect(saved).toContain("literal.text >> text.text");

    await page.getByRole("button", { name: "Inspect", exact: true }).click();
    await expect(page.locator("body")).toHaveAttribute("data-inspector-open", "false");
    await expect.poll(async () => (await current(page)).navigation.cursor.depth).toBe("Primary");
    await clickNavigation(page, page.getByRole("button", { name: "Entrance", exact: true }));
    await expect(page.locator("body")).toHaveAttribute("data-place", "Entrance");
    await page.getByRole("button", { name: "Inspect", exact: true }).click();
    await selectRole(page, "Plot", "Empty Plot");
    await clickInteraction(page, page.getByRole("button", { name: "BIRTH", exact: true }));
    await expect.poll(async () => Boolean((await current(page)).presentation.basis.body_id)).toBe(true);
    await selectRole(page, "Plot", "Current checked and expanded Plot");
    await clickInteraction(page, page.getByRole("button", { name: "WAKE", exact: true }));
    await expect.poll(async () => Boolean((await current(page)).presentation.basis.wake_id)).toBe(true);
    await page.locator("#structured-navigator").evaluate(element => { element.closest("details").open = true; });
    await clickInteraction(page, page.getByRole("button", { name: "Plan current plot" }));
    await expect(page.locator("#front-door-feedback")).toContainText("Plan Succeeded");
    await clickInteraction(page, page.getByRole("button", { name: "Play current plan" }));
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
    await page.getByRole("button", { name: "Open Plot Empty Plot" }).click();
    const restored = await current(page);
    expect(restored.presentation.subjects.filter(subject => subject.role === "Gear").map(subject => subject.name).sort()).toEqual([
      "making/literal Gear, text/literal",
      "making/text Gear, presentation/text",
    ]);
    expect(restored.presentation.subjects.filter(subject => subject.role === "Cord")).toHaveLength(1);
    expect(restored.presentation.properties.some(property => property.name.startsWith("authored-control-") && property.value.Text.includes("Browser-authored truth"))).toBe(true);
  } finally {
    server.child.kill("SIGTERM");
    await rm(server.directory, { recursive: true, force: true });
  }
});
