import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

const STEPS = [
  ["body.absent", "body-absent", "runtime-receipt"], ["bootstrap.started", "bootstrap-started", "runtime-receipt"],
  ["body.born", "body-born", "body-biography"], ["body.awake", "body-awake", "body-biography"],
  ["form.used", "standing-form-used", "runtime-receipt"], ["body.inspected", "body-inspected", "semantic-face"],
  ["workload.revised", "workload-revised", "body-biography"], ["host.added", "host-added", "runtime-receipt"],
  ["fault.observed", "fault-observed", "stream-disposition"], ["body.repaired", "body-repaired", "runtime-receipt"],
  ["body.long-running", "body-long-running", "runtime-receipt"], ["body.lulled", "body-lulled", "lifecycle-action"],
  ["body.fulfilled", "body-fulfilled", "fulfilled-transition"],
];
const MASK_ACTION_IDS = [
  "mask.inspect-initial-show", "mask.wear-alternate", "mask.prefer-alternate",
  "mask.withdraw-selected-route", "mask.inspect-unavailable-show", "mask.add-face-host",
  "mask.admit-replacement-plan", "mask.inspect-replanned-show", "mask.doff-alternate",
  "mask.inspect-restored-show",
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
  const make = source.host_make?.obtainment;
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
  const inspectedMask = captures["body.inspected"]?.mask;
  if (inspectedMask?.schema !== "conduit.browser/mask-observation@1") {
    throw new Error("browser track lacks the Rust-owned Mask observation for body.inspected");
  }
  if (inspectedMask.mask_show?.show?.lifecycle !== "Available") {
    throw new Error("browser track refuses a Show that was not made Available by exact DOM acknowledgement");
  }
  if (inspectedMask.execution?.schema !== "conduit.browser/mask-kernel-execution@1"
      || inspectedMask.execution.fore?.length !== 3) {
    throw new Error("browser track lacks the Mask kernel Fore execution receipt");
  }
  const interaction = inspectedMask.interaction;
  if (interaction?.schema !== "conduit.browser/mask-interaction@1"
      || interaction.correlation?.interaction?.presentation_id !== inspectedMask.presentation?.identity
      || interaction.correlation?.interaction?.presentation_revision !== inspectedMask.presentation?.revision
      || interaction.correlation?.interaction?.manifestation_id !== inspectedMask.mask_show?.show?.manifestation_id) {
    throw new Error("browser track lacks exact Face/Show-correlated Mask interaction");
  }
  const kernelSigns = new Set(inspectedMask.execution.remote_signs?.map(sign => sign.kind));
  for (const requiredSign of ["RemoteInputAdmitted", "RemoteInputClosed", "RemoteValueOffered",
    "RemoteValueAccepted", "RemoteValueDelivered", "RemoteOutputClosed"]) {
    if (!kernelSigns.has(requiredSign)) throw new Error(`browser track lacks Mask kernel Sign ${requiredSign}`);
  }
  const ids = { body: required(current.body_id, "Body identity"), host: required(current.host_id, "Host identity"),
    boot: required(current.boot_id, "Boot identity"), peerHost: required(peer?.host_id, "peer Host identity"),
    peerBoot: required(peer?.boot_id, "peer Boot identity"), plan: required(plan.plan_id, "Plan identity"),
    play: required(plan.active_play_id, "Play identity"),
    face: required(inspectedMask.presentation?.identity, "Face identity"),
    show: required(inspectedMask.mask_show?.show?.manifestation_id, "Show identity"),
    maskForm: required(inspectedMask.planned_mask?.mask?.form_identity?.checked_form_id, "Mask Form identity"),
    maskPlan: required(inspectedMask.planned_mask?.plan?.plan_id, "Mask Plan identity"),
    maskPlay: required(inspectedMask.mask_play?.active_play_id, "Mask Play identity") };
  const construction = [
    {
      host_id: ids.host,
      profile: { disposition: "omitted", reason: "The original page Host predates this producer's make step; no profile make identity was retained for it." },
      build: { disposition: "omitted", reason: "The original page Host predates this producer's make step; no build identity was retained for it." },
      image: { disposition: "omitted", reason: "The original page Host predates this producer's make step; no image identity was retained for it." },
    },
    {
      host_id: ids.peerHost,
      profile: { disposition: "exact", identity: required(make?.profile_id, "browser peer profile identity") },
      build: { disposition: "exact", identity: required(make?.build_id, "browser peer build identity") },
      image: { disposition: "exact", identity: required(make?.image_id, "browser peer image identity") },
    },
  ];
  const facts = [
    { initial_body: null, host_id: ids.host, boot_id: ids.boot }, { host_id: ids.host, boot_id: ids.boot },
    { event: events[0] }, { wake: evidence.wakes[0] }, { plan_id: ids.plan, active_play_id: ids.play },
    { face_id: ids.face, show_id: ids.show },
    { workload_revision: current.workload_revision, forms: evidence.body.workset.forms },
    { peer, membership_revision: source.checkpoints.joined.evidence.membership.revision },
    { refusal: source.checkpoints.refused.playback.refusal, wake: refusedWake }, { wake: repairedWake },
    { active_play_id: ids.play }, { lull_sign_id: lull }, { event: events.at(-1) },
  ];
  const signs = [null, null, born, wake, repaired, null, workload,
    required(source.checkpoints.joined.evidence.membership.events.at(-1)?.sign_id, "join sign"),
    fault, repaired, repaired, lull, fulfilled];
  await mkdir(join(output, "artifacts"), { recursive: true });
  const receipts = [];
  for (let index = 0; index < STEPS.length; index += 1) {
    const [stepId, assertion, rung] = STEPS[index];
    const relative = `artifacts/${String(index + 1).padStart(2, "0")}-${stepId}.json`;
    const bytes = Buffer.from(`${JSON.stringify({ schema: "conduit.evidence/semantic-step-receipt@2",
      git_commit: commit, track: "browser-graphical", step_id: stepId, assertion,
      source_facts: facts[index] }, null, 2)}\n`);
    await writeFile(join(output, relative), bytes, { flag: "wx" });
    const hostAdded = stepId === "host.added";
    receipts.push({ step_id: stepId, assertion, disposition: "established", provenance: {
      body_id: index >= 2 ? ids.body : null, host_id: index < 3 ? ids.host : hostAdded ? ids.peerHost : null,
      boot_id: index < 3 ? ids.boot : hostAdded ? ids.peerBoot : null,
      plan_id: ["form.used", "workload.revised", "host.added", "body.repaired"].includes(stepId) ? ids.plan : null,
      play_id: ["form.used", "body.repaired", "body.long-running"].includes(stepId) ? ids.play : null,
      face_id: stepId === "body.inspected" ? ids.face : null,
      show_id: stepId === "body.inspected" ? ids.show : null, line_id: null, sign_id: signs[index],
    }, evidence: [{ artifact_id: `browser-graphical/${stepId}`, evidence_class: "semantic-receipt",
      assertion_rung: rung, documentary_description: `browser-wasm-body producer receipt for ${stepId}.`,
      path: relative, sha256: digest(bytes) }] });
  }
  for (const [stepId, capture] of Object.entries(captures)) {
    const step = receipts.find(candidate => candidate.step_id === stepId);
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
    if (capture.mask) {
      const observationRelative = `artifacts/${stepId}-mask.json`;
      const observationBytes = Buffer.from(`${JSON.stringify(capture.mask, null, 2)}\n`);
      await writeFile(join(output, observationRelative), observationBytes, { flag: "wx" });
      step.evidence.push({ artifact_id: `browser-graphical/${stepId}/mask`, evidence_class: "semantic-receipt",
        assertion_rung: "semantic-face", documentary_description: "Rust-owned ordinary Mask Form, wardrobe, Plan, Play, Face and acknowledged Show observation for this action.",
        path: observationRelative, sha256: digest(observationBytes) });
    }
  }
  const maskActions = structuredClone(captures["body.inspected"]?.maskActions);
  if (!Array.isArray(maskActions)
      || maskActions.length !== MASK_ACTION_IDS.length
      || maskActions.some((action, index) => action.action_id !== MASK_ACTION_IDS[index])) {
    throw new Error("browser track lacks the exact producer-owned Mask journey");
  }
  for (const action of maskActions) {
    action.face_id = action.presentation_id;
    delete action.presentation_id;
  }
  for (let index = 0; index < maskActions.length; index += 1) {
    const action = maskActions[index];
    const stepId = `mask.action-${index}`;
    const assertion = `mask-${action.action_id}`;
    const relative = `artifacts/${String(STEPS.length + index + 1).padStart(2, "0")}-${stepId}.json`;
    const bytes = Buffer.from(`${JSON.stringify({ schema: "conduit.evidence/semantic-step-receipt@2",
      git_commit: commit, track: "browser-graphical", step_id: stepId, assertion,
      source_facts: action }, null, 2)}\n`);
    await writeFile(join(output, relative), bytes, { flag: "wx" });
    receipts.push({ step_id: stepId, assertion, disposition: "established", provenance: {
      body_id: ids.body, host_id: null, boot_id: null, plan_id: action.plan_id, play_id: null,
      face_id: action.face_id, show_id: action.show_id,
      line_id: null, sign_id: null,
    }, evidence: [{ artifact_id: `browser-graphical/mask-action-${index}`,
      evidence_class: "semantic-receipt", assertion_rung: "semantic-face",
      documentary_description: `browser-wasm-body producer outcome for ${action.action_id}.`,
      path: relative, sha256: digest(bytes) }] });
    action.receipt_ids = [stepId];
  }
  if (videoPath) {
    const bytes = await readFile(videoPath);
    const relative = "artifacts/browser-body-session.webm";
    await writeFile(join(output, relative), bytes, { flag: "wx" });
    receipts.at(-1).evidence.push({ artifact_id: "browser-graphical/session-video",
      evidence_class: "video", assertion_rung: "deterministic-observation",
      documentary_description: "Watch the complete uncut browser session, including birth, host admission, refusal, recovery and fulfillment.",
      path: relative, sha256: digest(bytes) });
  }
  const publicActions = [
    { action_id: "journey.bootstrap", concrete_event: "The browser Host started with no Body, then accepted the bounded bootstrap through the live DOM entrance.", receipt_ids: ["body.absent", "bootstrap.started"] },
    { action_id: "journey.birth", concrete_event: "The browser birth action created this independent Body and admitted its first wake.", receipt_ids: ["body.born", "body.awake"] },
    { action_id: "journey.useful-work", concrete_event: "A browser interaction exercised the standing Form through its retained Plan and Play.", receipt_ids: ["form.used"] },
    { action_id: "journey.break-recover", concrete_event: "A refused browser wake remained a fault until a later admitted wake established repair.", receipt_ids: ["fault.observed", "body.repaired"] },
    { action_id: "journey.rest-finish", concrete_event: "Explicit browser actions lulled the Body and then fulfilled its biography.", receipt_ids: ["body.lulled", "body.fulfilled"] },
  ];
  publicActions.splice(3, 0, ...maskActions.map(action => ({
    action_id: action.action_id, concrete_event: action.concrete_event, receipt_ids: action.receipt_ids,
  })));
  await writeFile(join(output, "track.json"), `${JSON.stringify({
    schema: "conduit.evidence/body-journey-track@6", journey_id: "orifina/tutorial@1", git_commit: commit,
    track_id: "browser-graphical", embodiment: "browser-wasm-body", body_id: ids.body,
    mask_form_id: ids.maskForm, construction, hosts: [
      { host_id: ids.host, boot_id: ids.boot }, { host_id: ids.peerHost, boot_id: ids.peerBoot },
    ], line_ids: [], distributed_plan_ids: [], receipts, actions: publicActions,
    mask_actions: maskActions,
  }, null, 2)}\n`, { flag: "wx" });
}
