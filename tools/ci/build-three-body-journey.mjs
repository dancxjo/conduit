import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";

const [commit, nativePath, browserPath, browserPngPath, modelPath, output] = process.argv.slice(2);
const readJson = async path => JSON.parse(await readFile(path, "utf8"));
const sha = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const native = await readJson(nativePath);
const browser = await readJson(browserPath);
const model = await readJson(modelPath);
const png = await readFile(browserPngPath);
const milestones = [
  ["body.absent", "body-absent", "runtime-receipt"],
  ["bootstrap.started", "bootstrap-started", "runtime-receipt"],
  ["body.born", "body-born", "body-biography"],
  ["body.awake", "body-awake", "body-biography"],
  ["form.used", "standing-form-used", "runtime-receipt"],
  ["body.inspected", "body-inspected", "semantic-presentation"],
  ["workload.revised", "workload-revised", "body-biography"],
  ["host.added", "host-added", "runtime-receipt"],
  ["fault.observed", "fault-observed", "stream-disposition"],
  ["body.repaired", "body-repaired", "runtime-receipt"],
  ["body.long-running", "body-long-running", "runtime-receipt"],
  ["body.lulled", "body-lulled", "lifecycle-action"],
  ["body.fulfilled", "body-fulfilled", "fulfilled-transition"],
];

const nativeIds = {
  body: native.body_id, host: native.host_id, boot: native.boot_id,
  peerHost: "host/qemu-usb-line-peer", peerBoot: "boot/qemu-usb-line-peer/1",
  plan: native.plan_id, distributedPlan: native.usb_line_plan_id, play: native.active_play_id,
  presentation: native.presentation_id, manifestation: native.manifestation_id,
  line: native.usb_line_id, born: native.born_sign_id, fulfilled: native.fulfilled_sign_id,
};
const browserBody = browser.current.body_id;
const browserEvents = browser.evidence.evidence.body.events;
const browserJoined = browser.checkpoints.joined.evidence.membership.parts.map(({ current }) => current).filter(Boolean);
const browserWake = browser.checkpoints.repaired.evidence.evidence.wakes.at(-1);
const browserPlan = browserWake.plans[0];
const browserIds = {
  body: browserBody, host: browser.current.host_id, boot: browser.current.boot_id,
  peerHost: browserJoined.find(entry => entry.host_id !== browser.current.host_id).host_id,
  peerBoot: browserJoined.find(entry => entry.host_id !== browser.current.host_id).boot_id,
  plan: browserPlan.plan_id, play: browserPlan.active_play_id,
  presentation: browser.state.terminal.presentation_id, manifestation: sha(png),
  born: browserEvents[0].Born.sign_id,
  fulfilled: browserEvents.at(-1).Fulfilled.sign_id,
  fault: browser.checkpoints.refused.evidence.evidence.wakes.at(-1).sign_ids.at(-1),
};
const journey = model.body_journey;
const modelRequest = model.local_model.presenter_requests[0];
const modelEvents = journey.biography.body.events;
const modelIds = {
  body: journey.body_id, host: journey.host_ids[0], boot: journey.boot_ids[0],
  peerHost: journey.host_ids[1], peerBoot: journey.boot_ids[1],
  plan: journey.plan_ids.at(-1), play: journey.play_ids.at(-1),
  presentation: modelRequest.source_presentation_identity,
  manifestation: modelRequest.manifestation.manifestation_identity,
  born: modelEvents[0].Born.sign_id, fulfilled: modelEvents.at(-1).Fulfilled.sign_id,
};

