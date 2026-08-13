import { spawn } from "node:child_process";
import { mkdir, rename, writeFile } from "node:fs/promises";
import { dirname } from "node:path";
import { createInterface } from "node:readline";
import { expect, test } from "@playwright/test";

function startPublicEntrance() {
  const child = spawn("target/debug/patchbay-html", [], {
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

test("public browser entrance begins at live canonical Body truth", async ({ browser, page }) => {
  const server = startPublicEntrance();
  try {
    const url = await server.url;
    const snapshot = await (await fetch(`${url}/api/snapshot`)).json();
    expect(snapshot.entrance.layer).toBe("World");
    expect(snapshot.entrance.selected_subject).toMatch(/^part\//);
    expect(snapshot.parts.parts).toHaveLength(1);
    expect(snapshot.parts.wants_to_join).toHaveLength(0);
    expect(snapshot.presentation.basis.plan_id).toBeNull();
    expect(snapshot.presentation.basis.active_play_id).toBeNull();
    expect(snapshot.presentation.subjects.some((subject) => subject.role === "Body")).toBe(true);
    expect(snapshot.presentation.subjects.some((subject) => subject.role === "Part")).toBe(true);
    expect(snapshot.presentation.subjects.some((subject) => subject.role === "Host")).toBe(true);
    expect(snapshot.presentation.subjects.some((subject) => subject.role === "Capability")).toBe(true);
    expect(snapshot.presentation.subjects.some((subject) => subject.role === "Form")).toBe(true);
    expect(snapshot.presentation.subjects.every((subject) =>
      !subject.label.includes("Pico") && !subject.label.includes("tab 3"),
    )).toBe(true);

    await page.goto(url);
    await expect(page.getByRole("heading", { name: "Live Body topology" })).toBeVisible();
    await expect.poll(() => page.evaluate(() => globalThis.__patchbayMembership?.state() ?? "starting"))
      .toBe("admitted");
    const attached = await (await fetch(`${url}/api/snapshot`)).json();
    expect(attached.parts.parts).toHaveLength(2);
    await page.evaluate(() => globalThis.patchbayReload());
    await expect(page.getByRole("list", { name: "Body Parts" }).getByRole("listitem")).toHaveCount(2);
    await expect(page.getByRole("list", { name: "Body Parts" })).toContainText("HERE · AVAILABLE");
    await expect(page.getByRole("list", { name: "Body Parts" })).toContainText("ATTACHED · AVAILABLE");
    await expect(page.locator("body")).toHaveAttribute("data-lens", "world");
    await expect(page.locator("#status")).toContainText("Manifestation Available");

    expect(attached.presentation.basis.body_id).toBe(snapshot.presentation.basis.body_id);
    expect(attached.revision).toBeGreaterThan(snapshot.revision);
    const browserHostId = await page.evaluate(() => globalThis.__patchbayMembership.hostId);
    const browserBootId = await page.evaluate(() => globalThis.__patchbayMembership.bootId);
    const browserPart = attached.parts.parts.find(({ details }) =>
      details.host_id === browserHostId && details.boot_id === browserBootId
    );
    expect(browserPart?.details.capabilities).toHaveLength(3);
    expect(attached.presentation.subjects.filter((subject) =>
      subject.role === "Capability" && subject.identity.includes(browserHostId)
    )).toHaveLength(3);
    const lineIdentity = attached.presentation.subjects.find((subject) =>
      subject.role === "Line"
    )?.identity;
    expect(lineIdentity).toBe("line/patchbay-html/browser-admission-line");
    expect(attached.presentation.properties).toEqual(expect.arrayContaining([
      expect.objectContaining({ subject: lineIdentity, name: "base", value: { ConnectionBase: "WebSocket" } }),
      expect.objectContaining({ subject: lineIdentity, name: "availability", value: { Text: "Ready" } }),
    ]));
    const line = page.locator(`#subjects button[data-subject="${lineIdentity}"]`);
    await line.click();
    await expect(line).toHaveAttribute("aria-pressed", "true");
    await expect(page.locator("#inspector .exact-selection")).toContainText(lineIdentity);
    const hostIdentity = attached.presentation.subjects.find((subject) =>
      subject.role === "Host" && subject.identity.includes(browserHostId) && subject.identity.includes(browserBootId)
    )?.identity;
    expect(hostIdentity).toBeTruthy();
    const host = page.locator(`#subjects button[data-subject="${hostIdentity}"]`);
    await host.click();
    await expect(host).toHaveAttribute("aria-pressed", "true");
    await expect(page.locator("#inspector .exact-selection")).toContainText(hostIdentity);
    const selected = await (await fetch(`${url}/api/snapshot`)).json();
    expect(selected.entrance.selected_subject).toBe(hostIdentity);
    expect(selected.entrance.available_actions).toEqual(["Inspect"]);

    await page.locator('[data-lens="form"]').click();
    await expect(page.locator("body")).toHaveAttribute("data-lens", "form");
    await expect(page.locator("#graph .gear")).toHaveCount(2);
    await expect(page.getByRole("navigation", { name: "Patchbay workspace" })).toContainText("Intent");

    await page.getByRole("button", { name: "Plan current Form" }).click();
    await expect(page.locator("#front-door-feedback")).toContainText("Plan Succeeded");
    const planned = await (await fetch(`${url}/api/snapshot`)).json();
    expect(planned.revision).toBe(attached.revision + 1);
    expect(planned.presentation.basis.plan_id).toBeTruthy();
    expect(planned.presentation.basis.active_play_id).toBeNull();
    expect(planned.entrance.selected_subject).toBe(hostIdentity);
    expect(planned.presentation.subjects.some((subject) => subject.role === "Plan")).toBe(true);
    await expect(page.getByRole("button", { name: "Play current Plan" })).toBeEnabled();

    await page.getByRole("button", { name: "Play current Plan" }).click();
    await expect(page.locator("#front-door-feedback")).toContainText("Play Succeeded");
    const playing = await (await fetch(`${url}/api/snapshot`)).json();
    expect(playing.revision).toBe(attached.revision + 2);
    expect(playing.presentation.basis.plan_id).toBe(planned.presentation.basis.plan_id);
    expect(playing.presentation.basis.active_play_id).toBeTruthy();
    expect(playing.entrance.selected_subject).toBe(hostIdentity);
    expect(playing.presentation.subjects.some((subject) => subject.role === "Play")).toBe(true);
    expect(playing.parts.parts[0].in_plan).toBe(true);
    expect(playing.parts.parts[0].playing).toBe(true);
    await page.locator('[data-lens="plan"]').click();
    await expect(page.locator("body")).toHaveAttribute("data-lens", "plan");
    await expect(page.locator("#plan")).toContainText(playing.presentation.basis.plan_id);
    await page.locator('[data-lens="play"]').click();
    await expect(page.locator("#play")).toContainText(playing.presentation.basis.active_play_id);

    const stale = await page.evaluate(async () =>
      (await fetch("/api/interaction", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          presentation_id: "presentation/stale",
          kind: "select",
          subject: "subject/not-current",
        }),
      })).json(),
    );
    expect(stale.interaction.last_disposition).toBe("Refused(StalePresentation)");
    expect(stale.entrance.selected_subject).toBe(hostIdentity);

    const receiptPath = process.env.CONDUIT_PATCHBAY_FRONT_DOOR_RECEIPT_PATH;
    if (receiptPath) {
      const receipt = {
        schema: "conduit.patchbay/front-door-capstone@1",
        proof_class: "live-browser",
        browser_engine: "chromium",
        browser_version: browser.version(),
        body_id: playing.presentation.basis.body_id,
        wake_id: playing.presentation.basis.wake_id,
        source_document_id: playing.presentation.basis.source_document_id,
        checked_form_id: playing.presentation.basis.checked_form_id,
        expanded_form_id: playing.presentation.basis.expanded_form_id,
        revisions: [snapshot.revision, attached.revision, planned.revision, playing.revision],
        presentation_ids: [
          snapshot.presentation.identity,
          attached.presentation.identity,
          planned.presentation.identity,
          playing.presentation.identity,
        ],
        browser_host_id: browserHostId,
        browser_boot_id: browserBootId,
        browser_capability_ids: browserPart.details.capabilities.map(({ capability_id }) => capability_id),
        line_id: lineIdentity,
        line_base: "WebSocket",
        plan_id: playing.presentation.basis.plan_id,
        active_play_id: playing.presentation.basis.active_play_id,
        subjects: playing.presentation.subjects
          .map(({ identity, role }) => ({ identity, role }))
          .sort((left, right) => left.identity.localeCompare(right.identity)),
        relationships: playing.presentation.relationships,
        properties: playing.presentation.properties,
        selection: playing.entrance.selected_subject,
        actions: playing.entrance.available_actions,
        layer: playing.entrance.layer,
        stale_outcome: stale.interaction.last_disposition,
        manifestation: {
          identity: playing.renderer.manifestation.manifestation_id,
          presentation_id: playing.renderer.manifestation.presentation_id,
          presentation_revision: playing.renderer.manifestation.presentation_revision,
          lifecycle: playing.renderer.manifestation.lifecycle,
        },
        assertions: {
          canonical_world_first: true,
          browser_host_attached_to_presented_body: attached.parts.parts.some(
            ({ details }) => details.host_id === browserHostId && details.boot_id === browserBootId,
          ),
          selection_preserved: playing.entrance.selected_subject === hostIdentity,
          plan_then_play: Boolean(
            playing.presentation.basis.plan_id && playing.presentation.basis.active_play_id,
          ),
          stale_refused_without_selection_change: true,
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
