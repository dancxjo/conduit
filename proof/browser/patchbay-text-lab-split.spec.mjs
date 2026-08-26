import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { expect, test } from "@playwright/test";

function firstLine(child, prefix, errors) {
  const output = createInterface({ input: child.stdout });
  const value = new Promise((resolve, reject) => {
    output.once("line", line => resolve(line.replace(prefix, "")));
    child.once("exit", code => reject(new Error(`process exited ${code}: ${errors.join("")}`)));
  });
  return { output, value };
}

function exitOutcome(child) {
  return new Promise((resolve, reject) => {
    child.once("error", reject);
    child.once("exit", (code, signal) => resolve({ code, signal }));
  });
}

test("one Patchbay session explains split Text Lab then presents actual Line loss", async ({ page }) => {
  const live = spawn("target/debug/text-lab-live-server", [], {
    stdio: ["ignore", "pipe", "pipe"],
  });
  const liveErrors = [];
  live.stderr.setEncoding("utf8");
  live.stderr.on("data", chunk => liveErrors.push(chunk));
  const liveOutput = firstLine(live, "", liveErrors);
  const liveExit = exitOutcome(live);
  const base = await liveOutput.value;

  const patchbay = spawn("target/debug/patchbay-html", ["--text-lab-split", base], {
    stdio: ["ignore", "pipe", "pipe"],
  });
  const patchbayErrors = [];
  patchbay.stderr.setEncoding("utf8");
  patchbay.stderr.on("data", chunk => patchbayErrors.push(chunk));
  const patchbayOutput = firstLine(patchbay, "PATCHBAY_HTML_URL=", patchbayErrors);
  const url = await patchbayOutput.value;

  try {
    await page.goto(url);
    await expect(page.getByText(
      "keyboard here -> uppercase there -> presentation here",
      { exact: true },
    ).first()).toBeVisible();
    const before = await page.evaluate(async () => (await fetch("/api/snapshot")).json());
    expect(before.presentation.basis).toMatchObject({
      source_document_id: expect.any(String),
      checked_form_id: expect.any(String),
      expanded_form_id: expect.any(String),
      body_id: expect.any(String),
      wake_id: expect.any(String),
      plan_id: expect.any(String),
      active_play_id: null,
    });
    const property = (subject, name, value) => before.presentation.properties.some(candidate =>
      candidate.subject === subject && candidate.name === name && candidate.value.Identity === value
    );
    const upper = before.presentation.subjects.find(subject =>
      subject.role === "Gear" && property(subject.identity, "host-id", "text-lab/browser")
    );
    const browserHost = before.presentation.subjects.find(subject =>
      subject.role === "Host" && property(subject.identity, "host-id", "text-lab/browser")
    );
    expect(before.presentation.relationships).toContainEqual({
      source: upper.identity,
      target: browserHost.identity,
      kind: "Realizes",
    });
    const programPlan = before.navigation.navigation.places
      .find(place => place.place === "Program").aspects.find(aspect => aspect.aspect === "Plan");
    const bodyPlan = before.navigation.navigation.places
      .find(place => place.place === "Body").aspects.find(aspect => aspect.aspect === "Plan");
    expect(programPlan.focusable_subjects).toContain(upper.identity);
    expect(programPlan.focusable_subjects).not.toContain(browserHost.identity);
    expect(bodyPlan.focusable_subjects).toContain(browserHost.identity);
    expect(bodyPlan.focusable_subjects).not.toContain(upper.identity);

    await page.getByRole("button", { name: "Observe browser loss" }).click();
    await expect(page.getByRole("status").filter({ hasText: "awaiting the native causal receipt" }))
      .toBeVisible();
    expect(await liveExit).toEqual({ code: 1, signal: null });
    const receipt = JSON.parse(liveErrors.join("").trim());
    expect(receipt).toMatchObject({
      schema: "conduit.text-lab/line-loss@1",
      code: "CND-TEXT-LIVE-301",
      line_id: "text-lab/browser-to-native",
      plan_id: before.presentation.basis.plan_id,
      source_document_id: before.presentation.basis.source_document_id,
      checked_form_id: before.presentation.basis.checked_form_id,
      old_plan_disposition: "immutable",
      fresh_planning: "unrealizable",
      form_unchanged: true,
    });
    const forged = { ...receipt, plan_id: `${receipt.plan_id}-forged` };
    const refused = await fetch(`${url}/api/text-lab-loss`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(forged),
    });
    expect(refused.status).toBe(400);
    expect(await (await fetch(`${url}/api/snapshot`)).json()).toEqual(before);
    const response = await fetch(`${url}/api/text-lab-loss`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(receipt),
    });
    expect(response.ok).toBe(true);
    const after = await response.json();
    expect(after.revision).toBe(before.revision + 1);
    expect(after.presentation.basis.plan_id).toBe(before.presentation.basis.plan_id);
    expect(after.presentation.basis.source_document_id)
      .toBe(before.presentation.basis.source_document_id);
    expect(after.presentation.basis.checked_form_id)
      .toBe(before.presentation.basis.checked_form_id);
    expect(after.presentation.basis.sign_ids).toContain(receipt.sign_id);
    expect(after.presentation.subjects).toContainEqual(expect.objectContaining({
      role: "Sign",
      label: "CND-TEXT-LIVE-301",
    }));
    await page.reload();
    await expect(page.locator("#ordinary-summary"))
      .toContainText("browser Part unavailable -> unchanged Form currently unrealizable");
    await expect(page.getByRole("button", { name: "Observe browser loss" })).toBeDisabled();
  } finally {
    liveOutput.output.close();
    patchbayOutput.output.close();
    if (live.exitCode === null) live.kill("SIGTERM");
    if (patchbay.exitCode === null) patchbay.kill("SIGTERM");
  }
});
