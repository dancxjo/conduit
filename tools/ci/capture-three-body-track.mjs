import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

const [mode, commit, sourcePath, mediaPath, output] = process.argv.slice(2);
if (!/^[0-9a-f]{40}$/.test(commit ?? "") || !["native", "browser", "generative"].includes(mode)
    || !sourcePath || !mediaPath || !output) {
  throw new Error("usage: capture-three-body-track.mjs MODE COMMIT SOURCE MEDIA|- OUTPUT");
}

const readJson = async path => JSON.parse(await readFile(path, "utf8"));
const sha = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const requireTruth = (condition, message) => {
  if (!condition) throw new Error(`${mode} Body receipt refused: ${message}`);
};
const requireIdentity = (value, label) => {
  requireTruth(typeof value === "string" && value.length > 0 && value.length <= 512, `missing ${label}`);
  return value;
};

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

function nativeTrack(source) {
  requireTruth(source.schema === "conduit.conduitos/product-journey-proof@4", "unexpected schema");
  requireTruth(source.base_commit === commit, "stale exact commit");
  requireTruth(source.open_effects === 0, "world state already had effects");
  requireTruth(source.workset?.patchbay_presenters_replanned === true, "workload was not revised and replanned");
  requireTruth(source.usb_line_membership === "admitted-present", "peer was not admitted and present");
  requireTruth(source.usb_line_final_membership === "admitted-offline", "peer loss was not retained");
  requireTruth(source.resize_input_refused_while_invalidated === true, "honest manifestation fault is absent");
  requireTruth(source.body_retained_after_lull === true && source.body_fulfilled === true,
    "lull and fulfillment were not retained");
  requireTruth(source.remained_alive === true, "body did not remain alive");
  const ids = {
    body: requireIdentity(source.body_id, "Body identity"),
    host: requireIdentity(source.host_id, "Host identity"),
    boot: requireIdentity(source.boot_id, "Boot identity"),
    peerHost: requireIdentity(source.usb_line_peer_host_id, "peer Host identity"),
    peerBoot: requireIdentity(source.usb_line_peer_boot_id, "peer Boot identity"),
    plan: requireIdentity(source.plan_id, "Plan identity"),
    distributedPlan: requireIdentity(source.usb_line_plan_id, "distributed Plan identity"),
    play: requireIdentity(source.active_play_id, "Play identity"),
    presentation: requireIdentity(source.inspector_presentation_id, "inspection Presentation identity"),
    manifestation: requireIdentity(source.inspector_manifestation_id, "inspection Manifestation identity"),
    line: requireIdentity(source.usb_line_id, "Line identity"),
    born: requireIdentity(source.born_sign_id, "birth Sign identity"),
    fulfilled: requireIdentity(source.fulfilled_sign_id, "fulfillment Sign identity"),
  };
  return {
    name: "native-graphical", embodiment: "freestanding-native-body",
    presenter: "presenter/native-graphical@1", ids, distributed: true,
    facts: {
      "body.absent": { open_effects: source.open_effects, host_id: ids.host, boot_id: ids.boot },
      "bootstrap.started": { profile_id: source.profile_id, build_id: source.build_id, image_id: source.image_id },
      "body.born": { body_id: ids.body, born_sign_id: ids.born, part_id: source.part_id },
      "body.awake": { wake_id: source.wake_id, plan_id: ids.plan },
      "form.used": { input_sign_id: source.input_sign_id, result_sign_id: source.result_sign_id, result: source.result },
      "body.inspected": { presentation_id: ids.presentation, manifestation_id: ids.manifestation },
      "workload.revised": { workset: source.workset, plan_id: ids.plan },
      "host.added": { host_id: ids.peerHost, boot_id: ids.peerBoot, membership: source.usb_line_membership,
        line_id: ids.line, plan_id: ids.distributedPlan },
      "fault.observed": { invalidated_manifestation_id: source.resize_invalidated_manifestation_id,
        input_refused: source.resize_input_refused_while_invalidated, final_membership: source.usb_line_final_membership },
      "body.repaired": { current_manifestation_id: source.resize_current_manifestation_id, plan_id: ids.plan },
      "body.long-running": { remained_alive: source.remained_alive, active_play_id: ids.play },
      "body.lulled": { body_retained_after_lull: source.body_retained_after_lull },
      "body.fulfilled": { fulfilled_sign_id: ids.fulfilled, body_fulfilled: source.body_fulfilled },
    },
  };
}

