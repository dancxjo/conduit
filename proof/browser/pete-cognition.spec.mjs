import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { createHash } from "node:crypto";
import { expect, test } from "@playwright/test";
import { persistCaptureDeclaration } from "./capture-declarations.mjs";

function required(name) {
  if (!process.env[name]) throw new Error(`${name} must identify actual retained cognition evidence`);
  return process.env[name];
}
const evidence = required("CONDUIT_PETE_COGNITION_EVIDENCE");
const output = required("CONDUIT_PETE_BROWSER_OUTPUT");
const binary = required("CONDUIT_PETE_INSPECT_BIN");
const json = async path => JSON.parse(await readFile(path, "utf8"));
const exactJson = text => JSON.parse(text, (key, value, context) =>
  typeof value === "number" && ["sign", "execution", "host_session"].includes(key) ? context.source : value);
const hash = async path => createHash("sha256").update(await readFile(path)).digest("hex");
const property = (snapshot, subject, name) => snapshot.face.properties.find(value => value.subject === subject && value.name === name)?.value;
async function capture(page, browser, name, snapshot, plan, play) {
  await page.locator("#structured-navigator").evaluate(element => { element.closest("details").open = false; });
  await page.locator("#center-flow").click();
  await page.locator("#inspector").evaluate(element => { element.scrollTop = 0; });
  await page.evaluate(() => document.fonts.ready);
  const font = await page.evaluate(() => {
    const style = getComputedStyle(document.documentElement);
    return { actual: style.fontFamily, declared: style.getPropertyValue("--conduit-font-body").trim() };
  });
  expect(font.actual.replaceAll('"', "")).toBe(font.declared.replaceAll('"', ""));
  await writeFile(join(output, `${name}.json`), JSON.stringify(snapshot, null, 2), { flag: "wx" });
  await page.screenshot({ path: join(output, `${name}.png`), fullPage: true, animations: "disabled", caret: "hide", scale: "css" });
  await persistCaptureDeclaration(output, { id: `pete-cognition.${name}`, kind: "screenshot", path: `${name}.png`, media_type: "image/png", required: true,
    provenance: { scenario_id: `pete-cognition.${name}@1`, step_id: "make.host.inspect-pete-cognition.browser-proof", browser_engine: "chromium", browser_version: browser.version(),
      viewport: "1366x768", device_scale_factor: "1", locale: "en-US", timezone: "UTC", face_id: snapshot.face.identity, face_revision: String(snapshot.face.revision),
      body_plan_id: plan.plan_id, body_play_id: play.active_play_id, manifestation_id: snapshot.renderer.manifestation.manifestation_id,
      renderer_plan_id: snapshot.renderer.manifestation.plan_id, renderer_play_id: snapshot.renderer.manifestation.active_play_id,
      renderer_id: "patchbay-html/dom-svg@1", asserted_semantic_disposition: "actual historical cognition evidence; no per-Plot Play fabricated" } });
  return { name, png_sha256: await hash(join(output, `${name}.png`)), snapshot_sha256: await hash(join(output, `${name}.json`)), selected_subject: snapshot.navigation.cursor.focus };
}
function startServer() {
  const handle = spawn(binary, [evidence, "--snapshot-output", join(output, "renderer-snapshot.json")], { stdio: ["ignore", "pipe", "pipe"] });
  let errors = "";
  handle.stderr.setEncoding("utf8");
  handle.stderr.on("data", chunk => { errors = (errors + chunk).slice(-8192); });
  const lines = createInterface({ input: handle.stdout });
  const url = new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(`Historical inspector did not bind: ${errors}`)), 10_000);
    lines.on("line", line => { if (line.startsWith("http://")) { clearTimeout(timeout); resolve(line); } });
    handle.once("exit", code => { clearTimeout(timeout); reject(new Error(`Historical inspector exited ${code}: ${errors}`)); });
    handle.once("error", reject);
  });
  return { handle, lines, url };
}

