import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { expect, test } from "@playwright/test";

test("Patchbay explains unchanged split Text Lab through Program and Body", async ({ page }) => {
  const child = spawn("target/debug/patchbay-html", [
    "--text-lab-split", "ws://127.0.0.1:1/conduit",
  ], { stdio: ["ignore", "pipe", "pipe"] });
  const errors = [];
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", chunk => errors.push(chunk));
  const output = createInterface({ input: child.stdout });
  const url = await new Promise((resolve, reject) => {
    output.once("line", line => resolve(line.replace("PATCHBAY_HTML_URL=", "")));
    child.once("exit", code => reject(new Error(`split Patchbay exited ${code}: ${errors.join("")}`)));
  });
  try {
    await page.goto(url);
    await expect(page.getByText(
      "keyboard here -> uppercase there -> presentation here",
      { exact: true },
    ).first()).toBeVisible();
    const snapshot = await page.evaluate(async () => (await fetch("/api/snapshot")).json());
    expect(snapshot.presentation.basis).toMatchObject({
      source_document_id: expect.any(String),
      checked_form_id: expect.any(String),
      expanded_form_id: expect.any(String),
      body_id: expect.any(String),
      wake_id: expect.any(String),
      plan_id: expect.any(String),
    });
    expect(snapshot.presentation.basis.active_play_id).toBeNull();
    const property = (subject, name, value) => snapshot.presentation.properties.some(candidate =>
      candidate.subject === subject && candidate.name === name && candidate.value.Identity === value
    );
    const upper = snapshot.presentation.subjects.find(subject =>
      subject.role === "Gear" && property(subject.identity, "host-id", "text-lab/browser")
    );
    const browserHost = snapshot.presentation.subjects.find(subject =>
      subject.role === "Host" && property(subject.identity, "host-id", "text-lab/browser")
    );
    expect(upper).toBeTruthy();
    expect(browserHost).toBeTruthy();
    expect(snapshot.presentation.relationships).toContainEqual({
      source: upper.identity,
      target: browserHost.identity,
      kind: "Realizes",
    });
    const programPlan = snapshot.navigation.navigation.places
      .find(place => place.place === "Program").aspects.find(aspect => aspect.aspect === "Plan");
    const bodyPlan = snapshot.navigation.navigation.places
      .find(place => place.place === "Body").aspects.find(aspect => aspect.aspect === "Plan");
    expect(programPlan.focusable_subjects).toContain(upper.identity);
    expect(programPlan.focusable_subjects).not.toContain(browserHost.identity);
    expect(bodyPlan.focusable_subjects).toContain(browserHost.identity);
    expect(bodyPlan.focusable_subjects).not.toContain(upper.identity);
    const follow = snapshot.navigation.navigation.follows.find(candidate =>
      candidate.source_subject === upper.identity && candidate.target_subject === browserHost.identity
    );
    expect(follow).toMatchObject({ target_place: "Body", target_aspect: "Plan" });
  } finally {
    output.close();
    if (child.exitCode === null) child.kill("SIGTERM");
  }
});