function browserTrack(source, png) {
  requireTruth(source.schema === "conduit.browser/body-journey@1", "unexpected schema");
  requireTruth(source.git_commit === commit, "stale exact commit");
  const body = source.current;
  const evidence = source.evidence?.evidence;
  const events = evidence?.body?.events;
  const joined = source.checkpoints?.joined?.evidence?.membership?.parts
    ?.map(({ current }) => current).filter(Boolean);
  const peer = joined?.find(entry => entry.host_id !== body?.host_id);
  const refused = source.checkpoints?.refused;
  const repaired = source.checkpoints?.repaired;
  const wake = repaired?.evidence?.evidence?.wakes?.at(-1);
  const plan = wake?.plans?.[0];
  requireTruth(Array.isArray(events) && events[0]?.Born && events.at(-1)?.Fulfilled,
    "birth-to-fulfillment biography is absent");
  requireTruth(joined?.length === 2 && peer, "reviewed peer membership is absent");
  requireTruth(refused?.playback?.refusal?.code === "ExecutionLineUnavailable", "honest Line refusal is absent");
  requireTruth(wake?.lifecycle === "Playing", "repair did not restore a Play");
  requireTruth(body.workload_revision === 1 && source.state?.state === "Fulfilled",
    "workload revision or fulfillment is absent");
  const ids = {
    body: requireIdentity(body.body_id, "Body identity"), host: requireIdentity(body.host_id, "Host identity"),
    boot: requireIdentity(body.boot_id, "Boot identity"), peerHost: requireIdentity(peer.host_id, "peer Host identity"),
    peerBoot: requireIdentity(peer.boot_id, "peer Boot identity"), plan: requireIdentity(plan.plan_id, "Plan identity"),
    play: requireIdentity(plan.active_play_id, "Play identity"),
    presentation: requireIdentity(source.state.terminal.presentation_id, "Presentation identity"),
    manifestation: sha(png), born: requireIdentity(events[0].Born.sign_id, "birth Sign identity"),
    fulfilled: requireIdentity(events.at(-1).Fulfilled.sign_id, "fulfillment Sign identity"),
    fault: requireIdentity(refused.evidence.evidence.wakes.at(-1).sign_ids.at(-1), "fault Sign identity"),
  };
  return {
    name: "browser-graphical", embodiment: "browser-wasm-body", presenter: "presenter/browser-dom@1",
    ids, distributed: false,
    facts: {
      "body.absent": { initial_body: null, host_id: ids.host, boot_id: ids.boot },
      "bootstrap.started": { host_id: ids.host, boot_id: ids.boot },
      "body.born": { event: events[0] }, "body.awake": { wake: evidence.wakes[0] },
      "form.used": { plan_id: ids.plan, active_play_id: ids.play },
      "body.inspected": { presentation_id: ids.presentation, manifestation_id: ids.manifestation },
      "workload.revised": { workload_revision: body.workload_revision, forms: evidence.body.workset.forms },
      "host.added": { peer, membership_revision: source.checkpoints.joined.evidence.membership.revision },
      "fault.observed": { refusal: refused.playback.refusal, wake: refused.evidence.evidence.wakes.at(-1) },
      "body.repaired": { wake }, "body.long-running": { active_play_id: ids.play },
      "body.lulled": { lifecycle: "Lulled" }, "body.fulfilled": { event: events.at(-1) },
    },
  };
}

function generativeTrack(source) {
  const journey = source.body_journey;
  const request = source.local_model?.presenter_requests?.[0];
  const events = journey?.biography?.body?.events;
  requireTruth(journey?.schema === "conduit.evidence/orifina-body-journey@1", "unexpected schema");
  requireTruth(journey.host_ids?.length === 2 && journey.boot_ids?.length === 2,
    "two exact Host boots are absent");
  requireTruth(journey.workload_revision === 1 && journey.repaired === true && journey.fulfilled === true,
    "revision, repair, or fulfillment is absent");
  requireTruth(request?.manifestation?.disposition === "Produced", "model manifestation did not complete");
  requireTruth(events?.[0]?.Born && events.at(-1)?.Fulfilled, "birth-to-fulfillment biography is absent");
  const ids = {
    body: requireIdentity(journey.body_id, "Body identity"), host: requireIdentity(journey.host_ids[0], "Host identity"),
    boot: requireIdentity(journey.boot_ids[0], "Boot identity"),
    peerHost: requireIdentity(journey.host_ids[1], "peer Host identity"),
    peerBoot: requireIdentity(journey.boot_ids[1], "peer Boot identity"),
    plan: requireIdentity(journey.plan_ids.at(-1), "Plan identity"),
    play: requireIdentity(journey.play_ids.at(-1), "Play identity"),
    presentation: requireIdentity(request.source_presentation_identity, "Presentation identity"),
    manifestation: requireIdentity(request.manifestation.manifestation_identity, "Manifestation identity"),
    born: requireIdentity(events[0].Born.sign_id, "birth Sign identity"),
    fulfilled: requireIdentity(events.at(-1).Fulfilled.sign_id, "fulfillment Sign identity"),
  };
  return {
    name: "hosted-generative", embodiment: "hosted-open-weight-model-body",
    presenter: "std/local-open-weight-model@1", ids, distributed: false,
    facts: {
      "body.absent": { initial_body: null, host_id: ids.host, boot_id: ids.boot },
      "bootstrap.started": { provider: request.manifestation.provider_identity,
        model: request.manifestation.model_identity },
      "body.born": { event: events[0] }, "body.awake": { wakes: journey.biography.wakes.slice(0, 1) },
      "form.used": { plan_id: journey.plan_ids[0], play_id: journey.play_ids[0] },
      "body.inspected": { presentation_id: ids.presentation, manifestation: request.manifestation },
      "workload.revised": { workload_revision: journey.workload_revision },
      "host.added": { host_id: ids.peerHost, boot_id: ids.peerBoot, membership: journey.biography.membership },
      "fault.observed": { reason: journey.fault_reason, wakes: journey.biography.wakes },
      "body.repaired": { repaired: journey.repaired, plan_id: ids.plan, play_id: ids.play },
      "body.long-running": { active_play_id: ids.play }, "body.lulled": { repaired: journey.repaired },
      "body.fulfilled": { event: events.at(-1), fulfilled: journey.fulfilled },
    },
  };
}