const summaries = {
  native: {
    schema: native.schema, base_commit: native.base_commit, body_id: native.body_id,
    host_id: native.host_id, boot_id: native.boot_id, born_sign_id: native.born_sign_id,
    fulfilled_sign_id: native.fulfilled_sign_id, plan_id: native.plan_id,
    active_play_id: native.active_play_id, presentation_id: native.presentation_id,
    manifestation_id: native.manifestation_id, workset: native.workset,
    usb_line_id: native.usb_line_id, usb_line_plan_id: native.usb_line_plan_id,
    body_fulfilled: native.body_fulfilled,
  },
  browser: {
    schema: "conduit.browser/body-journey@1", body_id: browserBody,
    host_id: browserIds.host, boot_id: browserIds.boot,
    peer_host_id: browserIds.peerHost, peer_boot_id: browserIds.peerBoot,
    body_events: browserEvents, failed_wake: browser.checkpoints.refused.evidence.evidence.wakes.at(-1),
    repaired_wake: browserWake, presentation_id: browserIds.presentation,
    manifestation_id: browserIds.manifestation, final_state: browser.state.state,
  },
  generative: {
    schema: journey.schema, body_id: journey.body_id, host_ids: journey.host_ids,
    boot_ids: journey.boot_ids, plan_ids: journey.plan_ids, play_ids: journey.play_ids,
    workload_revision: journey.workload_revision, fault_reason: journey.fault_reason,
    repaired: journey.repaired, fulfilled: journey.fulfilled,
    body_events: journey.biography.body.events,
    presentation_id: modelIds.presentation, manifestation: modelRequest.manifestation,
  },
};

async function buildTrack(name, embodiment, presenter, ids, sourceSummary, distributed = false) {
  const root = join(output, name);
  await mkdir(join(root, "artifacts"), { recursive: true });
  const steps = [];
  for (let index = 0; index < milestones.length; index += 1) {
    const [stepId, assertion, rung] = milestones[index];
    const artifactPath = join("artifacts", `${String(index + 1).padStart(2, "0")}-${stepId}.json`);
    const bytes = Buffer.from(`${JSON.stringify({
      schema: "conduit.evidence/semantic-step-receipt@1", git_commit: commit,
      track: name, step_id: stepId, assertion, source_receipt: sourceSummary,
    }, null, 2)}\n`);
    await writeFile(join(root, artifactPath), bytes);
    const hostAdded = stepId === "host.added";
    const provenance = {
      body_id: index >= 2 ? ids.body : null,
      host_id: index < 3 ? ids.host : (hostAdded ? ids.peerHost : null),
      boot_id: index < 3 ? ids.boot : (hostAdded ? ids.peerBoot : null),
      plan_id: ["form.used", "workload.revised", "host.added", "body.repaired"].includes(stepId)
        ? (hostAdded && distributed ? ids.distributedPlan : ids.plan) : null,
      play_id: ["form.used", "body.repaired", "body.long-running"].includes(stepId) ? ids.play : null,
      presentation_id: stepId === "body.inspected" ? ids.presentation : null,
      manifestation_id: stepId === "body.inspected" ? ids.manifestation : null,
      line_id: hostAdded && distributed ? ids.line : null,
      sign_id: index >= 2 ? (stepId === "body.fulfilled" ? ids.fulfilled : (stepId === "fault.observed" && ids.fault ? ids.fault : ids.born)) : null,
    };
    steps.push({
      step_id: stepId, assertion, disposition: "established", provenance,
      evidence: [{ artifact_id: `${name}/${stepId}`, evidence_class: "semantic-receipt",
        assertion_rung: rung, documentary_description: `${embodiment} receipt for ${stepId}.`,
        path: artifactPath, sha256: sha(bytes) }],
    });
  }
  const hosts = [{ host_id: ids.host, boot_id: ids.boot }];
  if (ids.peerHost) hosts.push({ host_id: ids.peerHost, boot_id: ids.peerBoot });
  const manifest = {
    schema: "conduit.evidence/body-journey-track@2", journey_id: "orifina/tutorial@1",
    git_commit: commit, track_id: name, embodiment, body_id: ids.body,
    presenter_id: presenter, hosts,
    line_ids: distributed ? [ids.line] : [], distributed_plan_ids: distributed ? [ids.distributedPlan] : [], steps,
  };
  await writeFile(join(root, "track.json"), `${JSON.stringify(manifest, null, 2)}\n`);
}

await buildTrack("native-graphical", "freestanding-native-body", "presenter/native-graphical@1", nativeIds, summaries.native, true);
await buildTrack("browser-graphical", "browser-wasm-body", "presenter/browser-dom@1", browserIds, summaries.browser);
await buildTrack("hosted-generative", "hosted-open-weight-model-body", "std/local-open-weight-model@1", modelIds, summaries.generative);
