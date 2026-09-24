import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

const STEPS = [
  ["body.absent", "body-absent", "runtime-receipt"], ["bootstrap.started", "bootstrap-started", "runtime-receipt"],
  ["body.born", "body-born", "body-biography"], ["body.awake", "body-awake", "body-biography"],
  ["form.used", "standing-form-used", "runtime-receipt"], ["body.inspected", "body-inspected", "semantic-presentation"],
  ["workload.revised", "workload-revised", "body-biography"], ["host.added", "host-added", "runtime-receipt"],
  ["fault.observed", "fault-observed", "stream-disposition"], ["body.repaired", "body-repaired", "runtime-receipt"],
  ["body.long-running", "body-long-running", "runtime-receipt"], ["body.lulled", "body-lulled", "lifecycle-action"],
  ["body.fulfilled", "body-fulfilled", "fulfilled-transition"],
];
const digest = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const required = (value, label) => {
  if (typeof value !== "string" || value.length === 0) throw new Error(`browser track lacks ${label}`);
  return value;
};

export async function writeBrowserBodyJourneyTrack(source, screenshotPath, output, captures = {}, videoPath) {
  const commit = required(source.git_commit, "exact commit");
  if (!/^[0-9a-f]{40}$/.test(commit)) throw new Error("browser track refuses a non-exact commit");
  const current = source.current;
  const evidence = source.evidence.evidence;
  const events = evidence.body.events;
  const joined = source.checkpoints.joined.evidence.membership.parts.map(part => part.current).filter(Boolean);
  const peer = joined.find(host => host.host_id !== current.host_id);
  const refusedWake = source.checkpoints.refused.evidence.evidence.wakes.at(-1);
  const repairedWake = source.checkpoints.repaired.evidence.evidence.wakes.at(-1);
  const plan = repairedWake.plans[0];
  const born = required(events[0]?.Born?.sign_id, "birth sign");
  const fulfilled = required(events.at(-1)?.Fulfilled?.sign_id, "fulfillment sign");
  const wake = required(evidence.wakes[0]?.sign_ids?.[0], "Wake sign");
  const workload = required(events.find(event => event.FormAdmitted)?.FormAdmitted?.sign_id, "workload sign");
  const fault = required(refusedWake.sign_ids.at(-1), "fault sign");
  const repaired = required(repairedWake.sign_ids.at(-1), "repair sign");
  const lull = required(events.findLast(event => event.LullRetained)?.LullRetained?.sign_id, "Lull sign");
  const manifestation = digest(await readFile(screenshotPath));
  const ids = { body: required(current.body_id, "Body identity"), host: required(current.host_id, "Host identity"),
    boot: required(current.boot_id, "Boot identity"), peerHost: required(peer?.host_id, "peer Host identity"),
    peerBoot: required(peer?.boot_id, "peer Boot identity"), plan: required(plan.plan_id, "Plan identity"),
    play: required(plan.active_play_id, "Play identity"),
    presentation: required(source.state.terminal.presentation_id, "Presentation identity"), manifestation };
  const facts = [
    { initial_body: null, host_id: ids.host, boot_id: ids.boot }, { host_id: ids.host, boot_id: ids.boot },
    { event: events[0] }, { wake: evidence.wakes[0] }, { plan_id: ids.plan, active_play_id: ids.play },
    { presentation_id: ids.presentation, manifestation_id: manifestation },
    { workload_revision: current.workload_revision, forms: evidence.body.workset.forms },
    { peer, membership_revision: source.checkpoints.joined.evidence.membership.revision },
    { refusal: source.checkpoints.refused.playback.refusal, wake: refusedWake }, { wake: repairedWake },
    { active_play_id: ids.play }, { lull_sign_id: lull }, { event: events.at(-1) },
  ];
  const signs = [null, null, born, wake, repaired, null, workload,
    required(source.checkpoints.joined.evidence.membership.events.at(-1)?.sign_id, "join sign"),
    fault, repaired, repaired, lull, fulfilled];
  await mkdir(join(output, "artifacts"), { recursive: true });
  const steps = [];
  for (let index = 0; index < STEPS.length; index += 1) {
    const [stepId, assertion, rung] = STEPS[index];
    const relative = `artifacts/${String(index + 1).padStart(2, "0")}-${stepId}.json`;
    const bytes = Buffer.from(`${JSON.stringify({ schema: "conduit.evidence/semantic-step-receipt@2",
      git_commit: commit, track: "browser-graphical", step_id: stepId, assertion,
      source_facts: facts[index] }, null, 2)}\n`);
    await writeFile(join(output, relative), bytes, { flag: "wx" });
    const hostAdded = stepId === "host.added";
    steps.push({ step_id: stepId, assertion, disposition: "established", provenance: {
      body_id: index >= 2 ? ids.body : null, host_id: index < 3 ? ids.host : hostAdded ? ids.peerHost : null,
      boot_id: index < 3 ? ids.boot : hostAdded ? ids.peerBoot : null,
      plan_id: ["form.used", "workload.revised", "host.added", "body.repaired"].includes(stepId) ? ids.plan : null,
      play_id: ["form.used", "body.repaired", "body.long-running"].includes(stepId) ? ids.play : null,
      presentation_id: stepId === "body.inspected" ? ids.presentation : null,
      manifestation_id: stepId === "body.inspected" ? ids.manifestation : null, line_id: null, sign_id: signs[index],
    }, evidence: [{ artifact_id: `browser-graphical/${stepId}`, evidence_class: "semantic-receipt",
      assertion_rung: rung, documentary_description: `browser-wasm-body producer receipt for ${stepId}.`,
      path: relative, sha256: digest(bytes) }] });
  }
  for (const [stepId, capture] of Object.entries(captures)) {
    const step = steps.find(candidate => candidate.step_id === stepId);
    if (!step) throw new Error(`unknown captured journey step: ${stepId}`);
    const bytes = await readFile(capture.path);
    if (!bytes.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]))) {
      throw new Error(`journey screenshot is not PNG: ${stepId}`);
    }
    const relative = `artifacts/${stepId}.png`;
    await writeFile(join(output, relative), bytes, { flag: "wx" });
    step.evidence.push({ artifact_id: `browser-graphical/${stepId}/screen`,
      evidence_class: "screenshot", assertion_rung: "deterministic-observation",
      documentary_description: capture.caption, path: relative, sha256: digest(bytes) });
  }
  if (videoPath) {
    const bytes = await readFile(videoPath);
    const relative = "artifacts/browser-body-session.webm";
    await writeFile(join(output, relative), bytes, { flag: "wx" });
    steps.at(-1).evidence.push({ artifact_id: "browser-graphical/session-video",
      evidence_class: "video", assertion_rung: "deterministic-observation",
      documentary_description: "Watch the complete uncut browser session, including birth, host admission, refusal, recovery and fulfillment.",
      path: relative, sha256: digest(bytes) });
  }
  await writeFile(join(output, "track.json"), `${JSON.stringify({
    schema: "conduit.evidence/body-journey-track@2", journey_id: "orifina/tutorial@1", git_commit: commit,
    track_id: "browser-graphical", embodiment: "browser-wasm-body", body_id: ids.body,
    presenter_id: "presenter/browser-dom@1", hosts: [
      { host_id: ids.host, boot_id: ids.boot }, { host_id: ids.peerHost, boot_id: ids.peerBoot },
    ], line_ids: [], distributed_plan_ids: [], steps,
  }, null, 2)}\n`, { flag: "wx" });
}