async function writeTrack(track) {
  await mkdir(join(output, "artifacts"), { recursive: true });
  const steps = [];
  for (let index = 0; index < milestones.length; index += 1) {
    const [stepId, assertion, rung] = milestones[index];
    requireTruth(track.facts[stepId] !== undefined, `missing facts for ${stepId}`);
    const path = join("artifacts", `${String(index + 1).padStart(2, "0")}-${stepId}.json`);
    const bytes = Buffer.from(`${JSON.stringify({ schema: "conduit.evidence/semantic-step-receipt@2",
      git_commit: commit, track: track.name, step_id: stepId, assertion, source_facts: track.facts[stepId] }, null, 2)}\n`);
    await writeFile(join(output, path), bytes);
    const hostAdded = stepId === "host.added";
    const provenance = {
      body_id: index >= 2 ? track.ids.body : null,
      host_id: index < 3 ? track.ids.host : (hostAdded ? track.ids.peerHost : null),
      boot_id: index < 3 ? track.ids.boot : (hostAdded ? track.ids.peerBoot : null),
      plan_id: ["form.used", "workload.revised", "host.added", "body.repaired"].includes(stepId)
        ? (hostAdded && track.distributed ? track.ids.distributedPlan : track.ids.plan) : null,
      play_id: ["form.used", "body.repaired", "body.long-running"].includes(stepId) ? track.ids.play : null,
      presentation_id: stepId === "body.inspected" ? track.ids.presentation : null,
      manifestation_id: stepId === "body.inspected" ? track.ids.manifestation : null,
      line_id: hostAdded && track.distributed ? track.ids.line : null,
      sign_id: index >= 2 ? (stepId === "body.fulfilled" ? track.ids.fulfilled
        : (stepId === "fault.observed" && track.ids.fault ? track.ids.fault : track.ids.born)) : null,
    };
    steps.push({ step_id: stepId, assertion, disposition: "established", provenance,
      evidence: [{ artifact_id: `${track.name}/${stepId}`, evidence_class: "semantic-receipt",
        assertion_rung: rung, documentary_description: `${track.embodiment} receipt for ${stepId}.`,
        path, sha256: sha(bytes) }] });
  }
  const hosts = [{ host_id: track.ids.host, boot_id: track.ids.boot }];
  if (track.ids.peerHost) hosts.push({ host_id: track.ids.peerHost, boot_id: track.ids.peerBoot });
  const manifest = { schema: "conduit.evidence/body-journey-track@2", journey_id: "orifina/tutorial@1",
    git_commit: commit, track_id: track.name, embodiment: track.embodiment, body_id: track.ids.body,
    presenter_id: track.presenter, hosts, line_ids: track.distributed ? [track.ids.line] : [],
    distributed_plan_ids: track.distributed ? [track.ids.distributedPlan] : [], steps };
  await writeFile(join(output, "track.json"), `${JSON.stringify(manifest, null, 2)}\n`);
}

const source = await readJson(sourcePath);
let track;
if (mode === "native") track = nativeTrack(source);
if (mode === "browser") track = browserTrack(source, await readFile(mediaPath));
if (mode === "generative") track = generativeTrack(source);
await writeTrack(track);