test("actual continuing cognition retains exact causal evidence in historical Patchbay", async ({ page, browser }) => {
  const source = await json(join(evidence, "receipt.json"));
  const plan = await json(join(evidence, "body-plan.json"));
  const run = await json(join(evidence, "body-run.json"));
  const original = await json(join(evidence, "debugger-timeline.json"));
  const dag = exactJson(await readFile(join(evidence, "causal-final-response.json"), "utf8"));
  expect(source.accepted).toBe(true);
  expect(dag.summary.retained_explanation_complete).toBe(true);
  expect(dag.summary.exact_execution_identity).toBe(true);
  expect(dag.summary.all_topology_stages_present).toBe(true);
  expect(dag.summary.two_episode_temporal_feedback_chain).toBe(true);
  expect(dag.summary.missing_branches).toEqual([]);
  const installed = await json("proof/browser/node_modules/@playwright/test/package.json");
  const locked = await json("proof/browser/package-lock.json");
  expect(installed.version).toBe(locked.packages["node_modules/@playwright/test"].version);
  const browsers = await json("proof/browser/node_modules/playwright-core/browsers.json");
  expect(browser.version()).toBe(browsers.browsers.find(value => value.name === "chromium").browserVersion);
  const server = startServer();
  const pageErrors = [];
  page.on("pageerror", error => pageErrors.push(error.message));
  try {
    const url = await server.url;
    const snapshot = await (await page.request.get(new URL("/api/snapshot", url).href)).json();
    expect(snapshot.face.basis.body_id).toBe(plan.body_id);
    expect(snapshot.face.basis.wake_id).toBe(plan.wake_id);
    expect(snapshot.face.actions).toEqual([]);
    const body = snapshot.face.subjects.find(subject => subject.role === "Body");
    expect(property(snapshot, body.identity, "body-plan-id").Identity).toBe(plan.plan_id);
    expect(property(snapshot, body.identity, "body-play-id").Identity).toBe(run.play.active_play_id);
    expect(property(snapshot, body.identity, "historical-terminal").Text).toBe(run.terminal);
    expect(property(snapshot, body.identity, "inspection-scope").Text).toContain("historical-replay");
    const subjects = new Set(snapshot.face.subjects.map(subject => subject.identity));
    const truth = event => { const { subject, related_subject, ...record } = event; return record; };
    expect(snapshot.timeline.events.map(truth)).toEqual(original.timeline.events.map(truth));
    expect(snapshot.timeline.gap).toEqual(original.timeline.gap);
    expect(snapshot.timeline.evicted_events).toBe(original.timeline.evicted_events);
    expect(snapshot.timeline.mode).toBe("replay");
    for (const event of snapshot.timeline.events) {
      expect(subjects.has(event.subject)).toBe(true);
      if (event.related_subject) expect(subjects.has(event.related_subject)).toBe(true);
    }
    const kinds = snapshot.face.subjects.filter(subject => subject.role === "Gear").map(subject => property(snapshot, subject.identity, "kind-id")?.Identity);
    for (const kind of ["experience/situation-current", "experience/autobiography-recall", "experience/integrate-current", "experience/project-evidence-flow", "experience/context-to-prompt", "llm/generate-recoverable-flow", "experience/autobiography-retain-recoverable", "state/value", "time/every"]) expect(kinds).toContain(kind);
    expect(snapshot.face.subjects.filter(subject => subject.role === "Plot")).toHaveLength(2);

    const signId = value => `causal-sign/${value.host_session}/${value.execution}/${value.sign}`;
    for (const node of dag.nodes) {
      const sign = signId(node.evidence);
      expect(subjects.has(sign)).toBe(true);
      expect(property(snapshot, sign, "causal-ordinal").Count).toBe(node.ordinal);
      expect(property(snapshot, sign, "kernel-sequence").Count).toBe(node.kernel_event.sequence);
      expect(exactJson(property(snapshot, sign, "exact-sign-evidence").Text)).toEqual(node.evidence);
      expect(property(snapshot, sign, "sign-evidence-class").Text).toBe(node.outcome);
      for (const [index, metadata] of node.metadata.entries()) {
        expect(exactJson(property(snapshot, sign, `sign-metadata-${index}`).Text)).toEqual(metadata);
      }
    }
    for (const [index, edge] of dag.edges.entries()) {
      const effect = signId(dag.nodes.find(node => node.ordinal === edge.effect).evidence);
      const cause = signId(dag.nodes.find(node => node.ordinal === edge.cause).evidence);
      expect(property(snapshot, effect, `causal-edge-${index}-cause`).Identity).toBe(cause);
      expect(property(snapshot, effect, `causal-edge-${index}-relationship`).Text).toBe(edge.relationship);
    }
    expect(property(snapshot, body.identity, "dag-all-topology-stages-present").Flag).toBe(true);
    expect(property(snapshot, body.identity, "dag-two-episode-feedback-chain").Flag).toBe(true);
    const finalSign = property(snapshot, body.identity, "dag-final-sign").Identity;
    expect(property(snapshot, finalSign, "causal-root").Flag).toBe(true);
    expect(snapshot.face.subjects.some(subject => subject.role === "Host")).toBe(true);
    for (const partition of plan.plots) for (const fragment of partition.plan.fragments) for (const placement of fragment.placements) {
      const gear = snapshot.face.subjects.find(subject => subject.role === "Gear" && property(snapshot, subject.identity, "placement-id")?.Identity === placement.placement_id);
      expect(gear).toBeTruthy();
      for (const [key, sourceKey] of [["implementation-id", "implementation_id"], ["host-id", "host_id"], ["boot-id", "boot_id"], ["artifact-id", "artifact_id"], ["execution-profile-id", "execution_profile_id"]]) {
        expect(property(snapshot, gear.identity, key).Identity).toBe(placement[sourceKey]);
      }
    }
    expect(snapshot.face.subjects.some(subject => subject.role === "Plan" && property(snapshot, subject.identity, "plan-id")?.Identity === plan.plan_id)).toBe(true);
    expect(snapshot.face.subjects.some(subject => subject.role === "Play" && property(snapshot, subject.identity, "active-play-id")?.Identity === run.play.active_play_id)).toBe(true);

    const secondResult = await json(join(evidence, "response-2.json"));
    const secondResultSubject = "historical-episode/2/model-result";
    expect(property(snapshot, secondResultSubject, "provenance").Text).toBe("ModelDerived");
    expect(property(snapshot, secondResultSubject, "payload").Text).toBe(Buffer.from(secondResult.payload).toString("utf8"));
    expect(property(snapshot, "historical-episode/1/evidence/0", "origin").Text).toBe("HumanReported");
    expect(property(snapshot, "historical-episode/1/evidence/1", "availability").Text).toBe("NotObserved");
    const firstPrompt = await json(join(evidence, "model-request-1.json"));
    const firstHuman = firstPrompt.evidence.items.find(item => item.origin === "human_reported");
    expect(property(snapshot, "historical-episode/1/evidence/0", "content-encoding").Text).toBe("utf8/native-json-text-field");
    expect(property(snapshot, "historical-episode/1/evidence/0", "content").Text).toBe(JSON.parse(firstHuman.content).text);
    expect(property(snapshot, "historical-episode/1/evidence/0", "content-bytes").Count).toBe(Buffer.byteLength(firstHuman.content));
    expect(JSON.parse(property(snapshot, "historical-episode/1/evidence/0", "source-0").Text)).toEqual(firstHuman.sources[0]);

    await page.goto(url);
    await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
    await expect(page.locator("body")).toHaveAttribute("data-historical-replay", "true");
    await expect(page.locator("#truth-summary-title")).toHaveText("Historical evidence");
    await expect(page.getByRole("button", { name: "Plan current plot", exact: true })).toHaveCount(0);
    if (await page.locator("body").getAttribute("data-inspector-open") !== "true") await page.locator("#toggle-inspector").click();
    await page.getByRole("button", { name: "Debug", exact: true }).click();
    await expect(page.locator("#debugger-timeline")).toBeVisible();
    const finalSequence = source.debugger_export.final_response_sequence;
    await page.locator("#debugger-timeline").getByRole("button", { name: new RegExp(`seq ${finalSequence} ·`) }).click();
    await page.locator("#debugger-timeline").getByRole("button", { name: "Trace upstream", exact: true }).click();
    const traced = await (await page.request.get(new URL("/api/snapshot", url).href)).json();
    expect(traced.navigation.cursor.aspect).toBe("Signs");
    expect(traced.timeline.trace.origin_sequence).toBe(finalSequence);
    expect(traced.timeline.trace.steps.length).toBeGreaterThan(1);
    expect(traced.timeline.trace.missing_parent_sequences).toEqual([]);
    await page.locator("#structured-navigator").evaluate(element => { element.closest("details").open = true; });
    await page.locator(`#structured-navigator input[type="radio"][data-subject="${finalSign}"]`).check();
    await expect(page.locator("#inspector .selected-summary")).toContainText("InfoRouted");
    expect(pageErrors).toEqual([]);
    const modelGear = snapshot.face.subjects.find(subject => subject.role === "Gear" && property(snapshot, subject.identity, "kind-id")?.Identity === "llm/generate-recoverable-flow");
    await page.locator(`#structured-navigator input[type="radio"][data-subject="${modelGear.identity}"]`).check();
    await expect(page.locator("#inspector .selected-summary")).toContainText("subject-specific causal Sign");
    const debugSnapshot = await (await page.request.get(new URL("/api/snapshot", url).href)).json();
    const debugCapture = await capture(page, browser, "patchbay-cognition", debugSnapshot, plan, run.play);
    await page.getByRole("button", { name: "Structure", exact: true }).click();
    await page.locator("#structured-navigator").evaluate(element => { element.closest("details").open = true; });
    await page.locator(`#structured-navigator input[type="radio"][data-subject="${secondResultSubject}"]`).check();
    await expect(page.locator("#inspector .selected-summary")).toContainText("cobalt-blue");
    await expect(page.locator("#inspector .selected-summary")).toContainText("ModelDerived");
    const responseSnapshot = await (await page.request.get(new URL("/api/snapshot", url).href)).json();
    const responseCapture = await capture(page, browser, "patchbay-response", responseSnapshot, plan, run.play);
    await writeFile(join(output, "browser-receipt.json"), JSON.stringify({
      schema: "conduit.pete/historical-patchbay-browser-proof@1", rendered_browser_verified: true,
      browser_version: browser.version(), playwright_version: installed.version, workers: 1, retries: 0,
      body_id: plan.body_id, wake_id: plan.wake_id, body_plan_id: plan.plan_id, body_play_id: run.play.active_play_id,
      source_receipt_sha256: await hash(join(evidence, "receipt.json")), source_timeline_sha256: await hash(join(evidence, "debugger-timeline.json")),
      final_response_causal_sha256: await hash(join(evidence, "causal-final-response.json")),
      actual_events_preserved: snapshot.timeline.events.length, collector_gap: snapshot.timeline.gap,
      rendered_face_identity: snapshot.face.identity, renderer_manifestation: responseSnapshot.renderer.manifestation,
      captures: [debugCapture, responseCapture], capture_declarations_sha256: await hash(join(output, "captures.json")), inspector_binary_sha256: await hash(binary),
      camera: { viewport: page.viewportSize(), device_scale_factor: 1, locale: "en-US", timezone: "UTC", animations: "disabled", caret: "hide" },
      causal_nodes: dag.nodes.length, causal_edges: dag.edges.length, two_episode_feedback_verified: dag.summary.two_episode_temporal_feedback_chain,
      scope: "Historical rendered inspection of this actual scripted-input Body Play; no physical sensing or general model-quality claim."
    }, null, 2), { flag: "wx" });
  } finally {
    server.lines.close();
    server.handle.kill("SIGTERM");
  }
});
